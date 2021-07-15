use crate::transform::MirPass;
use rustc_data_structures::stable_map::FxHashMap;
use rustc_middle::mir::interpret::Allocation;
use rustc_middle::mir::*;
use rustc_middle::ty::util::IntTypeExt;
use rustc_middle::ty::{self, AdtDef, Const, ParamEnv, Ty, TyCtxt};
use rustc_target::abi::{HasDataLayout, Size, TagEncoding, Variants};
use std::array::IntoIter;

/// A pass that seeks to optimize unnecessary moves of large enum types, if there is a large
/// enough discrepanc between them
pub struct EnumSizeOpt {
    pub(crate) discrepancy: u64,
}

impl<'tcx> MirPass<'tcx> for EnumSizeOpt {
    fn run_pass(&self, tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
        self.optim(tcx, body);
    }
}

impl EnumSizeOpt {
    fn candidate<'tcx>(
        &self,
        tcx: TyCtxt<'tcx>,
        param_env: ParamEnv<'tcx>,
        ty: Ty<'tcx>,
        alloc_cache: &mut FxHashMap<Ty<'tcx>, &'tcx Allocation<(), ()>>,
    ) -> Option<(&'tcx AdtDef, usize, &'tcx Allocation<(), ()>)> {
        let adt_def = match ty.kind() {
            ty::Adt(adt_def, _substs) if adt_def.is_enum() => adt_def,
            _ => return None,
        };
        let layout = if let Ok(layout) = tcx.layout_of(param_env.and(ty)) {
            layout
        } else {
            return None;
        };
        let variants = match &layout.variants {
            Variants::Single { .. } => return None,
            Variants::Multiple { tag_encoding, .. }
                if matches!(tag_encoding, TagEncoding::Niche { .. }) =>
            {
                return None;
            }
            Variants::Multiple { variants, .. } if variants.len() <= 1 => return None,
            Variants::Multiple { variants, .. } => variants,
        };
        let min = variants.iter().map(|v| v.size).min().unwrap();
        let max = variants.iter().map(|v| v.size).max().unwrap();
        if max.bytes() - min.bytes() < self.discrepancy {
            return None;
        }

        let num_discrs = adt_def.discriminants(tcx).count();
        if variants.iter_enumerated().any(|(var_idx, _)| {
            let discr_for_var = adt_def.discriminant_for_variant(tcx, var_idx).val;
            (discr_for_var > usize::MAX as u128) || (discr_for_var as usize >= num_discrs)
        }) {
            return None;
        }
        if let Some(alloc) = alloc_cache.get(ty) {
            return Some((adt_def, num_discrs, alloc));
        }

        let data_layout = tcx.data_layout();
        let ptr_sized_int = data_layout.ptr_sized_integer();
        let target_bytes = ptr_sized_int.size().bytes() as usize;
        let mut data = vec![0; target_bytes * num_discrs];
        macro_rules! encode_store {
            ($curr_idx: expr, $endian: expr, $bytes: expr) => {
                let bytes = match $endian {
                    rustc_target::abi::Endian::Little => $bytes.to_le_bytes(),
                    rustc_target::abi::Endian::Big => $bytes.to_be_bytes(),
                };
                for (i, b) in IntoIter::new(bytes).enumerate() {
                    data[$curr_idx + i] = b;
                }
            };
        }

        for (var_idx, layout) in variants.iter_enumerated() {
            let curr_idx =
                target_bytes * adt_def.discriminant_for_variant(tcx, var_idx).val as usize;
            let sz = layout.size;
            match ptr_sized_int {
                rustc_target::abi::Integer::I32 => {
                    encode_store!(curr_idx, data_layout.endian, sz.bytes() as u32);
                }
                rustc_target::abi::Integer::I64 => {
                    encode_store!(curr_idx, data_layout.endian, sz.bytes());
                }
                _ => unreachable!(),
            };
        }
        let alloc = interpret::Allocation::from_bytes(
            data,
            tcx.data_layout.ptr_sized_integer().align(&tcx.data_layout).abi,
            Mutability::Not,
        );
        let alloc = tcx.intern_const_alloc(alloc);
        Some((adt_def, num_discrs, alloc_cache.entry(ty).insert(alloc).get()))
    }
    fn optim(&self, tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
        let mut alloc_cache = FxHashMap::default();
        let body_did = body.source.def_id();
        let param_env = tcx.param_env(body_did);
        let (bbs, local_decls) = body.basic_blocks_and_local_decls_mut();
        for bb in bbs {
            bb.expand_statements(|st| match &st.kind {
                StatementKind::Assign(box (
                    lhs,
                    Rvalue::Use(Operand::Copy(rhs) | Operand::Move(rhs)),
                )) => {
                    let ty = lhs.ty(local_decls, tcx).ty;

                    let source_info = st.source_info;
                    let span = source_info.span;

                    let (adt_def, num_variants, alloc) =
                        self.candidate(tcx, param_env, ty, &mut alloc_cache)?;

                    let tmp_ty = tcx.mk_ty(ty::Array(
                        tcx.types.usize,
                        Const::from_usize(tcx, num_variants as u64),
                    ));

                    let size_array_local = local_decls.push(LocalDecl::new(tmp_ty, span));
                    let store_live = Statement {
                        source_info,
                        kind: StatementKind::StorageLive(size_array_local),
                    };

                    let place = Place::from(size_array_local);
                    let constant_vals = Constant {
                        span,
                        user_ty: None,
                        literal: ConstantKind::Val(
                            interpret::ConstValue::ByRef { alloc, offset: Size::ZERO },
                            tmp_ty,
                        ),
                    };
                    let rval = Rvalue::Use(Operand::Constant(box (constant_vals)));

                    let const_assign =
                        Statement { source_info, kind: StatementKind::Assign(box (place, rval)) };

                    let discr_place = Place::from(
                        local_decls
                            .push(LocalDecl::new(adt_def.repr.discr_type().to_ty(tcx), span)),
                    );

                    let store_discr = Statement {
                        source_info,
                        kind: StatementKind::Assign(box (discr_place, Rvalue::Discriminant(*rhs))),
                    };

                    let discr_cast_place =
                        Place::from(local_decls.push(LocalDecl::new(tcx.types.usize, span)));

                    let cast_discr = Statement {
                        source_info,
                        kind: StatementKind::Assign(box (
                            discr_cast_place,
                            Rvalue::Cast(
                                CastKind::Misc,
                                Operand::Copy(discr_place),
                                tcx.types.usize,
                            ),
                        )),
                    };

                    let size_place =
                        Place::from(local_decls.push(LocalDecl::new(tcx.types.usize, span)));

                    let store_size = Statement {
                        source_info,
                        kind: StatementKind::Assign(box (
                            size_place,
                            Rvalue::Use(Operand::Copy(Place {
                                local: size_array_local,
                                projection: tcx.intern_place_elems(&[PlaceElem::Index(
                                    discr_cast_place.local,
                                )]),
                            })),
                        )),
                    };

                    let dst =
                        Place::from(local_decls.push(LocalDecl::new(tcx.mk_mut_ptr(ty), span)));

                    let dst_ptr = Statement {
                        source_info,
                        kind: StatementKind::Assign(box (
                            dst,
                            Rvalue::AddressOf(Mutability::Mut, *lhs),
                        )),
                    };

                    let dst_cast_ty = tcx.mk_mut_ptr(tcx.types.u8);
                    let dst_cast_place =
                        Place::from(local_decls.push(LocalDecl::new(dst_cast_ty, span)));

                    let dst_cast = Statement {
                        source_info,
                        kind: StatementKind::Assign(box (
                            dst_cast_place,
                            Rvalue::Cast(CastKind::Misc, Operand::Copy(dst), dst_cast_ty),
                        )),
                    };

                    let src =
                        Place::from(local_decls.push(LocalDecl::new(tcx.mk_imm_ptr(ty), span)));

                    let src_ptr = Statement {
                        source_info,
                        kind: StatementKind::Assign(box (
                            src,
                            Rvalue::AddressOf(Mutability::Not, *rhs),
                        )),
                    };

                    let src_cast_ty = tcx.mk_imm_ptr(tcx.types.u8);
                    let src_cast_place =
                        Place::from(local_decls.push(LocalDecl::new(src_cast_ty, span)));

                    let src_cast = Statement {
                        source_info,
                        kind: StatementKind::Assign(box (
                            src_cast_place,
                            Rvalue::Cast(CastKind::Misc, Operand::Copy(src), src_cast_ty),
                        )),
                    };

                    let copy_bytes = Statement {
                        source_info,
                        kind: StatementKind::CopyNonOverlapping(box CopyNonOverlapping {
                            src: Operand::Copy(src_cast_place),
                            dst: Operand::Copy(dst_cast_place),
                            count: Operand::Copy(size_place),
                        }),
                    };

                    let store_dead = Statement {
                        source_info,
                        kind: StatementKind::StorageDead(size_array_local),
                    };
                    let iter = IntoIter::new([
                        store_live,
                        const_assign,
                        store_discr,
                        cast_discr,
                        store_size,
                        dst_ptr,
                        dst_cast,
                        src_ptr,
                        src_cast,
                        copy_bytes,
                        store_dead,
                    ]);

                    st.make_nop();
                    Some(iter)
                }
                _ => return None,
            });
        }
    }
}
