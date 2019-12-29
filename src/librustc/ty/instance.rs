use crate::infer::InferCtxt;
use crate::middle::codegen_fn_attrs::CodegenFnAttrFlags;
use crate::middle::lang_items::DropInPlaceFnLangItem;
use crate::traits;
use crate::ty::print::{FmtPrinter, Printer};
use crate::ty::{self, ParamEnv, SubstsRef, Ty, TyCtxt, TypeFoldable};
use rustc_hir::def::Namespace;
use rustc_hir::def_id::DefId;
use rustc_macros::HashStable;
use rustc_target::spec::abi::Abi;

use std::fmt;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, RustcEncodable, RustcDecodable)]
#[derive(HashStable, Lift)]
pub struct Instance<'tcx> {
    pub def: InstanceDef<'tcx>,
    pub substs: SubstsRef<'tcx>,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, RustcEncodable, RustcDecodable, HashStable)]
pub enum InstanceDef<'tcx> {
    Item(DefId),
    Intrinsic(DefId),

    /// `<T as Trait>::method` where `method` receives unsizeable `self: Self`.
    VtableShim(DefId),

    /// `fn()` pointer where the function itself cannot be turned into a pointer.
    ///
    /// One example is `<dyn Trait as Trait>::fn`, where the shim contains
    /// a virtual call, which codegen supports only via a direct call to the
    /// `<dyn Trait as Trait>::fn` instance (an `InstanceDef::Virtual`).
    ///
    /// Another example is functions annotated with `#[track_caller]`, which
    /// must have their implicit caller location argument populated for a call.
    /// Because this is a required part of the function's ABI but can't be tracked
    /// as a property of the function pointer, we use a single "caller location"
    /// (the definition of the function itself).
    ReifyShim(DefId),

    /// `<fn() as FnTrait>::call_*`
    /// `DefId` is `FnTrait::call_*`.
    FnPtrShim(DefId, Ty<'tcx>),

    /// `<dyn Trait as Trait>::fn`, "direct calls" of which are implicitly
    /// codegen'd as virtual calls.
    ///
    /// NB: if this is reified to a `fn` pointer, a `ReifyShim` is used
    /// (see `ReifyShim` above for more details on that).
    Virtual(DefId, usize),

    /// `<[mut closure] as FnOnce>::call_once`
    ClosureOnceShim {
        call_once: DefId,
    },

    /// `drop_in_place::<T>; None` for empty drop glue.
    DropGlue(DefId, Option<Ty<'tcx>>),

    ///`<T as Clone>::clone` shim.
    CloneShim(DefId, Ty<'tcx>),
}

impl<'tcx> Instance<'tcx> {
    /// Returns the `Ty` corresponding to this `Instance`,
    /// with generic substitutions applied and lifetimes erased.
    ///
    /// This method can only be called when the 'substs' for this Instance
    /// are fully monomorphic (no `ty::Param`'s are present).
    /// This is usually the case (e.g. during codegen).
    /// However, during constant evaluation, we may want
    /// to try to resolve a `Instance` using generic parameters
    /// (e.g. when we are attempting to to do const-propagation).
    /// In this case, `Instance.ty_env` should be used to provide
    /// the `ParamEnv` for our generic context.
    pub fn monomorphic_ty(&self, tcx: TyCtxt<'tcx>) -> Ty<'tcx> {
        let ty = tcx.type_of(self.def.def_id());
        // There shouldn't be any params - if there are, then
        // Instance.ty_env should have been used to provide the proper
        // ParamEnv
        if self.substs.has_param_types() {
            bug!("Instance.ty called for type {:?} with params in substs: {:?}", ty, self.substs);
        }
        tcx.subst_and_normalize_erasing_regions(self.substs, ty::ParamEnv::reveal_all(), &ty)
    }

    /// Like `Instance.ty`, but allows a `ParamEnv` to be specified for use during
    /// normalization. This method is only really useful during constant evaluation,
    /// where we are dealing with potentially generic types.
    pub fn ty_env(&self, tcx: TyCtxt<'tcx>, param_env: ty::ParamEnv<'tcx>) -> Ty<'tcx> {
        let ty = tcx.type_of(self.def.def_id());
        tcx.subst_and_normalize_erasing_regions(self.substs, param_env, &ty)
    }
}

impl<'tcx> InstanceDef<'tcx> {
    #[inline]
    pub fn def_id(&self) -> DefId {
        match *self {
            InstanceDef::Item(def_id)
            | InstanceDef::VtableShim(def_id)
            | InstanceDef::ReifyShim(def_id)
            | InstanceDef::FnPtrShim(def_id, _)
            | InstanceDef::Virtual(def_id, _)
            | InstanceDef::Intrinsic(def_id)
            | InstanceDef::ClosureOnceShim { call_once: def_id }
            | InstanceDef::DropGlue(def_id, _)
            | InstanceDef::CloneShim(def_id, _) => def_id,
        }
    }

    #[inline]
    pub fn attrs(&self, tcx: TyCtxt<'tcx>) -> ty::Attributes<'tcx> {
        tcx.get_attrs(self.def_id())
    }

    pub fn is_inline(&self, tcx: TyCtxt<'tcx>) -> bool {
        use crate::hir::map::DefPathData;
        let def_id = match *self {
            ty::InstanceDef::Item(def_id) => def_id,
            ty::InstanceDef::DropGlue(_, Some(_)) => return false,
            _ => return true,
        };
        match tcx.def_key(def_id).disambiguated_data.data {
            DefPathData::Ctor | DefPathData::ClosureExpr => true,
            _ => false,
        }
    }

    pub fn requires_local(&self, tcx: TyCtxt<'tcx>) -> bool {
        if self.is_inline(tcx) {
            return true;
        }
        if let ty::InstanceDef::DropGlue(..) = *self {
            // Drop glue wants to be instantiated at every codegen
            // unit, but without an #[inline] hint. We should make this
            // available to normal end-users.
            return true;
        }
        tcx.codegen_fn_attrs(self.def_id()).requests_inline()
    }

    pub fn requires_caller_location(&self, tcx: TyCtxt<'_>) -> bool {
        tcx.codegen_fn_attrs(self.def_id()).flags.contains(CodegenFnAttrFlags::TRACK_CALLER)
    }
}

impl<'tcx> fmt::Display for Instance<'tcx> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        ty::tls::with(|tcx| {
            let substs = tcx.lift(&self.substs).expect("could not lift for printing");
            FmtPrinter::new(tcx, &mut *f, Namespace::ValueNS)
                .print_def_path(self.def_id(), substs)?;
            Ok(())
        })?;

        match self.def {
            InstanceDef::Item(_) => Ok(()),
            InstanceDef::VtableShim(_) => write!(f, " - shim(vtable)"),
            InstanceDef::ReifyShim(_) => write!(f, " - shim(reify)"),
            InstanceDef::Intrinsic(_) => write!(f, " - intrinsic"),
            InstanceDef::Virtual(_, num) => write!(f, " - virtual#{}", num),
            InstanceDef::FnPtrShim(_, ty) => write!(f, " - shim({:?})", ty),
            InstanceDef::ClosureOnceShim { .. } => write!(f, " - shim"),
            InstanceDef::DropGlue(_, ty) => write!(f, " - shim({:?})", ty),
            InstanceDef::CloneShim(_, ty) => write!(f, " - shim({:?})", ty),
        }
    }
}

impl<'tcx> Instance<'tcx> {
    pub fn new(def_id: DefId, substs: SubstsRef<'tcx>) -> Instance<'tcx> {
        assert!(
            !substs.has_escaping_bound_vars(),
            "substs of instance {:?} not normalized for codegen: {:?}",
            def_id,
            substs
        );
        Instance { def: InstanceDef::Item(def_id), substs: substs }
    }

    pub fn mono(tcx: TyCtxt<'tcx>, def_id: DefId) -> Instance<'tcx> {
        Instance::new(def_id, tcx.empty_substs_for_def_id(def_id))
    }

    #[inline]
    pub fn def_id(&self) -> DefId {
        self.def.def_id()
    }

    /// Resolves a `(def_id, substs)` pair to an (optional) instance -- most commonly,
    /// this is used to find the precise code that will run for a trait method invocation,
    /// if known.
    ///
    /// Returns `None` if we cannot resolve `Instance` to a specific instance.
    /// For example, in a context like this,
    ///
    /// ```
    /// fn foo<T: Debug>(t: T) { ... }
    /// ```
    ///
    /// trying to resolve `Debug::fmt` applied to `T` will yield `None`, because we do not
    /// know what code ought to run. (Note that this setting is also affected by the
    /// `RevealMode` in the parameter environment.)
    ///
    /// Presuming that coherence and type-check have succeeded, if this method is invoked
    /// in a monomorphic context (i.e., like during codegen), then it is guaranteed to return
    /// `Some`.
    pub fn resolve<'infcx>(
        infcx: &'infcx InferCtxt<'infcx, 'tcx>,
        param_env: ty::ParamEnv<'tcx>,
        def_id: DefId,
        substs: SubstsRef<'tcx>,
    ) -> Option<Instance<'tcx>> {
        let tcx = infcx.tcx;
        debug!("resolve(def_id={:?}, substs={:?})", def_id, substs);
        let result = if let Some(trait_def_id) = tcx.trait_of_item(def_id) {
            debug!(" => associated item, attempting to find impl in param_env {:#?}", param_env);
            let item = tcx.associated_item(def_id);
            resolve_associated_item(infcx, &item, param_env, trait_def_id, substs)
        } else {
            let ty = tcx.type_of(def_id);
            let item_type = tcx.subst_and_normalize_erasing_regions(substs, param_env, &ty);

            let def = match item_type.kind {
                ty::FnDef(..)
                    if {
                        let f = item_type.fn_sig(tcx);
                        f.abi() == Abi::RustIntrinsic || f.abi() == Abi::PlatformIntrinsic
                    } =>
                {
                    debug!(" => intrinsic");
                    ty::InstanceDef::Intrinsic(def_id)
                }
                _ => {
                    if Some(def_id) == tcx.lang_items().drop_in_place_fn() {
                        let ty = substs.type_at(0);
                        if ty.needs_drop(tcx, ty::ParamEnv::reveal_all()) {
                            debug!(" => nontrivial drop glue");
                            ty::InstanceDef::DropGlue(def_id, Some(ty))
                        } else {
                            debug!(" => trivial drop glue");
                            ty::InstanceDef::DropGlue(def_id, None)
                        }
                    } else {
                        debug!(" => free item");
                        ty::InstanceDef::Item(def_id)
                    }
                }
            };
            Some(Instance { def: def, substs: substs })
        };
        debug!("resolve(def_id={:?}, substs={:?}) = {:?}", def_id, substs, result);
        result
    }

    pub fn resolve_mono(
        tcx: TyCtxt<'tcx>,
        def_id: DefId,
        substs: SubstsRef<'tcx>,
    ) -> Instance<'tcx> {
        tcx.infer_ctxt().enter(|ref infcx| {
            Instance::resolve(infcx, ParamEnv::reveal_all(), def_id, substs).unwrap()
        })
    }

    pub fn resolve_for_fn_ptr<'infcx>(
        infcx: &'infcx InferCtxt<'infcx, 'tcx>,
        param_env: ty::ParamEnv<'tcx>,
        def_id: DefId,
        substs: SubstsRef<'tcx>,
    ) -> Option<Instance<'tcx>> {
        debug!("resolve(def_id={:?}, substs={:?})", def_id, substs);
        Instance::resolve(infcx, param_env, def_id, substs).map(|mut resolved| {
            match resolved.def {
                InstanceDef::Item(def_id) if resolved.def.requires_caller_location(infcx.tcx) => {
                    debug!(" => fn pointer created for function with #[track_caller]");
                    resolved.def = InstanceDef::ReifyShim(def_id);
                }
                InstanceDef::Virtual(def_id, _) => {
                    debug!(" => fn pointer created for virtual call");
                    resolved.def = InstanceDef::ReifyShim(def_id);
                }
                _ => {}
            }

            resolved
        })
    }

    pub fn resolve_for_fn_ptr_mono(
        tcx: TyCtxt<'tcx>,
        def_id: DefId,
        substs: SubstsRef<'tcx>,
    ) -> Instance<'tcx> {
        tcx.infer_ctxt().enter(|ref infcx| {
            Instance::resolve_for_fn_ptr(infcx, ParamEnv::reveal_all(), def_id, substs).unwrap()
        })
    }

    pub fn resolve_for_vtable<'infcx>(
        infcx: &'infcx InferCtxt<'infcx, 'tcx>,
        param_env: ty::ParamEnv<'tcx>,
        def_id: DefId,
        substs: SubstsRef<'tcx>,
    ) -> Option<Instance<'tcx>> {
        debug!("resolve(def_id={:?}, substs={:?})", def_id, substs);
        let tcx = infcx.tcx;
        let fn_sig = tcx.fn_sig(def_id);
        let is_vtable_shim = fn_sig.inputs().skip_binder().len() > 0
            && fn_sig.input(0).skip_binder().is_param(0)
            && tcx.generics_of(def_id).has_self;
        if is_vtable_shim {
            debug!(" => associated item with unsizeable self: Self");
            Some(Instance { def: InstanceDef::VtableShim(def_id), substs })
        } else {
            Instance::resolve(infcx, param_env, def_id, substs)
        }
    }

    pub fn resolve_for_vtable_mono(
        tcx: TyCtxt<'tcx>,
        def_id: DefId,
        substs: SubstsRef<'tcx>,
    ) -> Instance<'tcx> {
        tcx.infer_ctxt().enter(|ref infcx| {
            Instance::resolve_for_vtable(infcx, ParamEnv::reveal_all(), def_id, substs).unwrap()
        })
    }

    pub fn resolve_closure(
        tcx: TyCtxt<'tcx>,
        def_id: DefId,
        substs: ty::SubstsRef<'tcx>,
        requested_kind: ty::ClosureKind,
    ) -> Instance<'tcx> {
        let actual_kind = substs.as_closure().kind(def_id, tcx);

        match needs_fn_once_adapter_shim(actual_kind, requested_kind) {
            Ok(true) => Instance::fn_once_adapter_instance(tcx, def_id, substs),
            _ => Instance::new(def_id, substs),
        }
    }

    pub fn resolve_drop_in_place(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> ty::Instance<'tcx> {
        let def_id = tcx.require_lang_item(DropInPlaceFnLangItem, None);
        let substs = tcx.intern_substs(&[ty.into()]);
        Instance::resolve_mono(tcx, def_id, substs)
    }

    pub fn fn_once_adapter_instance(
        tcx: TyCtxt<'tcx>,
        closure_did: DefId,
        substs: ty::SubstsRef<'tcx>,
    ) -> Instance<'tcx> {
        debug!("fn_once_adapter_shim({:?}, {:?})", closure_did, substs);
        let fn_once = tcx.lang_items().fn_once_trait().unwrap();
        let call_once = tcx
            .associated_items(fn_once)
            .find(|it| it.kind == ty::AssocKind::Method)
            .unwrap()
            .def_id;
        let def = ty::InstanceDef::ClosureOnceShim { call_once };

        let self_ty = tcx.mk_closure(closure_did, substs);

        let sig = substs.as_closure().sig(closure_did, tcx);
        let sig = tcx.normalize_erasing_late_bound_regions(ty::ParamEnv::reveal_all(), &sig);
        assert_eq!(sig.inputs().len(), 1);
        let substs = tcx.mk_substs_trait(self_ty, &[sig.inputs()[0].into()]);

        debug!("fn_once_adapter_shim: self_ty={:?} sig={:?}", self_ty, sig);
        Instance { def, substs }
    }

    pub fn is_vtable_shim(&self) -> bool {
        if let InstanceDef::VtableShim(..) = self.def { true } else { false }
    }
}

fn resolve_associated_item<'infcx, 'tcx>(
    infcx: &'infcx InferCtxt<'infcx, 'tcx>,
    trait_item: &ty::AssocItem,
    param_env: ty::ParamEnv<'tcx>,
    trait_id: DefId,
    rcvr_substs: SubstsRef<'tcx>,
) -> Option<Instance<'tcx>> {
    let tcx = infcx.tcx;
    let def_id = trait_item.def_id;
    debug!(
        "resolve_associated_item(trait_item={:?}, \
            param_env={:?}, \
            trait_id={:?}, \
            rcvr_substs={:?})",
        def_id, param_env, trait_id, rcvr_substs
    );

    let trait_ref = ty::TraitRef::from_method(tcx, trait_id, rcvr_substs);

    let vtbl = infcx.resolve_vtable(param_env, trait_ref)?;

    // Now that we know which impl is being used, we can dispatch to
    // the actual function:
    match vtbl {
        traits::VtableImpl(impl_data) => {
            let (def_id, substs) =
                traits::find_associated_item(tcx, param_env, trait_item, rcvr_substs, &impl_data);

            let resolved_item = tcx.associated_item(def_id);

            // Since this is a trait item, we need to see if the item is either a trait default item
            // or a specialization because we can't resolve those unless we can `Reveal::All`.
            // NOTE: This should be kept in sync with the similar code in
            // `rustc::traits::project::assemble_candidates_from_impls()`.
            let eligible = if !resolved_item.defaultness.is_default() {
                true
            } else if param_env.reveal == traits::Reveal::All {
                !trait_ref.needs_subst()
            } else {
                false
            };

            if !eligible {
                return None;
            }

            let substs = tcx.erase_regions(&substs);
            Some(ty::Instance::new(def_id, substs))
        }
        traits::VtableGenerator(generator_data) => Some(Instance {
            def: ty::InstanceDef::Item(generator_data.generator_def_id),
            substs: generator_data.substs,
        }),
        traits::VtableClosure(closure_data) => {
            let trait_closure_kind = tcx.lang_items().fn_trait_kind(trait_id).unwrap();
            Some(Instance::resolve_closure(
                tcx,
                closure_data.closure_def_id,
                closure_data.substs,
                trait_closure_kind,
            ))
        }
        traits::VtableFnPointer(ref data) => Some(Instance {
            def: ty::InstanceDef::FnPtrShim(trait_item.def_id, data.fn_ty),
            substs: rcvr_substs,
        }),
        traits::VtableObject(ref data) => {
            let index = tcx.get_vtable_index_of_object_method(data, def_id);
            Some(Instance { def: ty::InstanceDef::Virtual(def_id, index), substs: rcvr_substs })
        }
        traits::VtableBuiltin(..) => {
            if tcx.lang_items().clone_trait().is_some() {
                Some(Instance {
                    def: ty::InstanceDef::CloneShim(def_id, trait_ref.self_ty()),
                    substs: rcvr_substs,
                })
            } else {
                None
            }
        }
        traits::VtableAutoImpl(..) | traits::VtableParam(..) | traits::VtableTraitAlias(..) => None,
    }
}

fn needs_fn_once_adapter_shim(
    actual_closure_kind: ty::ClosureKind,
    trait_closure_kind: ty::ClosureKind,
) -> Result<bool, ()> {
    match (actual_closure_kind, trait_closure_kind) {
        (ty::ClosureKind::Fn, ty::ClosureKind::Fn)
        | (ty::ClosureKind::FnMut, ty::ClosureKind::FnMut)
        | (ty::ClosureKind::FnOnce, ty::ClosureKind::FnOnce) => {
            // No adapter needed.
            Ok(false)
        }
        (ty::ClosureKind::Fn, ty::ClosureKind::FnMut) => {
            // The closure fn `llfn` is a `fn(&self, ...)`.  We want a
            // `fn(&mut self, ...)`. In fact, at codegen time, these are
            // basically the same thing, so we can just return llfn.
            Ok(false)
        }
        (ty::ClosureKind::Fn, ty::ClosureKind::FnOnce)
        | (ty::ClosureKind::FnMut, ty::ClosureKind::FnOnce) => {
            // The closure fn `llfn` is a `fn(&self, ...)` or `fn(&mut
            // self, ...)`.  We want a `fn(self, ...)`. We can produce
            // this by doing something like:
            //
            //     fn call_once(self, ...) { call_mut(&self, ...) }
            //     fn call_once(mut self, ...) { call_mut(&mut self, ...) }
            //
            // These are both the same at codegen time.
            Ok(true)
        }
        (ty::ClosureKind::FnMut, _) | (ty::ClosureKind::FnOnce, _) => Err(()),
    }
}
