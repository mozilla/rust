//! The general point of the optimizations provided here is to simplify something like:
//!
//! ```rust
//! match x {
//!     Ok(x) => Ok(x),
//!     Err(x) => Err(x)
//! }
//! ```
//!
//! into just `x`.

use crate::transform::{simplify, MirPass, MirSource};
use itertools::Itertools as _;
use rustc::mir::*;
use rustc::ty::{Ty, TyCtxt};
use rustc_index::vec::IndexVec;
use rustc_target::abi::VariantIdx;

/// Simplifies arms of form `Variant(x) => Variant(x)` to just a move.
///
/// This is done by transforming basic blocks where the statements match:
///
/// ```rust
/// _LOCAL_TMP = ((_LOCAL_1 as Variant ).FIELD: TY );
/// ((_LOCAL_0 as Variant).FIELD: TY) = move _LOCAL_TMP;
/// discriminant(_LOCAL_0) = VAR_IDX;
/// ```
///
/// or
///
/// ```rust
/// StorageLive(_LOCAL_TMP);
/// _LOCAL_TMP = ((_LOCAL_1 as Variant ).FIELD: TY );
/// StorageLive(_LOCAL_TMP_2);
/// _LOCAL_TMP_2 = _LOCAL_TMP
/// ((_LOCAL_0 as Variant).FIELD: TY) = move _LOCAL_TMP_2;
/// discriminant(_LOCAL_0) = VAR_IDX;
/// StorageDead(_LOCAL_TMP_2);
/// StorageDead(_LOCAL_TMP);
/// ```
///
/// into:
///
/// ```rust
/// _LOCAL_0 = move _LOCAL_1
/// ```
pub struct SimplifyArmIdentity;

impl<'tcx> MirPass<'tcx> for SimplifyArmIdentity {
    fn run_pass(&self, _: TyCtxt<'tcx>, _: MirSource<'tcx>, body: &mut BodyAndCache<'tcx>) {
        let (basic_blocks, local_decls) = body.basic_blocks_and_local_decls_mut();
        for bb in basic_blocks {
            match &mut *bb.statements {
                [s0, s1, s2] => match_copypropd_arm([s0, s1, s2], local_decls),
                other => match_arm(other, local_decls),
            }
        }
    }
}

/// Match on:
/// ```rust
/// _LOCAL_TMP = ((_LOCAL_1 as Variant ).FIELD: TY );
/// ((_LOCAL_0 as Variant).FIELD: TY) = move _LOCAL_TMP;
/// discriminant(_LOCAL_0) = VAR_IDX;
/// ```
fn match_copypropd_arm(
    [s0, s1, s2]: [&mut Statement<'_>; 3],
    local_decls: &mut IndexVec<Local, LocalDecl<'_>>,
) {
    // Pattern match on the form we want:
    let (local_tmp_s0, local_1, vf_s0) = match match_get_variant_field(s0) {
        None => return,
        Some(x) => x,
    };
    let (local_tmp_s1, local_0, vf_s1) = match match_set_variant_field(s1) {
        None => return,
        Some(x) => x,
    };
    if local_tmp_s0 != local_tmp_s1
        // The field-and-variant information match up.
        || vf_s0 != vf_s1
        // Source and target locals have the same type.
        // FIXME(Centril | oli-obk): possibly relax to same layout?
        || local_decls[local_0].ty != local_decls[local_1].ty
        // We're setting the discriminant of `local_0` to this variant.
        || Some((local_0, vf_s0.var_idx)) != match_set_discr(s2)
    {
        return;
    }

    // Right shape; transform!
    match &mut s0.kind {
        StatementKind::Assign(box (place, rvalue)) => {
            *place = local_0.into();
            *rvalue = Rvalue::Use(Operand::Move(local_1.into()));
        }
        _ => unreachable!(),
    }
    s1.make_nop();
    s2.make_nop();
}

/// Match on:
/// ```rust
/// StorageLive(_LOCAL_TMP);
/// _LOCAL_TMP = ((_LOCAL_1 as Variant ).FIELD: TY );
/// StorageLive(_LOCAL_TMP_2);
/// _LOCAL_TMP_2 = _LOCAL_TMP
/// ((_LOCAL_0 as Variant).FIELD: TY) = move _LOCAL_TMP_2;
/// discriminant(_LOCAL_0) = VAR_IDX;
/// StorageDead(_LOCAL_TMP_2);
/// StorageDead(_LOCAL_TMP);
/// ```
fn match_arm(stmts: &mut [Statement<'_>], local_decls: &mut IndexVec<Local, LocalDecl<'_>>) {
    if stmts.len() != 8 {
        return;
    }

    // StorageLive(_LOCAL_TMP);
    let local_tmp_live = if let StatementKind::StorageLive(local_tmp_live) = &stmts[0].kind {
        *local_tmp_live
    } else {
        return;
    };

    // _LOCAL_TMP = ((_LOCAL_1 as Variant ).FIELD: TY );
    let (local_tmp, local_1, vf_1) = match match_get_variant_field(&stmts[1]) {
        None => return,
        Some(x) => x,
    };

    if local_tmp_live != local_tmp {
        return;
    }

    // StorageLive(_LOCAL_TMP_2);
    let local_tmp_2_live = if let StatementKind::StorageLive(local_tmp_2_live) = &stmts[2].kind {
        *local_tmp_2_live
    } else {
        return;
    };

    // _LOCAL_TMP_2 = _LOCAL_TMP
    if let StatementKind::Assign(box (lhs, Rvalue::Use(rhs))) = &stmts[3].kind {
        let lhs = lhs.as_local();
        if lhs != Some(local_tmp_2_live) {
            return;
        }
        match rhs {
            Operand::Copy(rhs) | Operand::Move(rhs) => {
                if rhs.as_local() != Some(local_tmp) {
                    return;
                }
            }
            _ => return,
        }
    } else {
        return;
    }

    // ((_LOCAL_0 as Variant).FIELD: TY) = move _LOCAL_TMP_2;
    let (local_tmp_2, local_0, vf_0) = match match_set_variant_field(&stmts[4]) {
        None => return,
        Some(x) => x,
    };

    if local_tmp_2 != local_tmp_2_live {
        return;
    }

    if vf_1 != vf_0 // The field-and-variant information match up.
        // Source and target locals have the same type.
        // FIXME(Centril | oli-obk): possibly relax to same layout?
        || local_decls[local_0].ty != local_decls[local_1].ty
        // We're setting the discriminant of `local_0` to this variant.
        || Some((local_0, vf_1.var_idx)) != match_set_discr(&stmts[5])
    {
        return;
    }

    // StorageDead(_LOCAL_TMP_2);
    // StorageDead(_LOCAL_TMP);
    match (&stmts[6].kind, &stmts[7].kind) {
        (
            StatementKind::StorageDead(local_tmp_2_dead),
            StatementKind::StorageDead(local_tmp_dead),
        ) => {
            if *local_tmp_2_dead != local_tmp_2 || *local_tmp_dead != local_tmp {
                return;
            }
        }
        _ => return,
    }

    // Right shape; transform!
    stmts[0].kind = StatementKind::Assign(Box::new((
        local_0.into(),
        Rvalue::Use(Operand::Move(local_1.into())),
    )));

    for s in &mut stmts[1..] {
        s.make_nop();
    }
}

/// Match on:
/// ```rust
/// _LOCAL_INTO = ((_LOCAL_FROM as Variant).FIELD: TY);
/// ```
fn match_get_variant_field<'tcx>(stmt: &Statement<'tcx>) -> Option<(Local, Local, VarField<'tcx>)> {
    match &stmt.kind {
        StatementKind::Assign(box (place_into, rvalue_from)) => match rvalue_from {
            Rvalue::Use(Operand::Copy(pf)) | Rvalue::Use(Operand::Move(pf)) => {
                let local_into = place_into.as_local()?;
                let (local_from, vf) = match_variant_field_place(&pf)?;
                Some((local_into, local_from, vf))
            }
            _ => None,
        },
        _ => None,
    }
}

/// Match on:
/// ```rust
/// ((_LOCAL_FROM as Variant).FIELD: TY) = move _LOCAL_INTO;
/// ```
fn match_set_variant_field<'tcx>(stmt: &Statement<'tcx>) -> Option<(Local, Local, VarField<'tcx>)> {
    match &stmt.kind {
        StatementKind::Assign(box (place_from, rvalue_into)) => match rvalue_into {
            Rvalue::Use(Operand::Move(place_into)) => {
                let local_into = place_into.as_local()?;
                let (local_from, vf) = match_variant_field_place(&place_from)?;
                Some((local_into, local_from, vf))
            }
            _ => None,
        },
        _ => None,
    }
}

/// Match on:
/// ```rust
/// discriminant(_LOCAL_TO_SET) = VAR_IDX;
/// ```
fn match_set_discr<'tcx>(stmt: &Statement<'tcx>) -> Option<(Local, VariantIdx)> {
    match &stmt.kind {
        StatementKind::SetDiscriminant { place, variant_index } => {
            Some((place.as_local()?, *variant_index))
        }
        _ => None,
    }
}

#[derive(PartialEq)]
struct VarField<'tcx> {
    field: Field,
    field_ty: Ty<'tcx>,
    var_idx: VariantIdx,
}

/// Match on `((_LOCAL as Variant).FIELD: TY)`.
fn match_variant_field_place<'tcx>(place: &Place<'tcx>) -> Option<(Local, VarField<'tcx>)> {
    match place.as_ref() {
        PlaceRef {
            local,
            projection: &[ProjectionElem::Downcast(_, var_idx), ProjectionElem::Field(field, ty)],
        } => Some((*local, VarField { field, field_ty: ty, var_idx })),
        _ => None,
    }
}

/// Simplifies `SwitchInt(_) -> [targets]`,
/// where all the `targets` have the same form,
/// into `goto -> target_first`.
pub struct SimplifyBranchSame;

impl<'tcx> MirPass<'tcx> for SimplifyBranchSame {
    fn run_pass(&self, _: TyCtxt<'tcx>, _: MirSource<'tcx>, body: &mut BodyAndCache<'tcx>) {
        let mut did_remove_blocks = false;
        let bbs = body.basic_blocks_mut();
        for bb_idx in bbs.indices() {
            let targets = match &bbs[bb_idx].terminator().kind {
                TerminatorKind::SwitchInt { targets, .. } => targets,
                _ => continue,
            };

            let mut iter_bbs_reachable = targets
                .iter()
                .map(|idx| (*idx, &bbs[*idx]))
                .filter(|(_, bb)| {
                    // Reaching `unreachable` is UB so assume it doesn't happen.
                    bb.terminator().kind != TerminatorKind::Unreachable
                    // But `asm!(...)` could abort the program,
                    // so we cannot assume that the `unreachable` terminator itself is reachable.
                    // FIXME(Centril): use a normalization pass instead of a check.
                    || bb.statements.iter().any(|stmt| match stmt.kind {
                        StatementKind::InlineAsm(..) => true,
                        _ => false,
                    })
                })
                .peekable();

            // We want to `goto -> bb_first`.
            let bb_first = iter_bbs_reachable.peek().map(|(idx, _)| *idx).unwrap_or(targets[0]);

            // All successor basic blocks should have the exact same form.
            let all_successors_equivalent =
                iter_bbs_reachable.map(|(_, bb)| bb).tuple_windows().all(|(bb_l, bb_r)| {
                    bb_l.is_cleanup == bb_r.is_cleanup
                        && bb_l.terminator().kind == bb_r.terminator().kind
                        && bb_l.statements.iter().eq_by(&bb_r.statements, |x, y| x.kind == y.kind)
                });

            if all_successors_equivalent {
                // Replace `SwitchInt(..) -> [bb_first, ..];` with a `goto -> bb_first;`.
                bbs[bb_idx].terminator_mut().kind = TerminatorKind::Goto { target: bb_first };
                did_remove_blocks = true;
            }
        }

        if did_remove_blocks {
            // We have dead blocks now, so remove those.
            simplify::remove_dead_blocks(body);
        }
    }
}
