use super::{MirPass, MirSource};
use rustc_middle::{
    mir::{
        interpret::Scalar, BasicBlock, BinOp, Body, Operand, Place, Rvalue, Statement,
        StatementKind, TerminatorKind,
    },
    ty::{Ty, TyCtxt},
};

/// Pass to convert `if` conditions on integrals into switches on the integral.
/// For an example, it turns something like
///
/// ```
/// _3 = Eq(move _4, const 43i32);
/// StorageDead(_4);
/// switchInt(_3) -> [false: bb2, otherwise: bb3];
/// ```
///
/// into:
///
/// ```
/// switchInt(_4) -> [43i32: bb3, otherwise: bb2];
/// ```
pub struct SimplifyComparisonIntegral;

impl<'tcx> MirPass<'tcx> for SimplifyComparisonIntegral {
    fn run_pass(&self, _: TyCtxt<'tcx>, source: MirSource<'tcx>, body: &mut Body<'tcx>) {
        trace!("Running SimplifyComparisonIntegral on {:?}", source);

        let helper = OptimizationFinder { body };
        let opts = helper.find_optimizations();
        let mut storage_deads_to_insert = vec![];
        let mut storage_deads_to_remove: Vec<(usize, BasicBlock)> = vec![];
        for opt in opts {
            trace!("SUCCESS: Applying {:?}", opt);
            // replace terminator with a switchInt that switches on the integer directly
            let bbs = &mut body.basic_blocks_mut();
            let bb = &mut bbs[opt.bb_idx];
            // We only use the bits for the untyped, not length checked `values` field. Thus we are
            // not using any of the convenience wrappers here and directly access the bits.
            let new_value = match opt.branch_value_scalar {
                Scalar::Raw { data, .. } => data,
                Scalar::Ptr(_) => continue,
            };
            const FALSE: u128 = 0;
            let mut new_targets = opt.targets.clone();
            let first_is_false_target = opt.values[0] == FALSE;
            match opt.op {
                BinOp::Eq => {
                    // if the assignment was Eq we want the true case to be first
                    if first_is_false_target {
                        new_targets.swap(0, 1);
                    }
                }
                BinOp::Ne => {
                    // if the assignment was Ne we want the false case to be first
                    if !first_is_false_target {
                        new_targets.swap(0, 1);
                    }
                }
                _ => unreachable!(),
            }

            let terminator = bb.terminator_mut();

            // add StorageDead for the place switched on at the top of each target
            for bb_idx in new_targets.iter() {
                storage_deads_to_insert.push((
                    *bb_idx,
                    Statement {
                        source_info: terminator.source_info,
                        kind: StatementKind::StorageDead(opt.to_switch_on.local),
                    },
                ));
            }

            terminator.kind = TerminatorKind::SwitchInt {
                discr: Operand::Move(opt.to_switch_on),
                switch_ty: opt.branch_value_ty,
                values: vec![new_value].into(),
                targets: new_targets,
            };

            // delete comparison statement if it the value being switched on was moved, which means it can not be user later on
            if opt.can_remove_bin_op_stmt {
                bb.statements[opt.bin_op_stmt_idx].make_nop();
            } else {
                // if the integer being compared to a const integral is being moved into the comparison,
                // e.g `_2 = Eq(move _3, const 'x');`
                // we want to avoid making a double move later on in the switchInt on _3.
                // So to avoid `switchInt(move _3) -> ['x': bb2, otherwise: bb1];`,
                // we convert the move in the comparison statement to a copy.

                // unwrap is safe as we know this statement is an assign
                let box (_, rhs) = bb.statements[opt.bin_op_stmt_idx].kind.as_assign_mut().unwrap();

                use Operand::*;
                match rhs {
                    Rvalue::BinaryOp(_, ref mut left @ Move(_), Constant(_)) => {
                        *left = Copy(opt.to_switch_on);
                    }
                    Rvalue::BinaryOp(_, Constant(_), ref mut right @ Move(_)) => {
                        *right = Copy(opt.to_switch_on);
                    }
                    _ => (),
                }
            }

            // remove StorageDead (if it exists) being used in the assign of the comparison
            for (stmt_idx, stmt) in bb.statements.iter().enumerate() {
                if !matches!(stmt.kind, StatementKind::StorageDead(local) if local == opt.to_switch_on.local)
                {
                    continue;
                }
                storage_deads_to_remove.push((stmt_idx, opt.bb_idx))
            }
        }

        for (idx, bb_idx) in storage_deads_to_remove {
            body.basic_blocks_mut()[bb_idx].statements[idx].make_nop();
        }

        for (idx, stmt) in storage_deads_to_insert {
            body.basic_blocks_mut()[idx].statements.insert(0, stmt);
        }
    }
}

struct OptimizationFinder<'a, 'tcx> {
    body: &'a Body<'tcx>,
}

impl<'a, 'tcx> OptimizationFinder<'a, 'tcx> {
    fn find_optimizations(&self) -> Vec<OptimizationInfo<'tcx>> {
        self.body
            .basic_blocks()
            .iter_enumerated()
            .filter_map(|(bb_idx, bb)| {
                // find switch
                let (place_switched_on, values, targets, place_switched_on_moved) = match &bb
                    .terminator()
                    .kind
                {
                    rustc_middle::mir::TerminatorKind::SwitchInt {
                        discr, values, targets, ..
                    } => Some((discr.place()?, values, targets, discr.is_move())),
                    _ => None,
                }?;

                // find the statement that assigns the place being switched on
                bb.statements.iter().enumerate().rev().find_map(|(stmt_idx, stmt)| {
                    match &stmt.kind {
                        rustc_middle::mir::StatementKind::Assign(box (lhs, rhs))
                            if *lhs == place_switched_on =>
                        {
                            match rhs {
                                Rvalue::BinaryOp(op @ (BinOp::Eq | BinOp::Ne), left, right) => {
                                    let (branch_value_scalar, branch_value_ty, to_switch_on) =
                                        find_branch_value_info(left, right)?;

                                    Some(OptimizationInfo {
                                        bin_op_stmt_idx: stmt_idx,
                                        bb_idx,
                                        can_remove_bin_op_stmt: place_switched_on_moved,
                                        to_switch_on,
                                        branch_value_scalar,
                                        branch_value_ty,
                                        op: *op,
                                        values: values.clone().into_owned(),
                                        targets: targets.clone(),
                                    })
                                }
                                _ => None,
                            }
                        }
                        _ => None,
                    }
                })
            })
            .collect()
    }
}

fn find_branch_value_info<'tcx>(
    left: &Operand<'tcx>,
    right: &Operand<'tcx>,
) -> Option<(Scalar, Ty<'tcx>, Place<'tcx>)> {
    // check that either left or right is a constant.
    // if any are, we can use the other to switch on, and the constant as a value in a switch
    use Operand::*;
    match (left, right) {
        (Constant(branch_value), Copy(to_switch_on) | Move(to_switch_on))
        | (Copy(to_switch_on) | Move(to_switch_on), Constant(branch_value)) => {
            let branch_value_ty = branch_value.literal.ty;
            // we only want to apply this optimization if we are matching on integrals (and chars), as it is not possible to switch on floats
            if !branch_value_ty.is_integral() && !branch_value_ty.is_char() {
                return None;
            };
            let branch_value_scalar = branch_value.literal.val.try_to_scalar()?;
            Some((branch_value_scalar, branch_value_ty, *to_switch_on))
        }
        _ => None,
    }
}

#[derive(Debug)]
struct OptimizationInfo<'tcx> {
    /// Basic block to apply the optimization
    bb_idx: BasicBlock,
    /// Statement index of Eq/Ne assignment that can be removed. None if the assignment can not be removed - i.e the statement is used later on
    bin_op_stmt_idx: usize,
    /// Can remove Eq/Ne assignment
    can_remove_bin_op_stmt: bool,
    /// Place that needs to be switched on. This place is of type integral
    to_switch_on: Place<'tcx>,
    /// Constant to use in switch target value
    branch_value_scalar: Scalar,
    /// Type of the constant value
    branch_value_ty: Ty<'tcx>,
    /// Either Eq or Ne
    op: BinOp,
    /// Current values used in the switch target. This needs to be replaced with the branch_value
    values: Vec<u128>,
    /// Current targets used in the switch
    targets: Vec<BasicBlock>,
}
