//! This pass eliminates casting of arrays [T; N] into &[T] when their length
//! is taken using `.len()` method. Handy to preserve information in MIR for const prop

use crate::transform::MirPass;
use rustc_data_structures::fx::FxIndexMap;
use rustc_index::vec::IndexVec;
use rustc_middle::mir::*;
use rustc_middle::ty::{self, TyCtxt};

pub struct NormalizeArrayLen;

impl<'tcx> MirPass<'tcx> for NormalizeArrayLen {
    fn run_pass(&self, tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
        // if tcx.sess.mir_opt_level() < 3 {
        //     return;
        // }
        normalize_array_len_calls(tcx, body)
    }
}

pub fn normalize_array_len_calls<'tcx>(tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
    let (basic_blocks, local_decls) = body.basic_blocks_and_local_decls_mut();

    let mut state = IndexVec::from_elem_n(None, local_decls.len());
    let mut patches_scratchpad = FxIndexMap::default();
    let mut replacements_scratchpad = FxIndexMap::default();
    for block in basic_blocks {
        // make length calls for arrays [T; N] not to decay into length calls for &[T]
        // that forbids constant propagation
        normalize_array_len_call(
            tcx,
            block,
            local_decls,
            &mut state,
            &mut patches_scratchpad,
            &mut replacements_scratchpad
        );
        for el in state.iter_mut() {
            *el = None;
        }
        patches_scratchpad.clear();
        replacements_scratchpad.clear();
    }
}

struct Patcher<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    patches_scratchpad: &'a FxIndexMap<usize, usize>,
    replacements_scratchpad: &'a mut FxIndexMap<usize, Local>,
    local_decls: &'a mut IndexVec<Local, LocalDecl<'tcx>>,
    statement_idx: usize,
}

impl<'a, 'tcx> Patcher<'a, 'tcx> {
    fn patch_expand_statement(&mut self, statement: &mut Statement<'tcx>) -> Option<std::vec::IntoIter<Statement<'tcx>>> {
        let idx = self.statement_idx;
        if let Some(len_statemnt_idx) = self.patches_scratchpad.get(&idx).copied() {
            let mut statements = Vec::with_capacity(2);

            // we are at statement that performs a cast. The only sound way is
            // to create another local that performs a similar copy without a cast and then
            // use this copy in the Len operation

            match &statement.kind {
                StatementKind::Assign(box(.., Rvalue::Cast(CastKind::Pointer(ty::adjustment::PointerCast::Unsize), operand, _))) => {
                    match operand {
                        Operand::Copy(place) | Operand::Move(place) => {
                            // create new local
                            let ty = operand.ty(self.local_decls, self.tcx);
                            let local_decl = LocalDecl::with_source_info(ty, statement.source_info.clone());
                            let local = self.local_decls.push(local_decl);
                            // make it live
                            let mut make_live_statement = statement.clone();
                            make_live_statement.kind = StatementKind::StorageLive(local);
                            statements.push(make_live_statement);
                            // copy into it

                            let operand = Operand::Copy(*place);
                            let mut make_copy_statement = statement.clone();
                            let assign_to = Place::from(local);
                            let rvalue = Rvalue::Use(operand);
                            make_copy_statement.kind = StatementKind::Assign(box(assign_to, rvalue));
                            statements.push(make_copy_statement);

                            // to reorder we have to copy and make NOP
                            statements.push(statement.clone());
                            statement.make_nop();

                            self.replacements_scratchpad.insert(len_statemnt_idx, local);
                        },
                        _ => {unreachable!("it's a bug in the implementation")}
                    }
                },
                _ => {unreachable!("it's a bug in the implementation")}
            }

            self.statement_idx += 1;

            Some(statements.into_iter())
        } else if let Some(local) = self.replacements_scratchpad.get(&idx).copied() {
            let mut statements = Vec::with_capacity(2);

            match &statement.kind {
                StatementKind::Assign(box(into, Rvalue::Len(place))) => {
                    let add_deref = if let Some(..) = place.as_local() {
                        false
                    } else if let Some(..) = place.local_or_deref_local() {
                        true
                    } else {
                        unreachable!("it's a bug in the implementation")
                    };
                    // replace len statement
                    let mut len_statement = statement.clone();
                    let mut place = Place::from(local);
                    if add_deref {
                        place = self.tcx.mk_place_deref(place);
                    }
                    len_statement.kind = StatementKind::Assign(box(*into, Rvalue::Len(place)));
                    statements.push(len_statement);

                    // make temporary dead
                    let mut make_dead_statement = statement.clone();
                    make_dead_statement.kind = StatementKind::StorageDead(local);
                    statements.push(make_dead_statement);

                    // make original statement NOP
                    statement.make_nop();
                },
                _ => {unreachable!("it's a bug in the implementation")}
            }

            self.statement_idx += 1;

            Some(statements.into_iter())
        } else {
            self.statement_idx += 1;
            None
        }
    }
}

fn normalize_array_len_call<'tcx>(
    tcx: TyCtxt<'tcx>,
    block: &mut BasicBlockData<'tcx>,
    local_decls: &mut IndexVec<Local, LocalDecl<'tcx>>,
    state: &mut IndexVec<Local, Option<usize>>,
    patches_scratchpad: &mut FxIndexMap<usize, usize>,
    replacements_scratchpad: &mut FxIndexMap<usize, Local>
) {
    for (statement_idx, statement) in block.statements.iter_mut().enumerate() {
        match &mut statement.kind {
            StatementKind::Assign(box (place, rvalue)) => {
                match rvalue {
                    Rvalue::Cast(CastKind::Pointer(ty::adjustment::PointerCast::Unsize), operand, cast_ty) => {
                        let local = if let Some(local) = place.as_local() {local} else {return};
                        match operand {
                            Operand::Copy(place) | Operand::Move(place) => {
                                let operand_local = if let Some(local) = place.local_or_deref_local() {
                                    local
                                } else {
                                    return
                                };
                                let operand_ty = local_decls[operand_local].ty;
                                match (operand_ty.kind(), cast_ty.kind()) {
                                    (ty::Array(of_ty_src, ..), ty::Slice(of_ty_dst)) => {
                                        if of_ty_src == of_ty_dst {
                                            // this is a cast from [T; N] into [T], so we are good
                                            state[local] = Some(statement_idx);
                                        }
                                    },
                                    // current way of patching doesn't allow to work with `mut`
                                    (ty::Ref(ty::RegionKind::ReErased, operand_ty, Mutability::Not), ty::Ref(ty::RegionKind::ReErased, cast_ty, Mutability::Not)) => {
                                        match (operand_ty.kind(), cast_ty.kind()) {
                                            // current way of patching doesn't allow to work with `mut`
                                            (ty::Array(of_ty_src, ..), ty::Slice(of_ty_dst)) => {
                                                if of_ty_src == of_ty_dst {
                                                    // this is a cast from [T; N] into [T], so we are good
                                                    state[local] = Some(statement_idx);
                                                }
                                            },
                                            _ => {}
                                        }
                                    },
                                    _ => {},
                                }
                            },
                            _ => {}
                        }
                    },
                    Rvalue::Len(place) => {
                        let local = if let Some(local) = place.local_or_deref_local() {
                            local
                        } else {
                            return
                        };
                        if let Some(cast_statement_idx) = state[local] {
                            patches_scratchpad.insert(cast_statement_idx, statement_idx);
                        }
                    },
                    _ => {
                        // invalidate
                        state[place.local] = None;
                    },
                }
            },
            _ => {}
        }
    }

    let mut patcher = Patcher {
        tcx,
        patches_scratchpad: &*patches_scratchpad,
        replacements_scratchpad,
        local_decls,
        statement_idx: 0,
    };

    block.expand_statements(|st| {
        patcher.patch_expand_statement(st)
    });
}
