//! Miscellaneous type-system utilities that are too small to deserve their own modules.

use crate::ich::NodeIdHashingMode;
use crate::middle::codegen_fn_attrs::CodegenFnAttrFlags;
use crate::mir::interpret::{sign_extend, truncate};
use crate::ty::layout::IntegerExt;
use crate::ty::query::TyCtxtAt;
use crate::ty::subst::{GenericArgKind, InternalSubsts, Subst, SubstsRef};
use crate::ty::TyKind::*;
use crate::ty::{self, DefIdTree, GenericParamDefKind, Ty, TyCtxt, TypeFoldable};
use rustc_apfloat::Float as _;
use rustc_ast::ast;
use rustc_attr::{self as attr, SignedInt, UnsignedInt};
use rustc_data_structures::fx::{FxHashMap, FxHashSet};
use rustc_data_structures::stable_hasher::{HashStable, StableHasher};
use rustc_errors::ErrorReported;
use rustc_hir as hir;
use rustc_hir::def::DefKind;
use rustc_hir::def_id::DefId;
use rustc_macros::HashStable;
use rustc_span::Span;
use rustc_target::abi::{Integer, Size, TargetDataLayout};
use smallvec::SmallVec;
use std::{cmp, fmt};

#[derive(Copy, Clone, Debug)]
pub struct Discr<'tcx> {
    /// Bit representation of the discriminant (e.g., `-128i8` is `0xFF_u128`).
    pub val: u128,
    pub ty: Ty<'tcx>,
}

impl<'tcx> fmt::Display for Discr<'tcx> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.ty.kind {
            ty::Int(ity) => {
                let size = ty::tls::with(|tcx| Integer::from_attr(&tcx, SignedInt(ity)).size());
                let x = self.val;
                // sign extend the raw representation to be an i128
                let x = sign_extend(x, size) as i128;
                write!(fmt, "{}", x)
            }
            _ => write!(fmt, "{}", self.val),
        }
    }
}

fn signed_min(size: Size) -> i128 {
    sign_extend(1_u128 << (size.bits() - 1), size) as i128
}

fn signed_max(size: Size) -> i128 {
    i128::MAX >> (128 - size.bits())
}

fn unsigned_max(size: Size) -> u128 {
    u128::MAX >> (128 - size.bits())
}

fn int_size_and_signed<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> (Size, bool) {
    let (int, signed) = match ty.kind {
        Int(ity) => (Integer::from_attr(&tcx, SignedInt(ity)), true),
        Uint(uty) => (Integer::from_attr(&tcx, UnsignedInt(uty)), false),
        _ => bug!("non integer discriminant"),
    };
    (int.size(), signed)
}

impl<'tcx> Discr<'tcx> {
    /// Adds `1` to the value and wraps around if the maximum for the type is reached.
    pub fn wrap_incr(self, tcx: TyCtxt<'tcx>) -> Self {
        self.checked_add(tcx, 1).0
    }
    pub fn checked_add(self, tcx: TyCtxt<'tcx>, n: u128) -> (Self, bool) {
        let (size, signed) = int_size_and_signed(tcx, self.ty);
        let (val, oflo) = if signed {
            let min = signed_min(size);
            let max = signed_max(size);
            let val = sign_extend(self.val, size) as i128;
            assert!(n < (i128::MAX as u128));
            let n = n as i128;
            let oflo = val > max - n;
            let val = if oflo { min + (n - (max - val) - 1) } else { val + n };
            // zero the upper bits
            let val = val as u128;
            let val = truncate(val, size);
            (val, oflo)
        } else {
            let max = unsigned_max(size);
            let val = self.val;
            let oflo = val > max - n;
            let val = if oflo { n - (max - val) - 1 } else { val + n };
            (val, oflo)
        };
        (Self { val, ty: self.ty }, oflo)
    }
}

pub trait IntTypeExt {
    fn to_ty<'tcx>(&self, tcx: TyCtxt<'tcx>) -> Ty<'tcx>;
    fn disr_incr<'tcx>(&self, tcx: TyCtxt<'tcx>, val: Option<Discr<'tcx>>) -> Option<Discr<'tcx>>;
    fn initial_discriminant<'tcx>(&self, tcx: TyCtxt<'tcx>) -> Discr<'tcx>;
}

impl IntTypeExt for attr::IntType {
    fn to_ty<'tcx>(&self, tcx: TyCtxt<'tcx>) -> Ty<'tcx> {
        match *self {
            SignedInt(ast::IntTy::I8) => tcx.types.i8,
            SignedInt(ast::IntTy::I16) => tcx.types.i16,
            SignedInt(ast::IntTy::I32) => tcx.types.i32,
            SignedInt(ast::IntTy::I64) => tcx.types.i64,
            SignedInt(ast::IntTy::I128) => tcx.types.i128,
            SignedInt(ast::IntTy::Isize) => tcx.types.isize,
            UnsignedInt(ast::UintTy::U8) => tcx.types.u8,
            UnsignedInt(ast::UintTy::U16) => tcx.types.u16,
            UnsignedInt(ast::UintTy::U32) => tcx.types.u32,
            UnsignedInt(ast::UintTy::U64) => tcx.types.u64,
            UnsignedInt(ast::UintTy::U128) => tcx.types.u128,
            UnsignedInt(ast::UintTy::Usize) => tcx.types.usize,
        }
    }

    fn initial_discriminant<'tcx>(&self, tcx: TyCtxt<'tcx>) -> Discr<'tcx> {
        Discr { val: 0, ty: self.to_ty(tcx) }
    }

    fn disr_incr<'tcx>(&self, tcx: TyCtxt<'tcx>, val: Option<Discr<'tcx>>) -> Option<Discr<'tcx>> {
        if let Some(val) = val {
            assert_eq!(self.to_ty(tcx), val.ty);
            let (new, oflo) = val.checked_add(tcx, 1);
            if oflo { None } else { Some(new) }
        } else {
            Some(self.initial_discriminant(tcx))
        }
    }
}

/// Describes whether a type is representable. For types that are not
/// representable, 'SelfRecursive' and 'ContainsRecursive' are used to
/// distinguish between types that are recursive with themselves and types that
/// contain a different recursive type. These cases can therefore be treated
/// differently when reporting errors.
///
/// The ordering of the cases is significant. They are sorted so that cmp::max
/// will keep the "more erroneous" of two values.
#[derive(Clone, PartialOrd, Ord, Eq, PartialEq, Debug)]
pub enum Representability {
    Representable,
    ContainsRecursive,
    SelfRecursive(Vec<Span>),
}

impl<'tcx> TyCtxt<'tcx> {
    /// Creates a hash of the type `Ty` which will be the same no matter what crate
    /// context it's calculated within. This is used by the `type_id` intrinsic.
    pub fn type_id_hash(self, ty: Ty<'tcx>) -> u64 {
        let mut hasher = StableHasher::new();
        let mut hcx = self.create_stable_hashing_context();

        // We want the type_id be independent of the types free regions, so we
        // erase them. The erase_regions() call will also anonymize bound
        // regions, which is desirable too.
        let ty = self.erase_regions(&ty);

        hcx.while_hashing_spans(false, |hcx| {
            hcx.with_node_id_hashing_mode(NodeIdHashingMode::HashDefPath, |hcx| {
                ty.hash_stable(hcx, &mut hasher);
            });
        });
        hasher.finish()
    }
}

impl<'tcx> TyCtxt<'tcx> {
    pub fn has_error_field(self, ty: Ty<'tcx>) -> bool {
        if let ty::Adt(def, substs) = ty.kind {
            for field in def.all_fields() {
                let field_ty = field.ty(self, substs);
                if let Error(_) = field_ty.kind {
                    return true;
                }
            }
        }
        false
    }

    /// Attempts to returns the deeply last field of nested structures, but
    /// does not apply any normalization in its search. Returns the same type
    /// if input `ty` is not a structure at all.
    pub fn struct_tail_without_normalization(self, ty: Ty<'tcx>) -> Ty<'tcx> {
        let tcx = self;
        tcx.struct_tail_with_normalize(ty, |ty| ty)
    }

    /// Returns the deeply last field of nested structures, or the same type if
    /// not a structure at all. Corresponds to the only possible unsized field,
    /// and its type can be used to determine unsizing strategy.
    ///
    /// Should only be called if `ty` has no inference variables and does not
    /// need its lifetimes preserved (e.g. as part of codegen); otherwise
    /// normalization attempt may cause compiler bugs.
    pub fn struct_tail_erasing_lifetimes(
        self,
        ty: Ty<'tcx>,
        param_env: ty::ParamEnv<'tcx>,
    ) -> Ty<'tcx> {
        let tcx = self;
        tcx.struct_tail_with_normalize(ty, |ty| tcx.normalize_erasing_regions(param_env, ty))
    }

    /// Returns the deeply last field of nested structures, or the same type if
    /// not a structure at all. Corresponds to the only possible unsized field,
    /// and its type can be used to determine unsizing strategy.
    ///
    /// This is parameterized over the normalization strategy (i.e. how to
    /// handle `<T as Trait>::Assoc` and `impl Trait`); pass the identity
    /// function to indicate no normalization should take place.
    ///
    /// See also `struct_tail_erasing_lifetimes`, which is suitable for use
    /// during codegen.
    pub fn struct_tail_with_normalize(
        self,
        mut ty: Ty<'tcx>,
        normalize: impl Fn(Ty<'tcx>) -> Ty<'tcx>,
    ) -> Ty<'tcx> {
        loop {
            match ty.kind {
                ty::Adt(def, substs) => {
                    if !def.is_struct() {
                        break;
                    }
                    match def.non_enum_variant().fields.last() {
                        Some(f) => ty = f.ty(self, substs),
                        None => break,
                    }
                }

                ty::Tuple(tys) => {
                    if let Some((&last_ty, _)) = tys.split_last() {
                        ty = last_ty.expect_ty();
                    } else {
                        break;
                    }
                }

                ty::Projection(_) | ty::Opaque(..) => {
                    let normalized = normalize(ty);
                    if ty == normalized {
                        return ty;
                    } else {
                        ty = normalized;
                    }
                }

                _ => {
                    break;
                }
            }
        }
        ty
    }

    /// Same as applying `struct_tail` on `source` and `target`, but only
    /// keeps going as long as the two types are instances of the same
    /// structure definitions.
    /// For `(Foo<Foo<T>>, Foo<dyn Trait>)`, the result will be `(Foo<T>, Trait)`,
    /// whereas struct_tail produces `T`, and `Trait`, respectively.
    ///
    /// Should only be called if the types have no inference variables and do
    /// not need their lifetimes preserved (e.g., as part of codegen); otherwise,
    /// normalization attempt may cause compiler bugs.
    pub fn struct_lockstep_tails_erasing_lifetimes(
        self,
        source: Ty<'tcx>,
        target: Ty<'tcx>,
        param_env: ty::ParamEnv<'tcx>,
    ) -> (Ty<'tcx>, Ty<'tcx>) {
        let tcx = self;
        tcx.struct_lockstep_tails_with_normalize(source, target, |ty| {
            tcx.normalize_erasing_regions(param_env, ty)
        })
    }

    /// Same as applying `struct_tail` on `source` and `target`, but only
    /// keeps going as long as the two types are instances of the same
    /// structure definitions.
    /// For `(Foo<Foo<T>>, Foo<dyn Trait>)`, the result will be `(Foo<T>, Trait)`,
    /// whereas struct_tail produces `T`, and `Trait`, respectively.
    ///
    /// See also `struct_lockstep_tails_erasing_lifetimes`, which is suitable for use
    /// during codegen.
    pub fn struct_lockstep_tails_with_normalize(
        self,
        source: Ty<'tcx>,
        target: Ty<'tcx>,
        normalize: impl Fn(Ty<'tcx>) -> Ty<'tcx>,
    ) -> (Ty<'tcx>, Ty<'tcx>) {
        let (mut a, mut b) = (source, target);
        loop {
            match (&a.kind, &b.kind) {
                (&Adt(a_def, a_substs), &Adt(b_def, b_substs))
                    if a_def == b_def && a_def.is_struct() =>
                {
                    if let Some(f) = a_def.non_enum_variant().fields.last() {
                        a = f.ty(self, a_substs);
                        b = f.ty(self, b_substs);
                    } else {
                        break;
                    }
                }
                (&Tuple(a_tys), &Tuple(b_tys)) if a_tys.len() == b_tys.len() => {
                    if let Some(a_last) = a_tys.last() {
                        a = a_last.expect_ty();
                        b = b_tys.last().unwrap().expect_ty();
                    } else {
                        break;
                    }
                }
                (ty::Projection(_) | ty::Opaque(..), _)
                | (_, ty::Projection(_) | ty::Opaque(..)) => {
                    // If either side is a projection, attempt to
                    // progress via normalization. (Should be safe to
                    // apply to both sides as normalization is
                    // idempotent.)
                    let a_norm = normalize(a);
                    let b_norm = normalize(b);
                    if a == a_norm && b == b_norm {
                        break;
                    } else {
                        a = a_norm;
                        b = b_norm;
                    }
                }

                _ => break,
            }
        }
        (a, b)
    }

    /// Calculate the destructor of a given type.
    pub fn calculate_dtor(
        self,
        adt_did: DefId,
        validate: &mut dyn FnMut(Self, DefId) -> Result<(), ErrorReported>,
    ) -> Option<ty::Destructor> {
        let drop_trait = self.lang_items().drop_trait()?;
        self.ensure().coherent_trait(drop_trait);

        let mut dtor_did = None;
        let ty = self.type_of(adt_did);
        self.for_each_relevant_impl(drop_trait, ty, |impl_did| {
            if let Some(item) = self.associated_items(impl_did).in_definition_order().next() {
                if validate(self, impl_did).is_ok() {
                    dtor_did = Some(item.def_id);
                }
            }
        });

        Some(ty::Destructor { did: dtor_did? })
    }

    /// Returns the set of types that are required to be alive in
    /// order to run the destructor of `def` (see RFCs 769 and
    /// 1238).
    ///
    /// Note that this returns only the constraints for the
    /// destructor of `def` itself. For the destructors of the
    /// contents, you need `adt_dtorck_constraint`.
    pub fn destructor_constraints(self, def: &'tcx ty::AdtDef) -> Vec<ty::subst::GenericArg<'tcx>> {
        let dtor = match def.destructor(self) {
            None => {
                debug!("destructor_constraints({:?}) - no dtor", def.did);
                return vec![];
            }
            Some(dtor) => dtor.did,
        };

        let impl_def_id = self.associated_item(dtor).container.id();
        let impl_generics = self.generics_of(impl_def_id);

        // We have a destructor - all the parameters that are not
        // pure_wrt_drop (i.e, don't have a #[may_dangle] attribute)
        // must be live.

        // We need to return the list of parameters from the ADTs
        // generics/substs that correspond to impure parameters on the
        // impl's generics. This is a bit ugly, but conceptually simple:
        //
        // Suppose our ADT looks like the following
        //
        //     struct S<X, Y, Z>(X, Y, Z);
        //
        // and the impl is
        //
        //     impl<#[may_dangle] P0, P1, P2> Drop for S<P1, P2, P0>
        //
        // We want to return the parameters (X, Y). For that, we match
        // up the item-substs <X, Y, Z> with the substs on the impl ADT,
        // <P1, P2, P0>, and then look up which of the impl substs refer to
        // parameters marked as pure.

        let impl_substs = match self.type_of(impl_def_id).kind {
            ty::Adt(def_, substs) if def_ == def => substs,
            _ => bug!(),
        };

        let item_substs = match self.type_of(def.did).kind {
            ty::Adt(def_, substs) if def_ == def => substs,
            _ => bug!(),
        };

        let result = item_substs
            .iter()
            .zip(impl_substs.iter())
            .filter(|&(_, k)| {
                match k.unpack() {
                    GenericArgKind::Lifetime(&ty::RegionKind::ReEarlyBound(ref ebr)) => {
                        !impl_generics.region_param(ebr, self).pure_wrt_drop
                    }
                    GenericArgKind::Type(&ty::TyS { kind: ty::Param(ref pt), .. }) => {
                        !impl_generics.type_param(pt, self).pure_wrt_drop
                    }
                    GenericArgKind::Const(&ty::Const {
                        val: ty::ConstKind::Param(ref pc), ..
                    }) => !impl_generics.const_param(pc, self).pure_wrt_drop,
                    GenericArgKind::Lifetime(_)
                    | GenericArgKind::Type(_)
                    | GenericArgKind::Const(_) => {
                        // Not a type, const or region param: this should be reported
                        // as an error.
                        false
                    }
                }
            })
            .map(|(item_param, _)| item_param)
            .collect();
        debug!("destructor_constraint({:?}) = {:?}", def.did, result);
        result
    }

    /// Returns `true` if `def_id` refers to a closure (e.g., `|x| x * 2`). Note
    /// that closures have a `DefId`, but the closure *expression* also
    /// has a `HirId` that is located within the context where the
    /// closure appears (and, sadly, a corresponding `NodeId`, since
    /// those are not yet phased out). The parent of the closure's
    /// `DefId` will also be the context where it appears.
    pub fn is_closure(self, def_id: DefId) -> bool {
        matches!(self.def_kind(def_id), DefKind::Closure | DefKind::Generator)
    }

    /// Returns `true` if `def_id` refers to a trait (i.e., `trait Foo { ... }`).
    pub fn is_trait(self, def_id: DefId) -> bool {
        self.def_kind(def_id) == DefKind::Trait
    }

    /// Returns `true` if `def_id` refers to a trait alias (i.e., `trait Foo = ...;`),
    /// and `false` otherwise.
    pub fn is_trait_alias(self, def_id: DefId) -> bool {
        self.def_kind(def_id) == DefKind::TraitAlias
    }

    /// Returns `true` if this `DefId` refers to the implicit constructor for
    /// a tuple struct like `struct Foo(u32)`, and `false` otherwise.
    pub fn is_constructor(self, def_id: DefId) -> bool {
        matches!(self.def_kind(def_id), DefKind::Ctor(..))
    }

    /// Given the def-ID of a fn or closure, returns the def-ID of
    /// the innermost fn item that the closure is contained within.
    /// This is a significant `DefId` because, when we do
    /// type-checking, we type-check this fn item and all of its
    /// (transitive) closures together. Therefore, when we fetch the
    /// `typeck` the closure, for example, we really wind up
    /// fetching the `typeck` the enclosing fn item.
    pub fn closure_base_def_id(self, def_id: DefId) -> DefId {
        let mut def_id = def_id;
        while self.is_closure(def_id) {
            def_id = self.parent(def_id).unwrap_or_else(|| {
                bug!("closure {:?} has no parent", def_id);
            });
        }
        def_id
    }

    /// Given the `DefId` and substs a closure, creates the type of
    /// `self` argument that the closure expects. For example, for a
    /// `Fn` closure, this would return a reference type `&T` where
    /// `T = closure_ty`.
    ///
    /// Returns `None` if this closure's kind has not yet been inferred.
    /// This should only be possible during type checking.
    ///
    /// Note that the return value is a late-bound region and hence
    /// wrapped in a binder.
    pub fn closure_env_ty(
        self,
        closure_def_id: DefId,
        closure_substs: SubstsRef<'tcx>,
    ) -> Option<ty::Binder<Ty<'tcx>>> {
        let closure_ty = self.mk_closure(closure_def_id, closure_substs);
        let env_region = ty::ReLateBound(ty::INNERMOST, ty::BrEnv);
        let closure_kind_ty = closure_substs.as_closure().kind_ty();
        let closure_kind = closure_kind_ty.to_opt_closure_kind()?;
        let env_ty = match closure_kind {
            ty::ClosureKind::Fn => self.mk_imm_ref(self.mk_region(env_region), closure_ty),
            ty::ClosureKind::FnMut => self.mk_mut_ref(self.mk_region(env_region), closure_ty),
            ty::ClosureKind::FnOnce => closure_ty,
        };
        Some(ty::Binder::bind(env_ty))
    }

    /// Given the `DefId` of some item that has no type or const parameters, make
    /// a suitable "empty substs" for it.
    pub fn empty_substs_for_def_id(self, item_def_id: DefId) -> SubstsRef<'tcx> {
        InternalSubsts::for_item(self, item_def_id, |param, _| match param.kind {
            GenericParamDefKind::Lifetime => self.lifetimes.re_erased.into(),
            GenericParamDefKind::Type { .. } => {
                bug!("empty_substs_for_def_id: {:?} has type parameters", item_def_id)
            }
            GenericParamDefKind::Const { .. } => {
                bug!("empty_substs_for_def_id: {:?} has const parameters", item_def_id)
            }
        })
    }

    /// Returns `true` if the node pointed to by `def_id` is a `static` item.
    pub fn is_static(&self, def_id: DefId) -> bool {
        self.static_mutability(def_id).is_some()
    }

    /// Returns `true` if this is a `static` item with the `#[thread_local]` attribute.
    pub fn is_thread_local_static(&self, def_id: DefId) -> bool {
        self.codegen_fn_attrs(def_id).flags.contains(CodegenFnAttrFlags::THREAD_LOCAL)
    }

    /// Returns `true` if the node pointed to by `def_id` is a mutable `static` item.
    pub fn is_mutable_static(&self, def_id: DefId) -> bool {
        self.static_mutability(def_id) == Some(hir::Mutability::Mut)
    }

    /// Get the type of the pointer to the static that we use in MIR.
    pub fn static_ptr_ty(&self, def_id: DefId) -> Ty<'tcx> {
        // Make sure that any constants in the static's type are evaluated.
        let static_ty = self.normalize_erasing_regions(ty::ParamEnv::empty(), self.type_of(def_id));

        if self.is_mutable_static(def_id) {
            self.mk_mut_ptr(static_ty)
        } else {
            self.mk_imm_ref(self.lifetimes.re_erased, static_ty)
        }
    }

    /// Expands the given impl trait type, stopping if the type is recursive.
    pub fn try_expand_impl_trait_type(
        self,
        def_id: DefId,
        substs: SubstsRef<'tcx>,
    ) -> Result<Ty<'tcx>, Ty<'tcx>> {
        use crate::ty::fold::TypeFolder;

        struct OpaqueTypeExpander<'tcx> {
            // Contains the DefIds of the opaque types that are currently being
            // expanded. When we expand an opaque type we insert the DefId of
            // that type, and when we finish expanding that type we remove the
            // its DefId.
            seen_opaque_tys: FxHashSet<DefId>,
            // Cache of all expansions we've seen so far. This is a critical
            // optimization for some large types produced by async fn trees.
            expanded_cache: FxHashMap<(DefId, SubstsRef<'tcx>), Ty<'tcx>>,
            primary_def_id: DefId,
            found_recursion: bool,
            tcx: TyCtxt<'tcx>,
        }

        impl<'tcx> OpaqueTypeExpander<'tcx> {
            fn expand_opaque_ty(
                &mut self,
                def_id: DefId,
                substs: SubstsRef<'tcx>,
            ) -> Option<Ty<'tcx>> {
                if self.found_recursion {
                    return None;
                }
                let substs = substs.fold_with(self);
                if self.seen_opaque_tys.insert(def_id) {
                    let expanded_ty = match self.expanded_cache.get(&(def_id, substs)) {
                        Some(expanded_ty) => expanded_ty,
                        None => {
                            let generic_ty = self.tcx.type_of(def_id);
                            let concrete_ty = generic_ty.subst(self.tcx, substs);
                            let expanded_ty = self.fold_ty(concrete_ty);
                            self.expanded_cache.insert((def_id, substs), expanded_ty);
                            expanded_ty
                        }
                    };
                    self.seen_opaque_tys.remove(&def_id);
                    Some(expanded_ty)
                } else {
                    // If another opaque type that we contain is recursive, then it
                    // will report the error, so we don't have to.
                    self.found_recursion = def_id == self.primary_def_id;
                    None
                }
            }
        }

        impl<'tcx> TypeFolder<'tcx> for OpaqueTypeExpander<'tcx> {
            fn tcx(&self) -> TyCtxt<'tcx> {
                self.tcx
            }

            fn fold_ty(&mut self, t: Ty<'tcx>) -> Ty<'tcx> {
                if let ty::Opaque(def_id, substs) = t.kind {
                    self.expand_opaque_ty(def_id, substs).unwrap_or(t)
                } else if t.has_opaque_types() {
                    t.super_fold_with(self)
                } else {
                    t
                }
            }
        }

        let mut visitor = OpaqueTypeExpander {
            seen_opaque_tys: FxHashSet::default(),
            expanded_cache: FxHashMap::default(),
            primary_def_id: def_id,
            found_recursion: false,
            tcx: self,
        };
        let expanded_type = visitor.expand_opaque_ty(def_id, substs).unwrap();
        if visitor.found_recursion { Err(expanded_type) } else { Ok(expanded_type) }
    }
}

impl<'tcx> ty::TyS<'tcx> {
    /// Returns the maximum value for the given numeric type (including `char`s)
    /// or returns `None` if the type is not numeric.
    pub fn numeric_max_val(&'tcx self, tcx: TyCtxt<'tcx>) -> Option<&'tcx ty::Const<'tcx>> {
        let val = match self.kind {
            ty::Int(_) | ty::Uint(_) => {
                let (size, signed) = int_size_and_signed(tcx, self);
                let val = if signed { signed_max(size) as u128 } else { unsigned_max(size) };
                Some(val)
            }
            ty::Char => Some(std::char::MAX as u128),
            ty::Float(fty) => Some(match fty {
                ast::FloatTy::F32 => ::rustc_apfloat::ieee::Single::INFINITY.to_bits(),
                ast::FloatTy::F64 => ::rustc_apfloat::ieee::Double::INFINITY.to_bits(),
            }),
            _ => None,
        };
        val.map(|v| ty::Const::from_bits(tcx, v, ty::ParamEnv::empty().and(self)))
    }

    /// Returns the minimum value for the given numeric type (including `char`s)
    /// or returns `None` if the type is not numeric.
    pub fn numeric_min_val(&'tcx self, tcx: TyCtxt<'tcx>) -> Option<&'tcx ty::Const<'tcx>> {
        let val = match self.kind {
            ty::Int(_) | ty::Uint(_) => {
                let (size, signed) = int_size_and_signed(tcx, self);
                let val = if signed { truncate(signed_min(size) as u128, size) } else { 0 };
                Some(val)
            }
            ty::Char => Some(0),
            ty::Float(fty) => Some(match fty {
                ast::FloatTy::F32 => (-::rustc_apfloat::ieee::Single::INFINITY).to_bits(),
                ast::FloatTy::F64 => (-::rustc_apfloat::ieee::Double::INFINITY).to_bits(),
            }),
            _ => None,
        };
        val.map(|v| ty::Const::from_bits(tcx, v, ty::ParamEnv::empty().and(self)))
    }

    /// Checks whether values of this type `T` are *moved* or *copied*
    /// when referenced -- this amounts to a check for whether `T:
    /// Copy`, but note that we **don't** consider lifetimes when
    /// doing this check. This means that we may generate MIR which
    /// does copies even when the type actually doesn't satisfy the
    /// full requirements for the `Copy` trait (cc #29149) -- this
    /// winds up being reported as an error during NLL borrow check.
    pub fn is_copy_modulo_regions(
        &'tcx self,
        tcx_at: TyCtxtAt<'tcx>,
        param_env: ty::ParamEnv<'tcx>,
    ) -> bool {
        tcx_at.is_copy_raw(param_env.and(self))
    }

    /// Checks whether values of this type `T` have a size known at
    /// compile time (i.e., whether `T: Sized`). Lifetimes are ignored
    /// for the purposes of this check, so it can be an
    /// over-approximation in generic contexts, where one can have
    /// strange rules like `<T as Foo<'static>>::Bar: Sized` that
    /// actually carry lifetime requirements.
    pub fn is_sized(&'tcx self, tcx_at: TyCtxtAt<'tcx>, param_env: ty::ParamEnv<'tcx>) -> bool {
        self.is_trivially_sized(tcx_at.tcx) || tcx_at.is_sized_raw(param_env.and(self))
    }

    /// Checks whether values of this type `T` implement the `Freeze`
    /// trait -- frozen types are those that do not contain a
    /// `UnsafeCell` anywhere. This is a language concept used to
    /// distinguish "true immutability", which is relevant to
    /// optimization as well as the rules around static values. Note
    /// that the `Freeze` trait is not exposed to end users and is
    /// effectively an implementation detail.
    // FIXME: use `TyCtxtAt` instead of separate `Span`.
    pub fn is_freeze(&'tcx self, tcx_at: TyCtxtAt<'tcx>, param_env: ty::ParamEnv<'tcx>) -> bool {
        self.is_trivially_freeze() || tcx_at.is_freeze_raw(param_env.and(self))
    }

    /// Fast path helper for testing if a type is `Freeze`.
    ///
    /// Returning true means the type is known to be `Freeze`. Returning
    /// `false` means nothing -- could be `Freeze`, might not be.
    fn is_trivially_freeze(&self) -> bool {
        match self.kind {
            ty::Int(_)
            | ty::Uint(_)
            | ty::Float(_)
            | ty::Bool
            | ty::Char
            | ty::Str
            | ty::Never
            | ty::Ref(..)
            | ty::RawPtr(_)
            | ty::FnDef(..)
            | ty::Error(_)
            | ty::FnPtr(_) => true,
            ty::Tuple(_) => self.tuple_fields().all(Self::is_trivially_freeze),
            ty::Slice(elem_ty) | ty::Array(elem_ty, _) => elem_ty.is_trivially_freeze(),
            ty::Adt(..)
            | ty::Bound(..)
            | ty::Closure(..)
            | ty::Dynamic(..)
            | ty::Foreign(_)
            | ty::Generator(..)
            | ty::GeneratorWitness(_)
            | ty::Infer(_)
            | ty::Opaque(..)
            | ty::Param(_)
            | ty::Placeholder(_)
            | ty::Projection(_) => false,
        }
    }

    /// If `ty.needs_drop(...)` returns `true`, then `ty` is definitely
    /// non-copy and *might* have a destructor attached; if it returns
    /// `false`, then `ty` definitely has no destructor (i.e., no drop glue).
    ///
    /// (Note that this implies that if `ty` has a destructor attached,
    /// then `needs_drop` will definitely return `true` for `ty`.)
    ///
    /// Note that this method is used to check eligible types in unions.
    #[inline]
    pub fn needs_drop(&'tcx self, tcx: TyCtxt<'tcx>, param_env: ty::ParamEnv<'tcx>) -> bool {
        // Avoid querying in simple cases.
        match needs_drop_components(self, &tcx.data_layout) {
            Err(AlwaysRequiresDrop) => true,
            Ok(components) => {
                let query_ty = match *components {
                    [] => return false,
                    // If we've got a single component, call the query with that
                    // to increase the chance that we hit the query cache.
                    [component_ty] => component_ty,
                    _ => self,
                };
                // This doesn't depend on regions, so try to minimize distinct
                // query keys used.
                let erased = tcx.normalize_erasing_regions(param_env, query_ty);
                tcx.needs_drop_raw(param_env.and(erased))
            }
        }
    }

    /// Returns `true` if equality for this type is both reflexive and structural.
    ///
    /// Reflexive equality for a type is indicated by an `Eq` impl for that type.
    ///
    /// Primitive types (`u32`, `str`) have structural equality by definition. For composite data
    /// types, equality for the type as a whole is structural when it is the same as equality
    /// between all components (fields, array elements, etc.) of that type. For ADTs, structural
    /// equality is indicated by an implementation of `PartialStructuralEq` and `StructuralEq` for
    /// that type.
    ///
    /// This function is "shallow" because it may return `true` for a composite type whose fields
    /// are not `StructuralEq`. For example, `[T; 4]` has structural equality regardless of `T`
    /// because equality for arrays is determined by the equality of each array element. If you
    /// want to know whether a given call to `PartialEq::eq` will proceed structurally all the way
    /// down, you will need to use a type visitor.
    #[inline]
    pub fn is_structural_eq_shallow(&'tcx self, tcx: TyCtxt<'tcx>) -> bool {
        match self.kind {
            // Look for an impl of both `PartialStructuralEq` and `StructuralEq`.
            Adt(..) => tcx.has_structural_eq_impls(self),

            // Primitive types that satisfy `Eq`.
            Bool | Char | Int(_) | Uint(_) | Str | Never => true,

            // Composite types that satisfy `Eq` when all of their fields do.
            //
            // Because this function is "shallow", we return `true` for these composites regardless
            // of the type(s) contained within.
            Ref(..) | Array(..) | Slice(_) | Tuple(..) => true,

            // Raw pointers use bitwise comparison.
            RawPtr(_) | FnPtr(_) => true,

            // Floating point numbers are not `Eq`.
            Float(_) => false,

            // Conservatively return `false` for all others...

            // Anonymous function types
            FnDef(..) | Closure(..) | Dynamic(..) | Generator(..) => false,

            // Generic or inferred types
            //
            // FIXME(ecstaticmorse): Maybe we should `bug` here? This should probably only be
            // called for known, fully-monomorphized types.
            Projection(_) | Opaque(..) | Param(_) | Bound(..) | Placeholder(_) | Infer(_) => false,

            Foreign(_) | GeneratorWitness(..) | Error(_) => false,
        }
    }

    pub fn same_type(a: Ty<'tcx>, b: Ty<'tcx>) -> bool {
        match (&a.kind, &b.kind) {
            (&Adt(did_a, substs_a), &Adt(did_b, substs_b)) => {
                if did_a != did_b {
                    return false;
                }

                substs_a.types().zip(substs_b.types()).all(|(a, b)| Self::same_type(a, b))
            }
            _ => a == b,
        }
    }

    /// Check whether a type is representable. This means it cannot contain unboxed
    /// structural recursion. This check is needed for structs and enums.
    pub fn is_representable(&'tcx self, tcx: TyCtxt<'tcx>, sp: Span) -> Representability {
        // Iterate until something non-representable is found
        fn fold_repr<It: Iterator<Item = Representability>>(iter: It) -> Representability {
            iter.fold(Representability::Representable, |r1, r2| match (r1, r2) {
                (Representability::SelfRecursive(v1), Representability::SelfRecursive(v2)) => {
                    Representability::SelfRecursive(v1.into_iter().chain(v2).collect())
                }
                (r1, r2) => cmp::max(r1, r2),
            })
        }

        fn are_inner_types_recursive<'tcx>(
            tcx: TyCtxt<'tcx>,
            sp: Span,
            seen: &mut Vec<Ty<'tcx>>,
            representable_cache: &mut FxHashMap<Ty<'tcx>, Representability>,
            ty: Ty<'tcx>,
        ) -> Representability {
            match ty.kind {
                Tuple(..) => {
                    // Find non representable
                    fold_repr(ty.tuple_fields().map(|ty| {
                        is_type_structurally_recursive(tcx, sp, seen, representable_cache, ty)
                    }))
                }
                // Fixed-length vectors.
                // FIXME(#11924) Behavior undecided for zero-length vectors.
                Array(ty, _) => {
                    is_type_structurally_recursive(tcx, sp, seen, representable_cache, ty)
                }
                Adt(def, substs) => {
                    // Find non representable fields with their spans
                    fold_repr(def.all_fields().map(|field| {
                        let ty = field.ty(tcx, substs);
                        let span = match field
                            .did
                            .as_local()
                            .map(|id| tcx.hir().as_local_hir_id(id))
                            .and_then(|id| tcx.hir().find(id))
                        {
                            Some(hir::Node::Field(field)) => field.ty.span,
                            _ => sp,
                        };
                        match is_type_structurally_recursive(
                            tcx,
                            span,
                            seen,
                            representable_cache,
                            ty,
                        ) {
                            Representability::SelfRecursive(_) => {
                                Representability::SelfRecursive(vec![span])
                            }
                            x => x,
                        }
                    }))
                }
                Closure(..) => {
                    // this check is run on type definitions, so we don't expect
                    // to see closure types
                    bug!("requires check invoked on inapplicable type: {:?}", ty)
                }
                _ => Representability::Representable,
            }
        }

        fn same_struct_or_enum<'tcx>(ty: Ty<'tcx>, def: &'tcx ty::AdtDef) -> bool {
            match ty.kind {
                Adt(ty_def, _) => ty_def == def,
                _ => false,
            }
        }

        // Does the type `ty` directly (without indirection through a pointer)
        // contain any types on stack `seen`?
        fn is_type_structurally_recursive<'tcx>(
            tcx: TyCtxt<'tcx>,
            sp: Span,
            seen: &mut Vec<Ty<'tcx>>,
            representable_cache: &mut FxHashMap<Ty<'tcx>, Representability>,
            ty: Ty<'tcx>,
        ) -> Representability {
            debug!("is_type_structurally_recursive: {:?} {:?}", ty, sp);
            if let Some(representability) = representable_cache.get(ty) {
                debug!(
                    "is_type_structurally_recursive: {:?} {:?} - (cached) {:?}",
                    ty, sp, representability
                );
                return representability.clone();
            }

            let representability =
                is_type_structurally_recursive_inner(tcx, sp, seen, representable_cache, ty);

            representable_cache.insert(ty, representability.clone());
            representability
        }

        fn is_type_structurally_recursive_inner<'tcx>(
            tcx: TyCtxt<'tcx>,
            sp: Span,
            seen: &mut Vec<Ty<'tcx>>,
            representable_cache: &mut FxHashMap<Ty<'tcx>, Representability>,
            ty: Ty<'tcx>,
        ) -> Representability {
            match ty.kind {
                Adt(def, _) => {
                    {
                        // Iterate through stack of previously seen types.
                        let mut iter = seen.iter();

                        // The first item in `seen` is the type we are actually curious about.
                        // We want to return SelfRecursive if this type contains itself.
                        // It is important that we DON'T take generic parameters into account
                        // for this check, so that Bar<T> in this example counts as SelfRecursive:
                        //
                        // struct Foo;
                        // struct Bar<T> { x: Bar<Foo> }

                        if let Some(&seen_type) = iter.next() {
                            if same_struct_or_enum(seen_type, def) {
                                debug!("SelfRecursive: {:?} contains {:?}", seen_type, ty);
                                return Representability::SelfRecursive(vec![sp]);
                            }
                        }

                        // We also need to know whether the first item contains other types
                        // that are structurally recursive. If we don't catch this case, we
                        // will recurse infinitely for some inputs.
                        //
                        // It is important that we DO take generic parameters into account
                        // here, so that code like this is considered SelfRecursive, not
                        // ContainsRecursive:
                        //
                        // struct Foo { Option<Option<Foo>> }

                        for &seen_type in iter {
                            if ty::TyS::same_type(ty, seen_type) {
                                debug!("ContainsRecursive: {:?} contains {:?}", seen_type, ty);
                                return Representability::ContainsRecursive;
                            }
                        }
                    }

                    // For structs and enums, track all previously seen types by pushing them
                    // onto the 'seen' stack.
                    seen.push(ty);
                    let out = are_inner_types_recursive(tcx, sp, seen, representable_cache, ty);
                    seen.pop();
                    out
                }
                _ => {
                    // No need to push in other cases.
                    are_inner_types_recursive(tcx, sp, seen, representable_cache, ty)
                }
            }
        }

        debug!("is_type_representable: {:?}", self);

        // To avoid a stack overflow when checking an enum variant or struct that
        // contains a different, structurally recursive type, maintain a stack
        // of seen types and check recursion for each of them (issues #3008, #3779).
        let mut seen: Vec<Ty<'_>> = Vec::new();
        let mut representable_cache = FxHashMap::default();
        let r = is_type_structurally_recursive(tcx, sp, &mut seen, &mut representable_cache, self);
        debug!("is_type_representable: {:?} is {:?}", self, r);
        r
    }

    /// Peel off all reference types in this type until there are none left.
    ///
    /// This method is idempotent, i.e. `ty.peel_refs().peel_refs() == ty.peel_refs()`.
    ///
    /// # Examples
    ///
    /// - `u8` -> `u8`
    /// - `&'a mut u8` -> `u8`
    /// - `&'a &'b u8` -> `u8`
    /// - `&'a *const &'b u8 -> *const &'b u8`
    pub fn peel_refs(&'tcx self) -> Ty<'tcx> {
        let mut ty = self;
        while let Ref(_, inner_ty, _) = ty.kind {
            ty = inner_ty;
        }
        ty
    }
}

pub enum ExplicitSelf<'tcx> {
    ByValue,
    ByReference(ty::Region<'tcx>, hir::Mutability),
    ByRawPointer(hir::Mutability),
    ByBox,
    Other,
}

impl<'tcx> ExplicitSelf<'tcx> {
    /// Categorizes an explicit self declaration like `self: SomeType`
    /// into either `self`, `&self`, `&mut self`, `Box<self>`, or
    /// `Other`.
    /// This is mainly used to require the arbitrary_self_types feature
    /// in the case of `Other`, to improve error messages in the common cases,
    /// and to make `Other` non-object-safe.
    ///
    /// Examples:
    ///
    /// ```
    /// impl<'a> Foo for &'a T {
    ///     // Legal declarations:
    ///     fn method1(self: &&'a T); // ExplicitSelf::ByReference
    ///     fn method2(self: &'a T); // ExplicitSelf::ByValue
    ///     fn method3(self: Box<&'a T>); // ExplicitSelf::ByBox
    ///     fn method4(self: Rc<&'a T>); // ExplicitSelf::Other
    ///
    ///     // Invalid cases will be caught by `check_method_receiver`:
    ///     fn method_err1(self: &'a mut T); // ExplicitSelf::Other
    ///     fn method_err2(self: &'static T) // ExplicitSelf::ByValue
    ///     fn method_err3(self: &&T) // ExplicitSelf::ByReference
    /// }
    /// ```
    ///
    pub fn determine<P>(self_arg_ty: Ty<'tcx>, is_self_ty: P) -> ExplicitSelf<'tcx>
    where
        P: Fn(Ty<'tcx>) -> bool,
    {
        use self::ExplicitSelf::*;

        match self_arg_ty.kind {
            _ if is_self_ty(self_arg_ty) => ByValue,
            ty::Ref(region, ty, mutbl) if is_self_ty(ty) => ByReference(region, mutbl),
            ty::RawPtr(ty::TypeAndMut { ty, mutbl }) if is_self_ty(ty) => ByRawPointer(mutbl),
            ty::Adt(def, _) if def.is_box() && is_self_ty(self_arg_ty.boxed_ty()) => ByBox,
            _ => Other,
        }
    }
}

/// Returns a list of types such that the given type needs drop if and only if
/// *any* of the returned types need drop. Returns `Err(AlwaysRequiresDrop)` if
/// this type always needs drop.
pub fn needs_drop_components(
    ty: Ty<'tcx>,
    target_layout: &TargetDataLayout,
) -> Result<SmallVec<[Ty<'tcx>; 2]>, AlwaysRequiresDrop> {
    match ty.kind {
        ty::Infer(ty::FreshIntTy(_))
        | ty::Infer(ty::FreshFloatTy(_))
        | ty::Bool
        | ty::Int(_)
        | ty::Uint(_)
        | ty::Float(_)
        | ty::Never
        | ty::FnDef(..)
        | ty::FnPtr(_)
        | ty::Char
        | ty::GeneratorWitness(..)
        | ty::RawPtr(_)
        | ty::Ref(..)
        | ty::Str => Ok(SmallVec::new()),

        // Foreign types can never have destructors.
        ty::Foreign(..) => Ok(SmallVec::new()),

        ty::Dynamic(..) | ty::Error(_) => Err(AlwaysRequiresDrop),

        ty::Slice(ty) => needs_drop_components(ty, target_layout),
        ty::Array(elem_ty, size) => {
            match needs_drop_components(elem_ty, target_layout) {
                Ok(v) if v.is_empty() => Ok(v),
                res => match size.val.try_to_bits(target_layout.pointer_size) {
                    // Arrays of size zero don't need drop, even if their element
                    // type does.
                    Some(0) => Ok(SmallVec::new()),
                    Some(_) => res,
                    // We don't know which of the cases above we are in, so
                    // return the whole type and let the caller decide what to
                    // do.
                    None => Ok(smallvec![ty]),
                },
            }
        }
        // If any field needs drop, then the whole tuple does.
        ty::Tuple(..) => ty.tuple_fields().try_fold(SmallVec::new(), move |mut acc, elem| {
            acc.extend(needs_drop_components(elem, target_layout)?);
            Ok(acc)
        }),

        // These require checking for `Copy` bounds or `Adt` destructors.
        ty::Adt(..)
        | ty::Projection(..)
        | ty::Param(_)
        | ty::Bound(..)
        | ty::Placeholder(..)
        | ty::Opaque(..)
        | ty::Infer(_)
        | ty::Closure(..)
        | ty::Generator(..) => Ok(smallvec![ty]),
    }
}

#[derive(Copy, Clone, Debug, HashStable, RustcEncodable, RustcDecodable)]
pub struct AlwaysRequiresDrop;
