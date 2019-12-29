use rustc::traits;
use rustc::ty::adjustment::CustomCoerceUnsized;
use rustc::ty::{self, Ty, TyCtxt};

pub mod collector;
pub mod partitioning;

pub fn custom_coerce_unsize_info<'tcx>(
    tcx: TyCtxt<'tcx>,
    source_ty: Ty<'tcx>,
    target_ty: Ty<'tcx>,
) -> CustomCoerceUnsized {
    let def_id = tcx.lang_items().coerce_unsized_trait().unwrap();

    let trait_ref =
        ty::TraitRef { def_id, substs: tcx.mk_substs_trait(source_ty, &[target_ty.into()]) };

    let impl_def_id = tcx.infer_ctxt().enter(|ref infcx| {
        match infcx.resolve_vtable(ty::ParamEnv::reveal_all(), trait_ref).unwrap() {
            traits::VtableImpl(traits::VtableImplData { impl_def_id, .. }) => impl_def_id,
            vtable => {
                bug!("invalid `CoerceUnsized` vtable: {:?}", vtable);
            }
        }
    });

    tcx.coerce_unsized_info(impl_def_id).custom_kind.unwrap()
}
