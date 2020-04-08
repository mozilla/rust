use crate::transform::{MirPass, MirSource};
use crate::util::expand_aggregate;
use rustc_middle::mir::*;
use rustc_middle::ty::TyCtxt;

pub struct Deaggregator;

impl<'tcx> MirPass<'tcx> for Deaggregator {
    fn run_pass(&self, tcx: TyCtxt<'tcx>, _source: MirSource<'tcx>, body: &mut BodyAndCache<'tcx>) {
        let (basic_blocks, local_decls) = body.basic_blocks_and_local_decls_mut();
        let local_decls = &*local_decls;
        for bb in basic_blocks {
            bb.expand_statements(|stmt| {
                // FIXME(eddyb) don't match twice on `stmt.kind` (post-NLL).
                if let StatementKind::Assign(box (_, ref rhs)) = stmt.kind {
                    if let Op::Aggregate(ref kind, _) = *rhs {
                        // FIXME(#48193) Deaggregate arrays when it's cheaper to do so.
                        if let AggregateKind::Array(_) = **kind {
                            return None;
                        }
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }

                let stmt = stmt.replace_nop();
                let source_info = stmt.source_info;
                let (lhs, kind, operands) = match stmt.kind {
                    StatementKind::Assign(box (lhs, rvalue)) => match rvalue {
                        Op::Aggregate(kind, operands) => (lhs, kind, operands),
                        _ => bug!(),
                    },
                    _ => bug!(),
                };

                Some(expand_aggregate(
                    lhs,
                    operands.into_iter().map(|op| {
                        let ty = op.ty(local_decls, tcx);
                        (op, ty)
                    }),
                    *kind,
                    source_info,
                    tcx,
                ))
            });
        }
    }
}
