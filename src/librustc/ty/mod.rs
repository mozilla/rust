// Copyright 2012-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

pub use self::Variance::*;
pub use self::AssociatedItemContainer::*;
pub use self::BorrowKind::*;
pub use self::IntVarValue::*;
pub use self::LvaluePreference::*;
pub use self::fold::TypeFoldable;

use dep_graph::{self, DepNode};
use hir::{map as hir_map, FreevarMap, TraitMap};
use hir::def::{Def, CtorKind, ExportMap};
use hir::def_id::{CrateNum, DefId, DefIndex, CRATE_DEF_INDEX, LOCAL_CRATE};
use ich::StableHashingContext;
use middle::const_val::ConstVal;
use middle::lang_items::{FnTraitLangItem, FnMutTraitLangItem, FnOnceTraitLangItem};
use middle::privacy::AccessLevels;
use middle::region::CodeExtent;
use middle::resolve_lifetime::ObjectLifetimeDefault;
use mir::Mir;
use traits;
use ty;
use ty::subst::{Subst, Substs};
use ty::util::IntTypeExt;
use ty::walk::TypeWalker;
use util::common::ErrorReported;
use util::nodemap::{NodeSet, DefIdMap, FxHashMap, FxHashSet};

use serialize::{self, Encodable, Encoder};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::cmp;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::iter::FromIterator;
use std::ops::Deref;
use std::rc::Rc;
use std::slice;
use std::vec::IntoIter;
use std::mem;
use syntax::ast::{self, DUMMY_NODE_ID, Name, NodeId};
use syntax::attr;
use syntax::symbol::{Symbol, InternedString};
use syntax_pos::{DUMMY_SP, Span};
use rustc_const_math::ConstInt;

use rustc_data_structures::accumulate_vec::IntoIter as AccIntoIter;
use rustc_data_structures::stable_hasher::{StableHasher, StableHasherResult,
                                           HashStable};

use hir;
use hir::itemlikevisit::ItemLikeVisitor;

pub use self::sty::{Binder, DebruijnIndex};
pub use self::sty::{FnSig, PolyFnSig};
pub use self::sty::{InferTy, ParamTy, ProjectionTy, ExistentialPredicate};
pub use self::sty::{ClosureSubsts, TypeAndMut};
pub use self::sty::{TraitRef, TypeVariants, PolyTraitRef};
pub use self::sty::{ExistentialTraitRef, PolyExistentialTraitRef};
pub use self::sty::{ExistentialProjection, PolyExistentialProjection};
pub use self::sty::{BoundRegion, EarlyBoundRegion, FreeRegion, Region};
pub use self::sty::RegionKind;
pub use self::sty::Issue32330;
pub use self::sty::{TyVid, IntVid, FloatVid, RegionVid, SkolemizedRegionVid};
pub use self::sty::BoundRegion::*;
pub use self::sty::InferTy::*;
pub use self::sty::RegionKind::*;
pub use self::sty::TypeVariants::*;

pub use self::context::{TyCtxt, GlobalArenas, tls};
pub use self::context::{Lift, TypeckTables};

pub use self::instance::{Instance, InstanceDef};

pub use self::trait_def::{TraitDef, TraitFlags};

pub use self::maps::queries;

pub mod adjustment;
pub mod cast;
pub mod error;
pub mod fast_reject;
pub mod fold;
pub mod inhabitedness;
pub mod item_path;
pub mod layout;
pub mod _match;
pub mod maps;
pub mod outlives;
pub mod relate;
pub mod steal;
pub mod subst;
pub mod trait_def;
pub mod walk;
pub mod wf;
pub mod util;

mod context;
mod flags;
mod instance;
mod structural_impls;
mod sty;

// Data types

/// The complete set of all analyses described in this module. This is
/// produced by the driver and fed to trans and later passes.
///
/// NB: These contents are being migrated into queries using the
/// *on-demand* infrastructure.
#[derive(Clone)]
pub struct CrateAnalysis {
    pub access_levels: Rc<AccessLevels>,
    pub reachable: Rc<NodeSet>,
    pub name: String,
    pub glob_map: Option<hir::GlobMap>,
}

#[derive(Clone)]
pub struct Resolutions {
    pub freevars: FreevarMap,
    pub trait_map: TraitMap,
    pub maybe_unused_trait_imports: NodeSet,
    pub export_map: ExportMap,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AssociatedItemContainer {
    TraitContainer(DefId),
    ImplContainer(DefId),
}

impl AssociatedItemContainer {
    pub fn id(&self) -> DefId {
        match *self {
            TraitContainer(id) => id,
            ImplContainer(id) => id,
        }
    }
}

/// The "header" of an impl is everything outside the body: a Self type, a trait
/// ref (in the case of a trait impl), and a set of predicates (from the
/// bounds/where clauses).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ImplHeader<'tcx> {
    pub impl_def_id: DefId,
    pub self_ty: Ty<'tcx>,
    pub trait_ref: Option<TraitRef<'tcx>>,
    pub predicates: Vec<Predicate<'tcx>>,
}

impl<'a, 'gcx, 'tcx> ImplHeader<'tcx> {
    pub fn with_fresh_ty_vars(selcx: &mut traits::SelectionContext<'a, 'gcx, 'tcx>,
                              impl_def_id: DefId)
                              -> ImplHeader<'tcx>
    {
        let tcx = selcx.tcx();
        let impl_substs = selcx.infcx().fresh_substs_for_item(DUMMY_SP, impl_def_id);

        let header = ImplHeader {
            impl_def_id: impl_def_id,
            self_ty: tcx.type_of(impl_def_id),
            trait_ref: tcx.impl_trait_ref(impl_def_id),
            predicates: tcx.predicates_of(impl_def_id).predicates
        }.subst(tcx, impl_substs);

        let traits::Normalized { value: mut header, obligations } =
            traits::normalize(selcx, traits::ObligationCause::dummy(), &header);

        header.predicates.extend(obligations.into_iter().map(|o| o.predicate));
        header
    }
}

#[derive(Copy, Clone, Debug)]
pub struct AssociatedItem {
    pub def_id: DefId,
    pub name: Name,
    pub kind: AssociatedKind,
    pub vis: Visibility,
    pub defaultness: hir::Defaultness,
    pub container: AssociatedItemContainer,

    /// Whether this is a method with an explicit self
    /// as its first argument, allowing method calls.
    pub method_has_self_argument: bool,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, RustcEncodable, RustcDecodable)]
pub enum AssociatedKind {
    Const,
    Method,
    Type
}

impl AssociatedItem {
    pub fn def(&self) -> Def {
        match self.kind {
            AssociatedKind::Const => Def::AssociatedConst(self.def_id),
            AssociatedKind::Method => Def::Method(self.def_id),
            AssociatedKind::Type => Def::AssociatedTy(self.def_id),
        }
    }

    /// Tests whether the associated item admits a non-trivial implementation
    /// for !
    pub fn relevant_for_never<'tcx>(&self) -> bool {
        match self.kind {
            AssociatedKind::Const => true,
            AssociatedKind::Type => true,
            // FIXME(canndrew): Be more thorough here, check if any argument is uninhabited.
            AssociatedKind::Method => !self.method_has_self_argument,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Copy, RustcEncodable, RustcDecodable)]
pub enum Visibility {
    /// Visible everywhere (including in other crates).
    Public,
    /// Visible only in the given crate-local module.
    Restricted(DefId),
    /// Not visible anywhere in the local crate. This is the visibility of private external items.
    Invisible,
}

pub trait DefIdTree: Copy {
    fn parent(self, id: DefId) -> Option<DefId>;

    fn is_descendant_of(self, mut descendant: DefId, ancestor: DefId) -> bool {
        if descendant.krate != ancestor.krate {
            return false;
        }

        while descendant != ancestor {
            match self.parent(descendant) {
                Some(parent) => descendant = parent,
                None => return false,
            }
        }
        true
    }
}

impl<'a, 'gcx, 'tcx> DefIdTree for TyCtxt<'a, 'gcx, 'tcx> {
    fn parent(self, id: DefId) -> Option<DefId> {
        self.def_key(id).parent.map(|index| DefId { index: index, ..id })
    }
}

impl Visibility {
    pub fn from_hir(visibility: &hir::Visibility, id: NodeId, tcx: TyCtxt) -> Self {
        match *visibility {
            hir::Public => Visibility::Public,
            hir::Visibility::Crate => Visibility::Restricted(DefId::local(CRATE_DEF_INDEX)),
            hir::Visibility::Restricted { ref path, .. } => match path.def {
                // If there is no resolution, `resolve` will have already reported an error, so
                // assume that the visibility is public to avoid reporting more privacy errors.
                Def::Err => Visibility::Public,
                def => Visibility::Restricted(def.def_id()),
            },
            hir::Inherited => {
                Visibility::Restricted(tcx.hir.local_def_id(tcx.hir.get_module_parent(id)))
            }
        }
    }

    /// Returns true if an item with this visibility is accessible from the given block.
    pub fn is_accessible_from<T: DefIdTree>(self, module: DefId, tree: T) -> bool {
        let restriction = match self {
            // Public items are visible everywhere.
            Visibility::Public => return true,
            // Private items from other crates are visible nowhere.
            Visibility::Invisible => return false,
            // Restricted items are visible in an arbitrary local module.
            Visibility::Restricted(other) if other.krate != module.krate => return false,
            Visibility::Restricted(module) => module,
        };

        tree.is_descendant_of(module, restriction)
    }

    /// Returns true if this visibility is at least as accessible as the given visibility
    pub fn is_at_least<T: DefIdTree>(self, vis: Visibility, tree: T) -> bool {
        let vis_restriction = match vis {
            Visibility::Public => return self == Visibility::Public,
            Visibility::Invisible => return true,
            Visibility::Restricted(module) => module,
        };

        self.is_accessible_from(vis_restriction, tree)
    }
}

#[derive(Clone, PartialEq, RustcDecodable, RustcEncodable, Copy)]
pub enum Variance {
    Covariant,      // T<A> <: T<B> iff A <: B -- e.g., function return type
    Invariant,      // T<A> <: T<B> iff B == A -- e.g., type of mutable cell
    Contravariant,  // T<A> <: T<B> iff B <: A -- e.g., function param type
    Bivariant,      // T<A> <: T<B>            -- e.g., unused type parameter
}

#[derive(Clone, Copy, Debug, RustcDecodable, RustcEncodable)]
pub struct MethodCallee<'tcx> {
    /// Impl method ID, for inherent methods, or trait method ID, otherwise.
    pub def_id: DefId,
    pub ty: Ty<'tcx>,
    pub substs: &'tcx Substs<'tcx>
}

/// With method calls, we store some extra information in
/// side tables (i.e method_map). We use
/// MethodCall as a key to index into these tables instead of
/// just directly using the expression's NodeId. The reason
/// for this being that we may apply adjustments (coercions)
/// with the resulting expression also needing to use the
/// side tables. The problem with this is that we don't
/// assign a separate NodeId to this new expression
/// and so it would clash with the base expression if both
/// needed to add to the side tables. Thus to disambiguate
/// we also keep track of whether there's an adjustment in
/// our key.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, RustcEncodable, RustcDecodable)]
pub struct MethodCall {
    pub expr_id: NodeId,
    pub autoderef: u32
}

impl MethodCall {
    pub fn expr(id: NodeId) -> MethodCall {
        MethodCall {
            expr_id: id,
            autoderef: 0
        }
    }

    pub fn autoderef(expr_id: NodeId, autoderef: u32) -> MethodCall {
        MethodCall {
            expr_id: expr_id,
            autoderef: 1 + autoderef
        }
    }
}

// maps from an expression id that corresponds to a method call to the details
// of the method to be invoked
pub type MethodMap<'tcx> = FxHashMap<MethodCall, MethodCallee<'tcx>>;

// Contains information needed to resolve types and (in the future) look up
// the types of AST nodes.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct CReaderCacheKey {
    pub cnum: CrateNum,
    pub pos: usize,
}

/// Describes the fragment-state associated with a NodeId.
///
/// Currently only unfragmented paths have entries in the table,
/// but longer-term this enum is expected to expand to also
/// include data for fragmented paths.
#[derive(Copy, Clone, Debug)]
pub enum FragmentInfo {
    Moved { var: NodeId, move_expr: NodeId },
    Assigned { var: NodeId, assign_expr: NodeId, assignee_id: NodeId },
}

// Flags that we track on types. These flags are propagated upwards
// through the type during type construction, so that we can quickly
// check whether the type has various kinds of types in it without
// recursing over the type itself.
bitflags! {
    flags TypeFlags: u32 {
        const HAS_PARAMS         = 1 << 0,
        const HAS_SELF           = 1 << 1,
        const HAS_TY_INFER       = 1 << 2,
        const HAS_RE_INFER       = 1 << 3,
        const HAS_RE_SKOL        = 1 << 4,
        const HAS_RE_EARLY_BOUND = 1 << 5,
        const HAS_FREE_REGIONS   = 1 << 6,
        const HAS_TY_ERR         = 1 << 7,
        const HAS_PROJECTION     = 1 << 8,
        const HAS_TY_CLOSURE     = 1 << 9,

        // true if there are "names" of types and regions and so forth
        // that are local to a particular fn
        const HAS_LOCAL_NAMES    = 1 << 10,

        // Present if the type belongs in a local type context.
        // Only set for TyInfer other than Fresh.
        const KEEP_IN_LOCAL_TCX  = 1 << 11,

        // Is there a projection that does not involve a bound region?
        // Currently we can't normalize projections w/ bound regions.
        const HAS_NORMALIZABLE_PROJECTION = 1 << 12,

        const NEEDS_SUBST        = TypeFlags::HAS_PARAMS.bits |
                                   TypeFlags::HAS_SELF.bits |
                                   TypeFlags::HAS_RE_EARLY_BOUND.bits,

        // Flags representing the nominal content of a type,
        // computed by FlagsComputation. If you add a new nominal
        // flag, it should be added here too.
        const NOMINAL_FLAGS     = TypeFlags::HAS_PARAMS.bits |
                                  TypeFlags::HAS_SELF.bits |
                                  TypeFlags::HAS_TY_INFER.bits |
                                  TypeFlags::HAS_RE_INFER.bits |
                                  TypeFlags::HAS_RE_SKOL.bits |
                                  TypeFlags::HAS_RE_EARLY_BOUND.bits |
                                  TypeFlags::HAS_FREE_REGIONS.bits |
                                  TypeFlags::HAS_TY_ERR.bits |
                                  TypeFlags::HAS_PROJECTION.bits |
                                  TypeFlags::HAS_TY_CLOSURE.bits |
                                  TypeFlags::HAS_LOCAL_NAMES.bits |
                                  TypeFlags::KEEP_IN_LOCAL_TCX.bits,

        // Caches for type_is_sized, type_moves_by_default
        const SIZEDNESS_CACHED  = 1 << 16,
        const IS_SIZED          = 1 << 17,
        const MOVENESS_CACHED   = 1 << 18,
        const MOVES_BY_DEFAULT  = 1 << 19,
        const FREEZENESS_CACHED = 1 << 20,
        const IS_FREEZE         = 1 << 21,
        const NEEDS_DROP_CACHED = 1 << 22,
        const NEEDS_DROP        = 1 << 23,
    }
}

pub struct TyS<'tcx> {
    pub sty: TypeVariants<'tcx>,
    pub flags: Cell<TypeFlags>,

    // the maximal depth of any bound regions appearing in this type.
    region_depth: u32,
}

impl<'tcx> PartialEq for TyS<'tcx> {
    #[inline]
    fn eq(&self, other: &TyS<'tcx>) -> bool {
        // (self as *const _) == (other as *const _)
        (self as *const TyS<'tcx>) == (other as *const TyS<'tcx>)
    }
}
impl<'tcx> Eq for TyS<'tcx> {}

impl<'tcx> Hash for TyS<'tcx> {
    fn hash<H: Hasher>(&self, s: &mut H) {
        (self as *const TyS).hash(s)
    }
}

impl<'a, 'tcx> HashStable<StableHashingContext<'a, 'tcx>> for ty::TyS<'tcx> {
    fn hash_stable<W: StableHasherResult>(&self,
                                          hcx: &mut StableHashingContext<'a, 'tcx>,
                                          hasher: &mut StableHasher<W>) {
        let ty::TyS {
            ref sty,

            // The other fields just provide fast access to information that is
            // also contained in `sty`, so no need to hash them.
            flags: _,
            region_depth: _,
        } = *self;

        sty.hash_stable(hcx, hasher);
    }
}

pub type Ty<'tcx> = &'tcx TyS<'tcx>;

impl<'tcx> serialize::UseSpecializedEncodable for Ty<'tcx> {}
impl<'tcx> serialize::UseSpecializedDecodable for Ty<'tcx> {}

/// A wrapper for slices with the additional invariant
/// that the slice is interned and no other slice with
/// the same contents can exist in the same context.
/// This means we can use pointer + length for both
/// equality comparisons and hashing.
#[derive(Debug, RustcEncodable)]
pub struct Slice<T>([T]);

impl<T> PartialEq for Slice<T> {
    #[inline]
    fn eq(&self, other: &Slice<T>) -> bool {
        (&self.0 as *const [T]) == (&other.0 as *const [T])
    }
}
impl<T> Eq for Slice<T> {}

impl<T> Hash for Slice<T> {
    fn hash<H: Hasher>(&self, s: &mut H) {
        (self.as_ptr(), self.len()).hash(s)
    }
}

impl<T> Deref for Slice<T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        &self.0
    }
}

impl<'a, T> IntoIterator for &'a Slice<T> {
    type Item = &'a T;
    type IntoIter = <&'a [T] as IntoIterator>::IntoIter;
    fn into_iter(self) -> Self::IntoIter {
        self[..].iter()
    }
}

impl<'tcx> serialize::UseSpecializedDecodable for &'tcx Slice<Ty<'tcx>> {}

impl<T> Slice<T> {
    pub fn empty<'a>() -> &'a Slice<T> {
        unsafe {
            mem::transmute(slice::from_raw_parts(0x1 as *const T, 0))
        }
    }
}

/// Upvars do not get their own node-id. Instead, we use the pair of
/// the original var id (that is, the root variable that is referenced
/// by the upvar) and the id of the closure expression.
#[derive(Clone, Copy, PartialEq, Eq, Hash, RustcEncodable, RustcDecodable)]
pub struct UpvarId {
    pub var_id: NodeId,
    pub closure_expr_id: NodeId,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, RustcEncodable, RustcDecodable, Copy)]
pub enum BorrowKind {
    /// Data must be immutable and is aliasable.
    ImmBorrow,

    /// Data must be immutable but not aliasable.  This kind of borrow
    /// cannot currently be expressed by the user and is used only in
    /// implicit closure bindings. It is needed when the closure
    /// is borrowing or mutating a mutable referent, e.g.:
    ///
    ///    let x: &mut isize = ...;
    ///    let y = || *x += 5;
    ///
    /// If we were to try to translate this closure into a more explicit
    /// form, we'd encounter an error with the code as written:
    ///
    ///    struct Env { x: & &mut isize }
    ///    let x: &mut isize = ...;
    ///    let y = (&mut Env { &x }, fn_ptr);  // Closure is pair of env and fn
    ///    fn fn_ptr(env: &mut Env) { **env.x += 5; }
    ///
    /// This is then illegal because you cannot mutate a `&mut` found
    /// in an aliasable location. To solve, you'd have to translate with
    /// an `&mut` borrow:
    ///
    ///    struct Env { x: & &mut isize }
    ///    let x: &mut isize = ...;
    ///    let y = (&mut Env { &mut x }, fn_ptr); // changed from &x to &mut x
    ///    fn fn_ptr(env: &mut Env) { **env.x += 5; }
    ///
    /// Now the assignment to `**env.x` is legal, but creating a
    /// mutable pointer to `x` is not because `x` is not mutable. We
    /// could fix this by declaring `x` as `let mut x`. This is ok in
    /// user code, if awkward, but extra weird for closures, since the
    /// borrow is hidden.
    ///
    /// So we introduce a "unique imm" borrow -- the referent is
    /// immutable, but not aliasable. This solves the problem. For
    /// simplicity, we don't give users the way to express this
    /// borrow, it's just used when translating closures.
    UniqueImmBorrow,

    /// Data is mutable and not aliasable.
    MutBorrow
}

/// Information describing the capture of an upvar. This is computed
/// during `typeck`, specifically by `regionck`.
#[derive(PartialEq, Clone, Debug, Copy, RustcEncodable, RustcDecodable)]
pub enum UpvarCapture<'tcx> {
    /// Upvar is captured by value. This is always true when the
    /// closure is labeled `move`, but can also be true in other cases
    /// depending on inference.
    ByValue,

    /// Upvar is captured by reference.
    ByRef(UpvarBorrow<'tcx>),
}

#[derive(PartialEq, Clone, Copy, RustcEncodable, RustcDecodable)]
pub struct UpvarBorrow<'tcx> {
    /// The kind of borrow: by-ref upvars have access to shared
    /// immutable borrows, which are not part of the normal language
    /// syntax.
    pub kind: BorrowKind,

    /// Region of the resulting reference.
    pub region: ty::Region<'tcx>,
}

pub type UpvarCaptureMap<'tcx> = FxHashMap<UpvarId, UpvarCapture<'tcx>>;

#[derive(Copy, Clone)]
pub struct ClosureUpvar<'tcx> {
    pub def: Def,
    pub span: Span,
    pub ty: Ty<'tcx>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum IntVarValue {
    IntType(ast::IntTy),
    UintType(ast::UintTy),
}

#[derive(Copy, Clone, RustcEncodable, RustcDecodable)]
pub struct TypeParameterDef {
    pub name: Name,
    pub def_id: DefId,
    pub index: u32,
    pub has_default: bool,
    pub object_lifetime_default: ObjectLifetimeDefault,

    /// `pure_wrt_drop`, set by the (unsafe) `#[may_dangle]` attribute
    /// on generic parameter `T`, asserts data behind the parameter
    /// `T` won't be accessed during the parent type's `Drop` impl.
    pub pure_wrt_drop: bool,
}

#[derive(Copy, Clone, RustcEncodable, RustcDecodable)]
pub struct RegionParameterDef {
    pub name: Name,
    pub def_id: DefId,
    pub index: u32,
    pub issue_32330: Option<ty::Issue32330>,

    /// `pure_wrt_drop`, set by the (unsafe) `#[may_dangle]` attribute
    /// on generic parameter `'a`, asserts data of lifetime `'a`
    /// won't be accessed during the parent type's `Drop` impl.
    pub pure_wrt_drop: bool,
}

impl RegionParameterDef {
    pub fn to_early_bound_region_data(&self) -> ty::EarlyBoundRegion {
        ty::EarlyBoundRegion {
            index: self.index,
            name: self.name,
        }
    }

    pub fn to_bound_region(&self) -> ty::BoundRegion {
        ty::BoundRegion::BrNamed(self.def_id, self.name)
    }
}

/// Information about the formal type/lifetime parameters associated
/// with an item or method. Analogous to hir::Generics.
#[derive(Clone, Debug, RustcEncodable, RustcDecodable)]
pub struct Generics {
    pub parent: Option<DefId>,
    pub parent_regions: u32,
    pub parent_types: u32,
    pub regions: Vec<RegionParameterDef>,
    pub types: Vec<TypeParameterDef>,

    /// Reverse map to each `TypeParameterDef`'s `index` field, from
    /// `def_id.index` (`def_id.krate` is the same as the item's).
    pub type_param_to_index: BTreeMap<DefIndex, u32>,

    pub has_self: bool,
}

impl Generics {
    pub fn parent_count(&self) -> usize {
        self.parent_regions as usize + self.parent_types as usize
    }

    pub fn own_count(&self) -> usize {
        self.regions.len() + self.types.len()
    }

    pub fn count(&self) -> usize {
        self.parent_count() + self.own_count()
    }

    pub fn region_param(&self, param: &EarlyBoundRegion) -> &RegionParameterDef {
        assert_eq!(self.parent_count(), 0);
        &self.regions[param.index as usize - self.has_self as usize]
    }

    pub fn type_param(&self, param: &ParamTy) -> &TypeParameterDef {
        assert_eq!(self.parent_count(), 0);
        &self.types[param.idx as usize - self.has_self as usize - self.regions.len()]
    }
}

/// Bounds on generics.
#[derive(Clone, Default)]
pub struct GenericPredicates<'tcx> {
    pub parent: Option<DefId>,
    pub predicates: Vec<Predicate<'tcx>>,
}

impl<'tcx> serialize::UseSpecializedEncodable for GenericPredicates<'tcx> {}
impl<'tcx> serialize::UseSpecializedDecodable for GenericPredicates<'tcx> {}

impl<'a, 'gcx, 'tcx> GenericPredicates<'tcx> {
    pub fn instantiate(&self, tcx: TyCtxt<'a, 'gcx, 'tcx>, substs: &Substs<'tcx>)
                       -> InstantiatedPredicates<'tcx> {
        let mut instantiated = InstantiatedPredicates::empty();
        self.instantiate_into(tcx, &mut instantiated, substs);
        instantiated
    }
    pub fn instantiate_own(&self, tcx: TyCtxt<'a, 'gcx, 'tcx>, substs: &Substs<'tcx>)
                           -> InstantiatedPredicates<'tcx> {
        InstantiatedPredicates {
            predicates: self.predicates.subst(tcx, substs)
        }
    }

    fn instantiate_into(&self, tcx: TyCtxt<'a, 'gcx, 'tcx>,
                        instantiated: &mut InstantiatedPredicates<'tcx>,
                        substs: &Substs<'tcx>) {
        if let Some(def_id) = self.parent {
            tcx.predicates_of(def_id).instantiate_into(tcx, instantiated, substs);
        }
        instantiated.predicates.extend(self.predicates.iter().map(|p| p.subst(tcx, substs)))
    }

    pub fn instantiate_supertrait(&self, tcx: TyCtxt<'a, 'gcx, 'tcx>,
                                  poly_trait_ref: &ty::PolyTraitRef<'tcx>)
                                  -> InstantiatedPredicates<'tcx>
    {
        assert_eq!(self.parent, None);
        InstantiatedPredicates {
            predicates: self.predicates.iter().map(|pred| {
                pred.subst_supertrait(tcx, poly_trait_ref)
            }).collect()
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, RustcEncodable, RustcDecodable)]
pub enum Predicate<'tcx> {
    /// Corresponds to `where Foo : Bar<A,B,C>`. `Foo` here would be
    /// the `Self` type of the trait reference and `A`, `B`, and `C`
    /// would be the type parameters.
    Trait(PolyTraitPredicate<'tcx>),

    /// where `T1 == T2`.
    Equate(PolyEquatePredicate<'tcx>),

    /// where 'a : 'b
    RegionOutlives(PolyRegionOutlivesPredicate<'tcx>),

    /// where T : 'a
    TypeOutlives(PolyTypeOutlivesPredicate<'tcx>),

    /// where <T as TraitRef>::Name == X, approximately.
    /// See `ProjectionPredicate` struct for details.
    Projection(PolyProjectionPredicate<'tcx>),

    /// no syntax: T WF
    WellFormed(Ty<'tcx>),

    /// trait must be object-safe
    ObjectSafe(DefId),

    /// No direct syntax. May be thought of as `where T : FnFoo<...>`
    /// for some substitutions `...` and T being a closure type.
    /// Satisfied (or refuted) once we know the closure's kind.
    ClosureKind(DefId, ClosureKind),

    /// `T1 <: T2`
    Subtype(PolySubtypePredicate<'tcx>),
}

impl<'a, 'gcx, 'tcx> Predicate<'tcx> {
    /// Performs a substitution suitable for going from a
    /// poly-trait-ref to supertraits that must hold if that
    /// poly-trait-ref holds. This is slightly different from a normal
    /// substitution in terms of what happens with bound regions.  See
    /// lengthy comment below for details.
    pub fn subst_supertrait(&self, tcx: TyCtxt<'a, 'gcx, 'tcx>,
                            trait_ref: &ty::PolyTraitRef<'tcx>)
                            -> ty::Predicate<'tcx>
    {
        // The interaction between HRTB and supertraits is not entirely
        // obvious. Let me walk you (and myself) through an example.
        //
        // Let's start with an easy case. Consider two traits:
        //
        //     trait Foo<'a> : Bar<'a,'a> { }
        //     trait Bar<'b,'c> { }
        //
        // Now, if we have a trait reference `for<'x> T : Foo<'x>`, then
        // we can deduce that `for<'x> T : Bar<'x,'x>`. Basically, if we
        // knew that `Foo<'x>` (for any 'x) then we also know that
        // `Bar<'x,'x>` (for any 'x). This more-or-less falls out from
        // normal substitution.
        //
        // In terms of why this is sound, the idea is that whenever there
        // is an impl of `T:Foo<'a>`, it must show that `T:Bar<'a,'a>`
        // holds.  So if there is an impl of `T:Foo<'a>` that applies to
        // all `'a`, then we must know that `T:Bar<'a,'a>` holds for all
        // `'a`.
        //
        // Another example to be careful of is this:
        //
        //     trait Foo1<'a> : for<'b> Bar1<'a,'b> { }
        //     trait Bar1<'b,'c> { }
        //
        // Here, if we have `for<'x> T : Foo1<'x>`, then what do we know?
        // The answer is that we know `for<'x,'b> T : Bar1<'x,'b>`. The
        // reason is similar to the previous example: any impl of
        // `T:Foo1<'x>` must show that `for<'b> T : Bar1<'x, 'b>`.  So
        // basically we would want to collapse the bound lifetimes from
        // the input (`trait_ref`) and the supertraits.
        //
        // To achieve this in practice is fairly straightforward. Let's
        // consider the more complicated scenario:
        //
        // - We start out with `for<'x> T : Foo1<'x>`. In this case, `'x`
        //   has a De Bruijn index of 1. We want to produce `for<'x,'b> T : Bar1<'x,'b>`,
        //   where both `'x` and `'b` would have a DB index of 1.
        //   The substitution from the input trait-ref is therefore going to be
        //   `'a => 'x` (where `'x` has a DB index of 1).
        // - The super-trait-ref is `for<'b> Bar1<'a,'b>`, where `'a` is an
        //   early-bound parameter and `'b' is a late-bound parameter with a
        //   DB index of 1.
        // - If we replace `'a` with `'x` from the input, it too will have
        //   a DB index of 1, and thus we'll have `for<'x,'b> Bar1<'x,'b>`
        //   just as we wanted.
        //
        // There is only one catch. If we just apply the substitution `'a
        // => 'x` to `for<'b> Bar1<'a,'b>`, the substitution code will
        // adjust the DB index because we substituting into a binder (it
        // tries to be so smart...) resulting in `for<'x> for<'b>
        // Bar1<'x,'b>` (we have no syntax for this, so use your
        // imagination). Basically the 'x will have DB index of 2 and 'b
        // will have DB index of 1. Not quite what we want. So we apply
        // the substitution to the *contents* of the trait reference,
        // rather than the trait reference itself (put another way, the
        // substitution code expects equal binding levels in the values
        // from the substitution and the value being substituted into, and
        // this trick achieves that).

        let substs = &trait_ref.0.substs;
        match *self {
            Predicate::Trait(ty::Binder(ref data)) =>
                Predicate::Trait(ty::Binder(data.subst(tcx, substs))),
            Predicate::Equate(ty::Binder(ref data)) =>
                Predicate::Equate(ty::Binder(data.subst(tcx, substs))),
            Predicate::Subtype(ty::Binder(ref data)) =>
                Predicate::Subtype(ty::Binder(data.subst(tcx, substs))),
            Predicate::RegionOutlives(ty::Binder(ref data)) =>
                Predicate::RegionOutlives(ty::Binder(data.subst(tcx, substs))),
            Predicate::TypeOutlives(ty::Binder(ref data)) =>
                Predicate::TypeOutlives(ty::Binder(data.subst(tcx, substs))),
            Predicate::Projection(ty::Binder(ref data)) =>
                Predicate::Projection(ty::Binder(data.subst(tcx, substs))),
            Predicate::WellFormed(data) =>
                Predicate::WellFormed(data.subst(tcx, substs)),
            Predicate::ObjectSafe(trait_def_id) =>
                Predicate::ObjectSafe(trait_def_id),
            Predicate::ClosureKind(closure_def_id, kind) =>
                Predicate::ClosureKind(closure_def_id, kind),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, RustcEncodable, RustcDecodable)]
pub struct TraitPredicate<'tcx> {
    pub trait_ref: TraitRef<'tcx>
}
pub type PolyTraitPredicate<'tcx> = ty::Binder<TraitPredicate<'tcx>>;

impl<'tcx> TraitPredicate<'tcx> {
    pub fn def_id(&self) -> DefId {
        self.trait_ref.def_id
    }

    /// Creates the dep-node for selecting/evaluating this trait reference.
    fn dep_node(&self) -> DepNode<DefId> {
        // Extact the trait-def and first def-id from inputs.  See the
        // docs for `DepNode::TraitSelect` for more information.
        let trait_def_id = self.def_id();
        let input_def_id =
            self.input_types()
                .flat_map(|t| t.walk())
                .filter_map(|t| match t.sty {
                    ty::TyAdt(adt_def, _) => Some(adt_def.did),
                    _ => None
                })
                .next()
                .unwrap_or(trait_def_id);
        DepNode::TraitSelect {
            trait_def_id: trait_def_id,
            input_def_id: input_def_id
        }
    }

    pub fn input_types<'a>(&'a self) -> impl DoubleEndedIterator<Item=Ty<'tcx>> + 'a {
        self.trait_ref.input_types()
    }

    pub fn self_ty(&self) -> Ty<'tcx> {
        self.trait_ref.self_ty()
    }
}

impl<'tcx> PolyTraitPredicate<'tcx> {
    pub fn def_id(&self) -> DefId {
        // ok to skip binder since trait def-id does not care about regions
        self.0.def_id()
    }

    pub fn dep_node(&self) -> DepNode<DefId> {
        // ok to skip binder since depnode does not care about regions
        self.0.dep_node()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, RustcEncodable, RustcDecodable)]
pub struct EquatePredicate<'tcx>(pub Ty<'tcx>, pub Ty<'tcx>); // `0 == 1`
pub type PolyEquatePredicate<'tcx> = ty::Binder<EquatePredicate<'tcx>>;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, RustcEncodable, RustcDecodable)]
pub struct OutlivesPredicate<A,B>(pub A, pub B); // `A : B`
pub type PolyOutlivesPredicate<A,B> = ty::Binder<OutlivesPredicate<A,B>>;
pub type PolyRegionOutlivesPredicate<'tcx> = PolyOutlivesPredicate<ty::Region<'tcx>,
                                                                   ty::Region<'tcx>>;
pub type PolyTypeOutlivesPredicate<'tcx> = PolyOutlivesPredicate<Ty<'tcx>, ty::Region<'tcx>>;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, RustcEncodable, RustcDecodable)]
pub struct SubtypePredicate<'tcx> {
    pub a_is_expected: bool,
    pub a: Ty<'tcx>,
    pub b: Ty<'tcx>
}
pub type PolySubtypePredicate<'tcx> = ty::Binder<SubtypePredicate<'tcx>>;

/// This kind of predicate has no *direct* correspondent in the
/// syntax, but it roughly corresponds to the syntactic forms:
///
/// 1. `T : TraitRef<..., Item=Type>`
/// 2. `<T as TraitRef<...>>::Item == Type` (NYI)
///
/// In particular, form #1 is "desugared" to the combination of a
/// normal trait predicate (`T : TraitRef<...>`) and one of these
/// predicates. Form #2 is a broader form in that it also permits
/// equality between arbitrary types. Processing an instance of Form
/// #2 eventually yields one of these `ProjectionPredicate`
/// instances to normalize the LHS.
#[derive(Copy, Clone, PartialEq, Eq, Hash, RustcEncodable, RustcDecodable)]
pub struct ProjectionPredicate<'tcx> {
    pub projection_ty: ProjectionTy<'tcx>,
    pub ty: Ty<'tcx>,
}

pub type PolyProjectionPredicate<'tcx> = Binder<ProjectionPredicate<'tcx>>;

impl<'tcx> PolyProjectionPredicate<'tcx> {
    pub fn item_name(&self) -> Name {
        self.0.projection_ty.item_name // safe to skip the binder to access a name
    }
}

pub trait ToPolyTraitRef<'tcx> {
    fn to_poly_trait_ref(&self) -> PolyTraitRef<'tcx>;
}

impl<'tcx> ToPolyTraitRef<'tcx> for TraitRef<'tcx> {
    fn to_poly_trait_ref(&self) -> PolyTraitRef<'tcx> {
        assert!(!self.has_escaping_regions());
        ty::Binder(self.clone())
    }
}

impl<'tcx> ToPolyTraitRef<'tcx> for PolyTraitPredicate<'tcx> {
    fn to_poly_trait_ref(&self) -> PolyTraitRef<'tcx> {
        self.map_bound_ref(|trait_pred| trait_pred.trait_ref)
    }
}

impl<'tcx> ToPolyTraitRef<'tcx> for PolyProjectionPredicate<'tcx> {
    fn to_poly_trait_ref(&self) -> PolyTraitRef<'tcx> {
        // Note: unlike with TraitRef::to_poly_trait_ref(),
        // self.0.trait_ref is permitted to have escaping regions.
        // This is because here `self` has a `Binder` and so does our
        // return value, so we are preserving the number of binding
        // levels.
        ty::Binder(self.0.projection_ty.trait_ref)
    }
}

pub trait ToPredicate<'tcx> {
    fn to_predicate(&self) -> Predicate<'tcx>;
}

impl<'tcx> ToPredicate<'tcx> for TraitRef<'tcx> {
    fn to_predicate(&self) -> Predicate<'tcx> {
        // we're about to add a binder, so let's check that we don't
        // accidentally capture anything, or else that might be some
        // weird debruijn accounting.
        assert!(!self.has_escaping_regions());

        ty::Predicate::Trait(ty::Binder(ty::TraitPredicate {
            trait_ref: self.clone()
        }))
    }
}

impl<'tcx> ToPredicate<'tcx> for PolyTraitRef<'tcx> {
    fn to_predicate(&self) -> Predicate<'tcx> {
        ty::Predicate::Trait(self.to_poly_trait_predicate())
    }
}

impl<'tcx> ToPredicate<'tcx> for PolyEquatePredicate<'tcx> {
    fn to_predicate(&self) -> Predicate<'tcx> {
        Predicate::Equate(self.clone())
    }
}

impl<'tcx> ToPredicate<'tcx> for PolyRegionOutlivesPredicate<'tcx> {
    fn to_predicate(&self) -> Predicate<'tcx> {
        Predicate::RegionOutlives(self.clone())
    }
}

impl<'tcx> ToPredicate<'tcx> for PolyTypeOutlivesPredicate<'tcx> {
    fn to_predicate(&self) -> Predicate<'tcx> {
        Predicate::TypeOutlives(self.clone())
    }
}

impl<'tcx> ToPredicate<'tcx> for PolyProjectionPredicate<'tcx> {
    fn to_predicate(&self) -> Predicate<'tcx> {
        Predicate::Projection(self.clone())
    }
}

impl<'tcx> Predicate<'tcx> {
    /// Iterates over the types in this predicate. Note that in all
    /// cases this is skipping over a binder, so late-bound regions
    /// with depth 0 are bound by the predicate.
    pub fn walk_tys(&self) -> IntoIter<Ty<'tcx>> {
        let vec: Vec<_> = match *self {
            ty::Predicate::Trait(ref data) => {
                data.skip_binder().input_types().collect()
            }
            ty::Predicate::Equate(ty::Binder(ref data)) => {
                vec![data.0, data.1]
            }
            ty::Predicate::Subtype(ty::Binder(SubtypePredicate { a, b, a_is_expected: _ })) => {
                vec![a, b]
            }
            ty::Predicate::TypeOutlives(ty::Binder(ref data)) => {
                vec![data.0]
            }
            ty::Predicate::RegionOutlives(..) => {
                vec![]
            }
            ty::Predicate::Projection(ref data) => {
                let trait_inputs = data.0.projection_ty.trait_ref.input_types();
                trait_inputs.chain(Some(data.0.ty)).collect()
            }
            ty::Predicate::WellFormed(data) => {
                vec![data]
            }
            ty::Predicate::ObjectSafe(_trait_def_id) => {
                vec![]
            }
            ty::Predicate::ClosureKind(_closure_def_id, _kind) => {
                vec![]
            }
        };

        // The only reason to collect into a vector here is that I was
        // too lazy to make the full (somewhat complicated) iterator
        // type that would be needed here. But I wanted this fn to
        // return an iterator conceptually, rather than a `Vec`, so as
        // to be closer to `Ty::walk`.
        vec.into_iter()
    }

    pub fn to_opt_poly_trait_ref(&self) -> Option<PolyTraitRef<'tcx>> {
        match *self {
            Predicate::Trait(ref t) => {
                Some(t.to_poly_trait_ref())
            }
            Predicate::Projection(..) |
            Predicate::Equate(..) |
            Predicate::Subtype(..) |
            Predicate::RegionOutlives(..) |
            Predicate::WellFormed(..) |
            Predicate::ObjectSafe(..) |
            Predicate::ClosureKind(..) |
            Predicate::TypeOutlives(..) => {
                None
            }
        }
    }
}

/// Represents the bounds declared on a particular set of type
/// parameters.  Should eventually be generalized into a flag list of
/// where clauses.  You can obtain a `InstantiatedPredicates` list from a
/// `GenericPredicates` by using the `instantiate` method. Note that this method
/// reflects an important semantic invariant of `InstantiatedPredicates`: while
/// the `GenericPredicates` are expressed in terms of the bound type
/// parameters of the impl/trait/whatever, an `InstantiatedPredicates` instance
/// represented a set of bounds for some particular instantiation,
/// meaning that the generic parameters have been substituted with
/// their values.
///
/// Example:
///
///     struct Foo<T,U:Bar<T>> { ... }
///
/// Here, the `GenericPredicates` for `Foo` would contain a list of bounds like
/// `[[], [U:Bar<T>]]`.  Now if there were some particular reference
/// like `Foo<isize,usize>`, then the `InstantiatedPredicates` would be `[[],
/// [usize:Bar<isize>]]`.
#[derive(Clone)]
pub struct InstantiatedPredicates<'tcx> {
    pub predicates: Vec<Predicate<'tcx>>,
}

impl<'tcx> InstantiatedPredicates<'tcx> {
    pub fn empty() -> InstantiatedPredicates<'tcx> {
        InstantiatedPredicates { predicates: vec![] }
    }

    pub fn is_empty(&self) -> bool {
        self.predicates.is_empty()
    }
}

/// When type checking, we use the `ParameterEnvironment` to track
/// details about the type/lifetime parameters that are in scope.
/// It primarily stores the bounds information.
///
/// Note: This information might seem to be redundant with the data in
/// `tcx.ty_param_defs`, but it is not. That table contains the
/// parameter definitions from an "outside" perspective, but this
/// struct will contain the bounds for a parameter as seen from inside
/// the function body. Currently the only real distinction is that
/// bound lifetime parameters are replaced with free ones, but in the
/// future I hope to refine the representation of types so as to make
/// more distinctions clearer.
#[derive(Clone)]
pub struct ParameterEnvironment<'tcx> {
    /// See `construct_free_substs` for details.
    pub free_substs: &'tcx Substs<'tcx>,

    /// Each type parameter has an implicit region bound that
    /// indicates it must outlive at least the function body (the user
    /// may specify stronger requirements). This field indicates the
    /// region of the callee. If it is `None`, then the parameter
    /// environment is for an item or something where the "callee" is
    /// not clear.
    pub implicit_region_bound: Option<ty::Region<'tcx>>,

    /// Obligations that the caller must satisfy. This is basically
    /// the set of bounds on the in-scope type parameters, translated
    /// into Obligations, and elaborated and normalized.
    pub caller_bounds: &'tcx [ty::Predicate<'tcx>],

    /// Scope that is attached to free regions for this scope. This is
    /// usually the id of the fn body, but for more abstract scopes
    /// like structs we use None or the item extent.
    ///
    /// FIXME(#3696). It would be nice to refactor so that free
    /// regions don't have this implicit scope and instead introduce
    /// relationships in the environment.
    pub free_id_outlive: Option<CodeExtent<'tcx>>,

    /// A cache for `moves_by_default`.
    pub is_copy_cache: RefCell<FxHashMap<Ty<'tcx>, bool>>,

    /// A cache for `type_is_sized`
    pub is_sized_cache: RefCell<FxHashMap<Ty<'tcx>, bool>>,

    /// A cache for `type_is_freeze`
    pub is_freeze_cache: RefCell<FxHashMap<Ty<'tcx>, bool>>,
}

impl<'a, 'tcx> ParameterEnvironment<'tcx> {
    pub fn with_caller_bounds(&self,
                              caller_bounds: &'tcx [ty::Predicate<'tcx>])
                              -> ParameterEnvironment<'tcx>
    {
        ParameterEnvironment {
            free_substs: self.free_substs,
            implicit_region_bound: self.implicit_region_bound,
            caller_bounds: caller_bounds,
            free_id_outlive: self.free_id_outlive,
            is_copy_cache: RefCell::new(FxHashMap()),
            is_sized_cache: RefCell::new(FxHashMap()),
            is_freeze_cache: RefCell::new(FxHashMap()),
        }
    }

    /// Construct a parameter environment given an item, impl item, or trait item
    pub fn for_item(tcx: TyCtxt<'a, 'tcx, 'tcx>, id: NodeId)
                    -> ParameterEnvironment<'tcx> {
        match tcx.hir.find(id) {
            Some(hir_map::NodeImplItem(ref impl_item)) => {
                match impl_item.node {
                    hir::ImplItemKind::Type(_) | hir::ImplItemKind::Const(..) => {
                        // associated types don't have their own entry (for some reason),
                        // so for now just grab environment for the impl
                        let impl_id = tcx.hir.get_parent(id);
                        let impl_def_id = tcx.hir.local_def_id(impl_id);
                        tcx.construct_parameter_environment(impl_item.span,
                                                            impl_def_id,
                                                            Some(tcx.item_extent(id)))
                    }
                    hir::ImplItemKind::Method(_, ref body) => {
                        tcx.construct_parameter_environment(
                            impl_item.span,
                            tcx.hir.local_def_id(id),
                            Some(tcx.call_site_extent(id, body.node_id)))
                    }
                }
            }
            Some(hir_map::NodeTraitItem(trait_item)) => {
                match trait_item.node {
                    hir::TraitItemKind::Type(..) | hir::TraitItemKind::Const(..) => {
                        // associated types don't have their own entry (for some reason),
                        // so for now just grab environment for the trait
                        let trait_id = tcx.hir.get_parent(id);
                        let trait_def_id = tcx.hir.local_def_id(trait_id);
                        tcx.construct_parameter_environment(trait_item.span,
                                                            trait_def_id,
                                                            Some(tcx.item_extent(id)))
                    }
                    hir::TraitItemKind::Method(_, ref body) => {
                        // Use call-site for extent (unless this is a
                        // trait method with no default; then fallback
                        // to the method id).
                        let extent = if let hir::TraitMethod::Provided(body_id) = *body {
                            // default impl: use call_site extent as free_id_outlive bound.
                            tcx.call_site_extent(id, body_id.node_id)
                        } else {
                            // no default impl: use item extent as free_id_outlive bound.
                            tcx.item_extent(id)
                        };
                        tcx.construct_parameter_environment(
                            trait_item.span,
                            tcx.hir.local_def_id(id),
                            Some(extent))
                    }
                }
            }
            Some(hir_map::NodeItem(item)) => {
                match item.node {
                    hir::ItemFn(.., body_id) => {
                        // We assume this is a function.
                        let fn_def_id = tcx.hir.local_def_id(id);

                        tcx.construct_parameter_environment(
                            item.span,
                            fn_def_id,
                            Some(tcx.call_site_extent(id, body_id.node_id)))
                    }
                    hir::ItemEnum(..) |
                    hir::ItemStruct(..) |
                    hir::ItemUnion(..) |
                    hir::ItemTy(..) |
                    hir::ItemImpl(..) |
                    hir::ItemConst(..) |
                    hir::ItemStatic(..) => {
                        let def_id = tcx.hir.local_def_id(id);
                        tcx.construct_parameter_environment(item.span,
                                                            def_id,
                                                            Some(tcx.item_extent(id)))
                    }
                    hir::ItemTrait(..) => {
                        let def_id = tcx.hir.local_def_id(id);
                        tcx.construct_parameter_environment(item.span,
                                                            def_id,
                                                            Some(tcx.item_extent(id)))
                    }
                    _ => {
                        span_bug!(item.span,
                                  "ParameterEnvironment::for_item():
                                   can't create a parameter \
                                   environment for this kind of item")
                    }
                }
            }
            Some(hir_map::NodeExpr(expr)) => {
                // This is a convenience to allow closures to work.
                if let hir::ExprClosure(.., body, _) = expr.node {
                    let def_id = tcx.hir.local_def_id(id);
                    let base_def_id = tcx.closure_base_def_id(def_id);
                    tcx.construct_parameter_environment(
                        expr.span,
                        base_def_id,
                        Some(tcx.call_site_extent(id, body.node_id)))
                } else {
                    tcx.empty_parameter_environment()
                }
            }
            Some(hir_map::NodeForeignItem(item)) => {
                let def_id = tcx.hir.local_def_id(id);
                tcx.construct_parameter_environment(item.span,
                                                    def_id,
                                                    None)
            }
            Some(hir_map::NodeStructCtor(..)) |
            Some(hir_map::NodeVariant(..)) => {
                let def_id = tcx.hir.local_def_id(id);
                tcx.construct_parameter_environment(tcx.hir.span(id),
                                                    def_id,
                                                    None)
            }
            it => {
                bug!("ParameterEnvironment::from_item(): \
                      `{}` = {:?} is unsupported",
                     tcx.hir.node_to_string(id), it)
            }
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Destructor {
    /// The def-id of the destructor method
    pub did: DefId,
}

bitflags! {
    flags AdtFlags: u32 {
        const NO_ADT_FLAGS        = 0,
        const IS_ENUM             = 1 << 0,
        const IS_PHANTOM_DATA     = 1 << 1,
        const IS_FUNDAMENTAL      = 1 << 2,
        const IS_UNION            = 1 << 3,
        const IS_BOX              = 1 << 4,
    }
}

#[derive(Debug)]
pub struct VariantDef {
    /// The variant's DefId. If this is a tuple-like struct,
    /// this is the DefId of the struct's ctor.
    pub did: DefId,
    pub name: Name, // struct's name if this is a struct
    pub discr: VariantDiscr,
    pub fields: Vec<FieldDef>,
    pub ctor_kind: CtorKind,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, RustcEncodable, RustcDecodable)]
pub enum VariantDiscr {
    /// Explicit value for this variant, i.e. `X = 123`.
    /// The `DefId` corresponds to the embedded constant.
    Explicit(DefId),

    /// The previous variant's discriminant plus one.
    /// For efficiency reasons, the distance from the
    /// last `Explicit` discriminant is being stored,
    /// or `0` for the first variant, if it has none.
    Relative(usize),
}

#[derive(Debug)]
pub struct FieldDef {
    pub did: DefId,
    pub name: Name,
    pub vis: Visibility,
}

/// The definition of an abstract data type - a struct or enum.
///
/// These are all interned (by intern_adt_def) into the adt_defs
/// table.
pub struct AdtDef {
    pub did: DefId,
    pub variants: Vec<VariantDef>,
    flags: AdtFlags,
    pub repr: ReprOptions,
}

impl PartialEq for AdtDef {
    // AdtDef are always interned and this is part of TyS equality
    #[inline]
    fn eq(&self, other: &Self) -> bool { self as *const _ == other as *const _ }
}

impl Eq for AdtDef {}

impl Hash for AdtDef {
    #[inline]
    fn hash<H: Hasher>(&self, s: &mut H) {
        (self as *const AdtDef).hash(s)
    }
}

impl<'tcx> serialize::UseSpecializedEncodable for &'tcx AdtDef {
    fn default_encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
        self.did.encode(s)
    }
}

impl<'tcx> serialize::UseSpecializedDecodable for &'tcx AdtDef {}


impl<'a, 'tcx> HashStable<StableHashingContext<'a, 'tcx>> for AdtDef {
    fn hash_stable<W: StableHasherResult>(&self,
                                          hcx: &mut StableHashingContext<'a, 'tcx>,
                                          hasher: &mut StableHasher<W>) {
        let ty::AdtDef {
            did,
            ref variants,
            ref flags,
            ref repr,
        } = *self;

        did.hash_stable(hcx, hasher);
        variants.hash_stable(hcx, hasher);
        flags.hash_stable(hcx, hasher);
        repr.hash_stable(hcx, hasher);
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum AdtKind { Struct, Union, Enum }

bitflags! {
    #[derive(RustcEncodable, RustcDecodable, Default)]
    flags ReprFlags: u8 {
        const IS_C               = 1 << 0,
        const IS_PACKED          = 1 << 1,
        const IS_SIMD            = 1 << 2,
        // Internal only for now. If true, don't reorder fields.
        const IS_LINEAR          = 1 << 3,

        // Any of these flags being set prevent field reordering optimisation.
        const IS_UNOPTIMISABLE   = ReprFlags::IS_C.bits |
                                   ReprFlags::IS_PACKED.bits |
                                   ReprFlags::IS_SIMD.bits |
                                   ReprFlags::IS_LINEAR.bits,
    }
}

impl_stable_hash_for!(struct ReprFlags {
    bits
});



/// Represents the repr options provided by the user,
#[derive(Copy, Clone, Eq, PartialEq, RustcEncodable, RustcDecodable, Default)]
pub struct ReprOptions {
    pub int: Option<attr::IntType>,
    pub align: u16,
    pub flags: ReprFlags,
}

impl_stable_hash_for!(struct ReprOptions {
    align,
    int,
    flags
});

impl ReprOptions {
    pub fn new(tcx: TyCtxt, did: DefId) -> ReprOptions {
        let mut flags = ReprFlags::empty();
        let mut size = None;
        let mut max_align = 0;
        for attr in tcx.get_attrs(did).iter() {
            for r in attr::find_repr_attrs(tcx.sess.diagnostic(), attr) {
                flags.insert(match r {
                    attr::ReprExtern => ReprFlags::IS_C,
                    attr::ReprPacked => ReprFlags::IS_PACKED,
                    attr::ReprSimd => ReprFlags::IS_SIMD,
                    attr::ReprInt(i) => {
                        size = Some(i);
                        ReprFlags::empty()
                    },
                    attr::ReprAlign(align) => {
                        max_align = cmp::max(align, max_align);
                        ReprFlags::empty()
                    },
                });
            }
        }

        // FIXME(eddyb) This is deprecated and should be removed.
        if tcx.has_attr(did, "simd") {
            flags.insert(ReprFlags::IS_SIMD);
        }

        // This is here instead of layout because the choice must make it into metadata.
        if !tcx.consider_optimizing(|| format!("Reorder fields of {:?}", tcx.item_path_str(did))) {
            flags.insert(ReprFlags::IS_LINEAR);
        }
        ReprOptions { int: size, align: max_align, flags: flags }
    }

    #[inline]
    pub fn simd(&self) -> bool { self.flags.contains(ReprFlags::IS_SIMD) }
    #[inline]
    pub fn c(&self) -> bool { self.flags.contains(ReprFlags::IS_C) }
    #[inline]
    pub fn packed(&self) -> bool { self.flags.contains(ReprFlags::IS_PACKED) }
    #[inline]
    pub fn linear(&self) -> bool { self.flags.contains(ReprFlags::IS_LINEAR) }

    pub fn discr_type(&self) -> attr::IntType {
        self.int.unwrap_or(attr::SignedInt(ast::IntTy::Is))
    }

    /// Returns true if this `#[repr()]` should inhabit "smart enum
    /// layout" optimizations, such as representing `Foo<&T>` as a
    /// single pointer.
    pub fn inhibit_enum_layout_opt(&self) -> bool {
        self.c() || self.int.is_some()
    }
}

impl<'a, 'gcx, 'tcx> AdtDef {
    fn new(tcx: TyCtxt,
           did: DefId,
           kind: AdtKind,
           variants: Vec<VariantDef>,
           repr: ReprOptions) -> Self {
        let mut flags = AdtFlags::NO_ADT_FLAGS;
        let attrs = tcx.get_attrs(did);
        if attr::contains_name(&attrs, "fundamental") {
            flags = flags | AdtFlags::IS_FUNDAMENTAL;
        }
        if Some(did) == tcx.lang_items.phantom_data() {
            flags = flags | AdtFlags::IS_PHANTOM_DATA;
        }
        if Some(did) == tcx.lang_items.owned_box() {
            flags = flags | AdtFlags::IS_BOX;
        }
        match kind {
            AdtKind::Enum => flags = flags | AdtFlags::IS_ENUM,
            AdtKind::Union => flags = flags | AdtFlags::IS_UNION,
            AdtKind::Struct => {}
        }
        AdtDef {
            did: did,
            variants: variants,
            flags: flags,
            repr: repr,
        }
    }

    #[inline]
    pub fn is_struct(&self) -> bool {
        !self.is_union() && !self.is_enum()
    }

    #[inline]
    pub fn is_union(&self) -> bool {
        self.flags.intersects(AdtFlags::IS_UNION)
    }

    #[inline]
    pub fn is_enum(&self) -> bool {
        self.flags.intersects(AdtFlags::IS_ENUM)
    }

    /// Returns the kind of the ADT - Struct or Enum.
    #[inline]
    pub fn adt_kind(&self) -> AdtKind {
        if self.is_enum() {
            AdtKind::Enum
        } else if self.is_union() {
            AdtKind::Union
        } else {
            AdtKind::Struct
        }
    }

    pub fn descr(&self) -> &'static str {
        match self.adt_kind() {
            AdtKind::Struct => "struct",
            AdtKind::Union => "union",
            AdtKind::Enum => "enum",
        }
    }

    pub fn variant_descr(&self) -> &'static str {
        match self.adt_kind() {
            AdtKind::Struct => "struct",
            AdtKind::Union => "union",
            AdtKind::Enum => "variant",
        }
    }

    /// Returns whether this type is #[fundamental] for the purposes
    /// of coherence checking.
    #[inline]
    pub fn is_fundamental(&self) -> bool {
        self.flags.intersects(AdtFlags::IS_FUNDAMENTAL)
    }

    /// Returns true if this is PhantomData<T>.
    #[inline]
    pub fn is_phantom_data(&self) -> bool {
        self.flags.intersects(AdtFlags::IS_PHANTOM_DATA)
    }

    /// Returns true if this is Box<T>.
    #[inline]
    pub fn is_box(&self) -> bool {
        self.flags.intersects(AdtFlags::IS_BOX)
    }

    /// Returns whether this type has a destructor.
    pub fn has_dtor(&self, tcx: TyCtxt<'a, 'gcx, 'tcx>) -> bool {
        self.destructor(tcx).is_some()
    }

    /// Asserts this is a struct and returns the struct's unique
    /// variant.
    pub fn struct_variant(&self) -> &VariantDef {
        assert!(!self.is_enum());
        &self.variants[0]
    }

    #[inline]
    pub fn predicates(&self, tcx: TyCtxt<'a, 'gcx, 'tcx>) -> GenericPredicates<'gcx> {
        tcx.predicates_of(self.did)
    }

    /// Returns an iterator over all fields contained
    /// by this ADT.
    #[inline]
    pub fn all_fields<'s>(&'s self) -> impl Iterator<Item = &'s FieldDef> {
        self.variants.iter().flat_map(|v| v.fields.iter())
    }

    #[inline]
    pub fn is_univariant(&self) -> bool {
        self.variants.len() == 1
    }

    pub fn is_payloadfree(&self) -> bool {
        !self.variants.is_empty() &&
            self.variants.iter().all(|v| v.fields.is_empty())
    }

    pub fn variant_with_id(&self, vid: DefId) -> &VariantDef {
        self.variants
            .iter()
            .find(|v| v.did == vid)
            .expect("variant_with_id: unknown variant")
    }

    pub fn variant_index_with_id(&self, vid: DefId) -> usize {
        self.variants
            .iter()
            .position(|v| v.did == vid)
            .expect("variant_index_with_id: unknown variant")
    }

    pub fn variant_of_def(&self, def: Def) -> &VariantDef {
        match def {
            Def::Variant(vid) | Def::VariantCtor(vid, ..) => self.variant_with_id(vid),
            Def::Struct(..) | Def::StructCtor(..) | Def::Union(..) |
            Def::TyAlias(..) | Def::AssociatedTy(..) | Def::SelfTy(..) => self.struct_variant(),
            _ => bug!("unexpected def {:?} in variant_of_def", def)
        }
    }

    #[inline]
    pub fn discriminants(&'a self, tcx: TyCtxt<'a, 'gcx, 'tcx>)
                         -> impl Iterator<Item=ConstInt> + 'a {
        let repr_type = self.repr.discr_type();
        let initial = repr_type.initial_discriminant(tcx.global_tcx());
        let mut prev_discr = None::<ConstInt>;
        self.variants.iter().map(move |v| {
            let mut discr = prev_discr.map_or(initial, |d| d.wrap_incr());
            if let VariantDiscr::Explicit(expr_did) = v.discr {
                let substs = Substs::empty();
                match tcx.const_eval((expr_did, substs)) {
                    Ok(ConstVal::Integral(v)) => {
                        discr = v;
                    }
                    err => {
                        if !expr_did.is_local() {
                            span_bug!(tcx.def_span(expr_did),
                                "variant discriminant evaluation succeeded \
                                 in its crate but failed locally: {:?}", err);
                        }
                    }
                }
            }
            prev_discr = Some(discr);

            discr
        })
    }

    /// Compute the discriminant value used by a specific variant.
    /// Unlike `discriminants`, this is (amortized) constant-time,
    /// only doing at most one query for evaluating an explicit
    /// discriminant (the last one before the requested variant),
    /// assuming there are no constant-evaluation errors there.
    pub fn discriminant_for_variant(&self,
                                    tcx: TyCtxt<'a, 'gcx, 'tcx>,
                                    variant_index: usize)
                                    -> ConstInt {
        let repr_type = self.repr.discr_type();
        let mut explicit_value = repr_type.initial_discriminant(tcx.global_tcx());
        let mut explicit_index = variant_index;
        loop {
            match self.variants[explicit_index].discr {
                ty::VariantDiscr::Relative(0) => break,
                ty::VariantDiscr::Relative(distance) => {
                    explicit_index -= distance;
                }
                ty::VariantDiscr::Explicit(expr_did) => {
                    let substs = Substs::empty();
                    match tcx.const_eval((expr_did, substs)) {
                        Ok(ConstVal::Integral(v)) => {
                            explicit_value = v;
                            break;
                        }
                        err => {
                            if !expr_did.is_local() {
                                span_bug!(tcx.def_span(expr_did),
                                    "variant discriminant evaluation succeeded \
                                     in its crate but failed locally: {:?}", err);
                            }
                            if explicit_index == 0 {
                                break;
                            }
                            explicit_index -= 1;
                        }
                    }
                }
            }
        }
        let discr = explicit_value.to_u128_unchecked()
            .wrapping_add((variant_index - explicit_index) as u128);
        match repr_type {
            attr::UnsignedInt(ty) => {
                ConstInt::new_unsigned_truncating(discr, ty,
                                                  tcx.sess.target.uint_type)
            }
            attr::SignedInt(ty) => {
                ConstInt::new_signed_truncating(discr as i128, ty,
                                                tcx.sess.target.int_type)
            }
        }
    }

    pub fn destructor(&self, tcx: TyCtxt<'a, 'gcx, 'tcx>) -> Option<Destructor> {
        tcx.adt_destructor(self.did)
    }

    /// Returns a list of types such that `Self: Sized` if and only
    /// if that type is Sized, or `TyErr` if this type is recursive.
    ///
    /// Oddly enough, checking that the sized-constraint is Sized is
    /// actually more expressive than checking all members:
    /// the Sized trait is inductive, so an associated type that references
    /// Self would prevent its containing ADT from being Sized.
    ///
    /// Due to normalization being eager, this applies even if
    /// the associated type is behind a pointer, e.g. issue #31299.
    pub fn sized_constraint(&self, tcx: TyCtxt<'a, 'gcx, 'tcx>) -> &'tcx [Ty<'tcx>] {
        match queries::adt_sized_constraint::try_get(tcx, DUMMY_SP, self.did) {
            Ok(tys) => tys,
            Err(_) => {
                debug!("adt_sized_constraint: {:?} is recursive", self);
                // This should be reported as an error by `check_representable`.
                //
                // Consider the type as Sized in the meanwhile to avoid
                // further errors.
                tcx.intern_type_list(&[tcx.types.err])
            }
        }
    }

    fn sized_constraint_for_ty(&self,
                               tcx: TyCtxt<'a, 'tcx, 'tcx>,
                               ty: Ty<'tcx>)
                               -> Vec<Ty<'tcx>> {
        let result = match ty.sty {
            TyBool | TyChar | TyInt(..) | TyUint(..) | TyFloat(..) |
            TyRawPtr(..) | TyRef(..) | TyFnDef(..) | TyFnPtr(_) |
            TyArray(..) | TyClosure(..) | TyNever => {
                vec![]
            }

            TyStr | TyDynamic(..) | TySlice(_) | TyError => {
                // these are never sized - return the target type
                vec![ty]
            }

            TyTuple(ref tys, _) => {
                match tys.last() {
                    None => vec![],
                    Some(ty) => self.sized_constraint_for_ty(tcx, ty)
                }
            }

            TyAdt(adt, substs) => {
                // recursive case
                let adt_tys = adt.sized_constraint(tcx);
                debug!("sized_constraint_for_ty({:?}) intermediate = {:?}",
                       ty, adt_tys);
                adt_tys.iter()
                    .map(|ty| ty.subst(tcx, substs))
                    .flat_map(|ty| self.sized_constraint_for_ty(tcx, ty))
                    .collect()
            }

            TyProjection(..) | TyAnon(..) => {
                // must calculate explicitly.
                // FIXME: consider special-casing always-Sized projections
                vec![ty]
            }

            TyParam(..) => {
                // perf hack: if there is a `T: Sized` bound, then
                // we know that `T` is Sized and do not need to check
                // it on the impl.

                let sized_trait = match tcx.lang_items.sized_trait() {
                    Some(x) => x,
                    _ => return vec![ty]
                };
                let sized_predicate = Binder(TraitRef {
                    def_id: sized_trait,
                    substs: tcx.mk_substs_trait(ty, &[])
                }).to_predicate();
                let predicates = tcx.predicates_of(self.did).predicates;
                if predicates.into_iter().any(|p| p == sized_predicate) {
                    vec![]
                } else {
                    vec![ty]
                }
            }

            TyInfer(..) => {
                bug!("unexpected type `{:?}` in sized_constraint_for_ty",
                     ty)
            }
        };
        debug!("sized_constraint_for_ty({:?}) = {:?}", ty, result);
        result
    }
}

impl<'a, 'gcx, 'tcx> VariantDef {
    #[inline]
    pub fn find_field_named(&self,
                            name: ast::Name)
                            -> Option<&FieldDef> {
        self.fields.iter().find(|f| f.name == name)
    }

    #[inline]
    pub fn index_of_field_named(&self,
                                name: ast::Name)
                                -> Option<usize> {
        self.fields.iter().position(|f| f.name == name)
    }

    #[inline]
    pub fn field_named(&self, name: ast::Name) -> &FieldDef {
        self.find_field_named(name).unwrap()
    }
}

impl<'a, 'gcx, 'tcx> FieldDef {
    pub fn ty(&self, tcx: TyCtxt<'a, 'gcx, 'tcx>, subst: &Substs<'tcx>) -> Ty<'tcx> {
        tcx.type_of(self.did).subst(tcx, subst)
    }
}

/// Records the substitutions used to translate the polytype for an
/// item into the monotype of an item reference.
#[derive(Clone, RustcEncodable, RustcDecodable)]
pub struct ItemSubsts<'tcx> {
    pub substs: &'tcx Substs<'tcx>,
}

#[derive(Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash, Debug, RustcEncodable, RustcDecodable)]
pub enum ClosureKind {
    // Warning: Ordering is significant here! The ordering is chosen
    // because the trait Fn is a subtrait of FnMut and so in turn, and
    // hence we order it so that Fn < FnMut < FnOnce.
    Fn,
    FnMut,
    FnOnce,
}

impl<'a, 'tcx> ClosureKind {
    pub fn trait_did(&self, tcx: TyCtxt<'a, 'tcx, 'tcx>) -> DefId {
        match *self {
            ClosureKind::Fn => tcx.require_lang_item(FnTraitLangItem),
            ClosureKind::FnMut => {
                tcx.require_lang_item(FnMutTraitLangItem)
            }
            ClosureKind::FnOnce => {
                tcx.require_lang_item(FnOnceTraitLangItem)
            }
        }
    }

    /// True if this a type that impls this closure kind
    /// must also implement `other`.
    pub fn extends(self, other: ty::ClosureKind) -> bool {
        match (self, other) {
            (ClosureKind::Fn, ClosureKind::Fn) => true,
            (ClosureKind::Fn, ClosureKind::FnMut) => true,
            (ClosureKind::Fn, ClosureKind::FnOnce) => true,
            (ClosureKind::FnMut, ClosureKind::FnMut) => true,
            (ClosureKind::FnMut, ClosureKind::FnOnce) => true,
            (ClosureKind::FnOnce, ClosureKind::FnOnce) => true,
            _ => false,
        }
    }
}

impl<'tcx> TyS<'tcx> {
    /// Iterator that walks `self` and any types reachable from
    /// `self`, in depth-first order. Note that just walks the types
    /// that appear in `self`, it does not descend into the fields of
    /// structs or variants. For example:
    ///
    /// ```notrust
    /// isize => { isize }
    /// Foo<Bar<isize>> => { Foo<Bar<isize>>, Bar<isize>, isize }
    /// [isize] => { [isize], isize }
    /// ```
    pub fn walk(&'tcx self) -> TypeWalker<'tcx> {
        TypeWalker::new(self)
    }

    /// Iterator that walks the immediate children of `self`.  Hence
    /// `Foo<Bar<i32>, u32>` yields the sequence `[Bar<i32>, u32]`
    /// (but not `i32`, like `walk`).
    pub fn walk_shallow(&'tcx self) -> AccIntoIter<walk::TypeWalkerArray<'tcx>> {
        walk::walk_shallow(self)
    }

    /// Walks `ty` and any types appearing within `ty`, invoking the
    /// callback `f` on each type. If the callback returns false, then the
    /// children of the current type are ignored.
    ///
    /// Note: prefer `ty.walk()` where possible.
    pub fn maybe_walk<F>(&'tcx self, mut f: F)
        where F : FnMut(Ty<'tcx>) -> bool
    {
        let mut walker = self.walk();
        while let Some(ty) = walker.next() {
            if !f(ty) {
                walker.skip_current_subtree();
            }
        }
    }
}

impl<'tcx> ItemSubsts<'tcx> {
    pub fn is_noop(&self) -> bool {
        self.substs.is_noop()
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LvaluePreference {
    PreferMutLvalue,
    NoPreference
}

impl LvaluePreference {
    pub fn from_mutbl(m: hir::Mutability) -> Self {
        match m {
            hir::MutMutable => PreferMutLvalue,
            hir::MutImmutable => NoPreference,
        }
    }
}

impl BorrowKind {
    pub fn from_mutbl(m: hir::Mutability) -> BorrowKind {
        match m {
            hir::MutMutable => MutBorrow,
            hir::MutImmutable => ImmBorrow,
        }
    }

    /// Returns a mutability `m` such that an `&m T` pointer could be used to obtain this borrow
    /// kind. Because borrow kinds are richer than mutabilities, we sometimes have to pick a
    /// mutability that is stronger than necessary so that it at least *would permit* the borrow in
    /// question.
    pub fn to_mutbl_lossy(self) -> hir::Mutability {
        match self {
            MutBorrow => hir::MutMutable,
            ImmBorrow => hir::MutImmutable,

            // We have no type corresponding to a unique imm borrow, so
            // use `&mut`. It gives all the capabilities of an `&uniq`
            // and hence is a safe "over approximation".
            UniqueImmBorrow => hir::MutMutable,
        }
    }

    pub fn to_user_str(&self) -> &'static str {
        match *self {
            MutBorrow => "mutable",
            ImmBorrow => "immutable",
            UniqueImmBorrow => "uniquely immutable",
        }
    }
}

#[derive(Debug, Clone)]
pub enum Attributes<'gcx> {
    Owned(Rc<[ast::Attribute]>),
    Borrowed(&'gcx [ast::Attribute])
}

impl<'gcx> ::std::ops::Deref for Attributes<'gcx> {
    type Target = [ast::Attribute];

    fn deref(&self) -> &[ast::Attribute] {
        match self {
            &Attributes::Owned(ref data) => &data,
            &Attributes::Borrowed(data) => data
        }
    }
}

impl<'a, 'gcx, 'tcx> TyCtxt<'a, 'gcx, 'tcx> {
    pub fn body_tables(self, body: hir::BodyId) -> &'gcx TypeckTables<'gcx> {
        self.typeck_tables_of(self.hir.body_owner_def_id(body))
    }

    /// Returns an iterator of the def-ids for all body-owners in this
    /// crate. If you would prefer to iterate over the bodies
    /// themselves, you can do `self.hir.krate().body_ids.iter()`.
    pub fn body_owners(self) -> impl Iterator<Item = DefId> + 'a {
        self.hir.krate()
                .body_ids
                .iter()
                .map(move |&body_id| self.hir.body_owner_def_id(body_id))
    }

    pub fn expr_span(self, id: NodeId) -> Span {
        match self.hir.find(id) {
            Some(hir_map::NodeExpr(e)) => {
                e.span
            }
            Some(f) => {
                bug!("Node id {} is not an expr: {:?}", id, f);
            }
            None => {
                bug!("Node id {} is not present in the node map", id);
            }
        }
    }

    pub fn local_var_name_str(self, id: NodeId) -> InternedString {
        match self.hir.find(id) {
            Some(hir_map::NodeLocal(pat)) => {
                match pat.node {
                    hir::PatKind::Binding(_, _, ref path1, _) => path1.node.as_str(),
                    _ => {
                        bug!("Variable id {} maps to {:?}, not local", id, pat);
                    },
                }
            },
            r => bug!("Variable id {} maps to {:?}, not local", id, r),
        }
    }

    pub fn expr_is_lval(self, expr: &hir::Expr) -> bool {
         match expr.node {
            hir::ExprPath(hir::QPath::Resolved(_, ref path)) => {
                match path.def {
                    Def::Local(..) | Def::Upvar(..) | Def::Static(..) | Def::Err => true,
                    _ => false,
                }
            }

            hir::ExprType(ref e, _) => {
                self.expr_is_lval(e)
            }

            hir::ExprUnary(hir::UnDeref, _) |
            hir::ExprField(..) |
            hir::ExprTupField(..) |
            hir::ExprIndex(..) => {
                true
            }

            // Partially qualified paths in expressions can only legally
            // refer to associated items which are always rvalues.
            hir::ExprPath(hir::QPath::TypeRelative(..)) |

            hir::ExprCall(..) |
            hir::ExprMethodCall(..) |
            hir::ExprStruct(..) |
            hir::ExprTup(..) |
            hir::ExprIf(..) |
            hir::ExprMatch(..) |
            hir::ExprClosure(..) |
            hir::ExprBlock(..) |
            hir::ExprRepeat(..) |
            hir::ExprArray(..) |
            hir::ExprBreak(..) |
            hir::ExprAgain(..) |
            hir::ExprRet(..) |
            hir::ExprWhile(..) |
            hir::ExprLoop(..) |
            hir::ExprAssign(..) |
            hir::ExprInlineAsm(..) |
            hir::ExprAssignOp(..) |
            hir::ExprLit(_) |
            hir::ExprUnary(..) |
            hir::ExprBox(..) |
            hir::ExprAddrOf(..) |
            hir::ExprBinary(..) |
            hir::ExprCast(..) => {
                false
            }
        }
    }

    pub fn provided_trait_methods(self, id: DefId) -> Vec<AssociatedItem> {
        self.associated_items(id)
            .filter(|item| item.kind == AssociatedKind::Method && item.defaultness.has_value())
            .collect()
    }

    pub fn trait_relevant_for_never(self, did: DefId) -> bool {
        self.associated_items(did).any(|item| {
            item.relevant_for_never()
        })
    }

    pub fn opt_associated_item(self, def_id: DefId) -> Option<AssociatedItem> {
        let is_associated_item = if let Some(node_id) = self.hir.as_local_node_id(def_id) {
            match self.hir.get(node_id) {
                hir_map::NodeTraitItem(_) | hir_map::NodeImplItem(_) => true,
                _ => false,
            }
        } else {
            match self.describe_def(def_id).expect("no def for def-id") {
                Def::AssociatedConst(_) | Def::Method(_) | Def::AssociatedTy(_) => true,
                _ => false,
            }
        };

        if is_associated_item {
            Some(self.associated_item(def_id))
        } else {
            None
        }
    }

    fn associated_item_from_trait_item_ref(self,
                                           parent_def_id: DefId,
                                           parent_vis: &hir::Visibility,
                                           trait_item_ref: &hir::TraitItemRef)
                                           -> AssociatedItem {
        let def_id = self.hir.local_def_id(trait_item_ref.id.node_id);
        let (kind, has_self) = match trait_item_ref.kind {
            hir::AssociatedItemKind::Const => (ty::AssociatedKind::Const, false),
            hir::AssociatedItemKind::Method { has_self } => {
                (ty::AssociatedKind::Method, has_self)
            }
            hir::AssociatedItemKind::Type => (ty::AssociatedKind::Type, false),
        };

        AssociatedItem {
            name: trait_item_ref.name,
            kind: kind,
            // Visibility of trait items is inherited from their traits.
            vis: Visibility::from_hir(parent_vis, trait_item_ref.id.node_id, self),
            defaultness: trait_item_ref.defaultness,
            def_id: def_id,
            container: TraitContainer(parent_def_id),
            method_has_self_argument: has_self
        }
    }

    fn associated_item_from_impl_item_ref(self,
                                          parent_def_id: DefId,
                                          impl_item_ref: &hir::ImplItemRef)
                                          -> AssociatedItem {
        let def_id = self.hir.local_def_id(impl_item_ref.id.node_id);
        let (kind, has_self) = match impl_item_ref.kind {
            hir::AssociatedItemKind::Const => (ty::AssociatedKind::Const, false),
            hir::AssociatedItemKind::Method { has_self } => {
                (ty::AssociatedKind::Method, has_self)
            }
            hir::AssociatedItemKind::Type => (ty::AssociatedKind::Type, false),
        };

        ty::AssociatedItem {
            name: impl_item_ref.name,
            kind: kind,
            // Visibility of trait impl items doesn't matter.
            vis: ty::Visibility::from_hir(&impl_item_ref.vis, impl_item_ref.id.node_id, self),
            defaultness: impl_item_ref.defaultness,
            def_id: def_id,
            container: ImplContainer(parent_def_id),
            method_has_self_argument: has_self
        }
    }

    #[inline] // FIXME(#35870) Avoid closures being unexported due to impl Trait.
    pub fn associated_items(self, def_id: DefId)
                            -> impl Iterator<Item = ty::AssociatedItem> + 'a {
        let def_ids = self.associated_item_def_ids(def_id);
        (0..def_ids.len()).map(move |i| self.associated_item(def_ids[i]))
    }

    /// Returns true if the impls are the same polarity and are implementing
    /// a trait which contains no items
    pub fn impls_are_allowed_to_overlap(self, def_id1: DefId, def_id2: DefId) -> bool {
        if !self.sess.features.borrow().overlapping_marker_traits {
            return false;
        }
        let trait1_is_empty = self.impl_trait_ref(def_id1)
            .map_or(false, |trait_ref| {
                self.associated_item_def_ids(trait_ref.def_id).is_empty()
            });
        let trait2_is_empty = self.impl_trait_ref(def_id2)
            .map_or(false, |trait_ref| {
                self.associated_item_def_ids(trait_ref.def_id).is_empty()
            });
        self.impl_polarity(def_id1) == self.impl_polarity(def_id2)
            && trait1_is_empty
            && trait2_is_empty
    }

    // Returns `ty::VariantDef` if `def` refers to a struct,
    // or variant or their constructors, panics otherwise.
    pub fn expect_variant_def(self, def: Def) -> &'tcx VariantDef {
        match def {
            Def::Variant(did) | Def::VariantCtor(did, ..) => {
                let enum_did = self.parent_def_id(did).unwrap();
                self.adt_def(enum_did).variant_with_id(did)
            }
            Def::Struct(did) | Def::Union(did) => {
                self.adt_def(did).struct_variant()
            }
            Def::StructCtor(ctor_did, ..) => {
                let did = self.parent_def_id(ctor_did).expect("struct ctor has no parent");
                self.adt_def(did).struct_variant()
            }
            _ => bug!("expect_variant_def used with unexpected def {:?}", def)
        }
    }

    pub fn def_key(self, id: DefId) -> hir_map::DefKey {
        if id.is_local() {
            self.hir.def_key(id)
        } else {
            self.sess.cstore.def_key(id)
        }
    }

    /// Convert a `DefId` into its fully expanded `DefPath` (every
    /// `DefId` is really just an interned def-path).
    ///
    /// Note that if `id` is not local to this crate, the result will
    ///  be a non-local `DefPath`.
    pub fn def_path(self, id: DefId) -> hir_map::DefPath {
        if id.is_local() {
            self.hir.def_path(id)
        } else {
            self.sess.cstore.def_path(id)
        }
    }

    #[inline]
    pub fn def_path_hash(self, def_id: DefId) -> u64 {
        if def_id.is_local() {
            self.hir.definitions().def_path_hash(def_id.index)
        } else {
            self.sess.cstore.def_path_hash(def_id)
        }
    }

    pub fn vis_is_accessible_from(self, vis: Visibility, block: NodeId) -> bool {
        vis.is_accessible_from(self.hir.local_def_id(self.hir.get_module_parent(block)), self)
    }

    pub fn item_name(self, id: DefId) -> ast::Name {
        if let Some(id) = self.hir.as_local_node_id(id) {
            self.hir.name(id)
        } else if id.index == CRATE_DEF_INDEX {
            self.sess.cstore.original_crate_name(id.krate)
        } else {
            let def_key = self.sess.cstore.def_key(id);
            // The name of a StructCtor is that of its struct parent.
            if let hir_map::DefPathData::StructCtor = def_key.disambiguated_data.data {
                self.item_name(DefId {
                    krate: id.krate,
                    index: def_key.parent.unwrap()
                })
            } else {
                def_key.disambiguated_data.data.get_opt_name().unwrap_or_else(|| {
                    bug!("item_name: no name for {:?}", self.def_path(id));
                })
            }
        }
    }

    /// Return the possibly-auto-generated MIR of a (DefId, Subst) pair.
    pub fn instance_mir(self, instance: ty::InstanceDef<'gcx>)
                        -> &'gcx Mir<'gcx>
    {
        match instance {
            ty::InstanceDef::Item(did) => {
                self.optimized_mir(did)
            }
            ty::InstanceDef::Intrinsic(..) |
            ty::InstanceDef::FnPtrShim(..) |
            ty::InstanceDef::Virtual(..) |
            ty::InstanceDef::ClosureOnceShim { .. } |
            ty::InstanceDef::DropGlue(..) => {
                self.mir_shims(instance)
            }
        }
    }

    /// Given the DefId of an item, returns its MIR, borrowed immutably.
    /// Returns None if there is no MIR for the DefId
    pub fn maybe_optimized_mir(self, did: DefId) -> Option<&'gcx Mir<'gcx>> {
        if self.is_mir_available(did) {
            Some(self.optimized_mir(did))
        } else {
            None
        }
    }

    /// Get the attributes of a definition.
    pub fn get_attrs(self, did: DefId) -> Attributes<'gcx> {
        if let Some(id) = self.hir.as_local_node_id(did) {
            Attributes::Borrowed(self.hir.attrs(id))
        } else {
            Attributes::Owned(self.sess.cstore.item_attrs(did))
        }
    }

    /// Determine whether an item is annotated with an attribute
    pub fn has_attr(self, did: DefId, attr: &str) -> bool {
        self.get_attrs(did).iter().any(|item| item.check_name(attr))
    }

    pub fn trait_has_default_impl(self, trait_def_id: DefId) -> bool {
        let def = self.trait_def(trait_def_id);
        def.flags.get().intersects(TraitFlags::HAS_DEFAULT_IMPL)
    }

    /// Populates the type context with all the implementations for the given
    /// trait if necessary.
    pub fn populate_implementations_for_trait_if_necessary(self, trait_id: DefId) {
        if trait_id.is_local() {
            return
        }

        // The type is not local, hence we are reading this out of
        // metadata and don't need to track edges.
        let _ignore = self.dep_graph.in_ignore();

        let def = self.trait_def(trait_id);
        if def.flags.get().intersects(TraitFlags::HAS_REMOTE_IMPLS) {
            return;
        }

        debug!("populate_implementations_for_trait_if_necessary: searching for {:?}", def);

        for impl_def_id in self.sess.cstore.implementations_of_trait(Some(trait_id)) {
            let trait_ref = self.impl_trait_ref(impl_def_id).unwrap();

            // Record the trait->implementation mapping.
            let parent = self.sess.cstore.impl_parent(impl_def_id).unwrap_or(trait_id);
            def.record_remote_impl(self, impl_def_id, trait_ref, parent);
        }

        def.flags.set(def.flags.get() | TraitFlags::HAS_REMOTE_IMPLS);
    }

    /// Given the def_id of an impl, return the def_id of the trait it implements.
    /// If it implements no trait, return `None`.
    pub fn trait_id_of_impl(self, def_id: DefId) -> Option<DefId> {
        self.impl_trait_ref(def_id).map(|tr| tr.def_id)
    }

    /// If the given def ID describes a method belonging to an impl, return the
    /// ID of the impl that the method belongs to. Otherwise, return `None`.
    pub fn impl_of_method(self, def_id: DefId) -> Option<DefId> {
        let item = if def_id.krate != LOCAL_CRATE {
            if let Some(Def::Method(_)) = self.describe_def(def_id) {
                Some(self.associated_item(def_id))
            } else {
                None
            }
        } else {
            self.opt_associated_item(def_id)
        };

        match item {
            Some(trait_item) => {
                match trait_item.container {
                    TraitContainer(_) => None,
                    ImplContainer(def_id) => Some(def_id),
                }
            }
            None => None
        }
    }

    /// If the given def ID describes an item belonging to a trait,
    /// return the ID of the trait that the trait item belongs to.
    /// Otherwise, return `None`.
    pub fn trait_of_item(self, def_id: DefId) -> Option<DefId> {
        if def_id.krate != LOCAL_CRATE {
            return self.sess.cstore.trait_of_item(def_id);
        }
        self.opt_associated_item(def_id)
            .and_then(|associated_item| {
                match associated_item.container {
                    TraitContainer(def_id) => Some(def_id),
                    ImplContainer(_) => None
                }
            })
    }

    /// Construct a parameter environment suitable for static contexts or other contexts where there
    /// are no free type/lifetime parameters in scope.
    pub fn empty_parameter_environment(self) -> ParameterEnvironment<'tcx> {
        ty::ParameterEnvironment {
            free_substs: self.intern_substs(&[]),
            caller_bounds: Slice::empty(),
            implicit_region_bound: None,
            free_id_outlive: None,
            is_copy_cache: RefCell::new(FxHashMap()),
            is_sized_cache: RefCell::new(FxHashMap()),
            is_freeze_cache: RefCell::new(FxHashMap()),
        }
    }

    /// Constructs and returns a substitution that can be applied to move from
    /// the "outer" view of a type or method to the "inner" view.
    /// In general, this means converting from bound parameters to
    /// free parameters. Since we currently represent bound/free type
    /// parameters in the same way, this only has an effect on regions.
    pub fn construct_free_substs(self,
                                 def_id: DefId,
                                 free_id_outlive: Option<CodeExtent<'gcx>>)
                                 -> &'gcx Substs<'gcx> {

        let substs = Substs::for_item(self.global_tcx(), def_id, |def, _| {
            // map bound 'a => free 'a
            self.global_tcx().mk_region(ReFree(FreeRegion {
                scope: free_id_outlive,
                bound_region: def.to_bound_region()
            }))
        }, |def, _| {
            // map T => T
            self.global_tcx().mk_param_from_def(def)
        });

        debug!("construct_parameter_environment: {:?}", substs);
        substs
    }

    /// See `ParameterEnvironment` struct def'n for details.
    /// If you were using `free_id: NodeId`, you might try `self.region_maps().item_extent(free_id)`
    /// for the `free_id_outlive` parameter. (But note that this is not always quite right.)
    pub fn construct_parameter_environment(self,
                                           span: Span,
                                           def_id: DefId,
                                           free_id_outlive: Option<CodeExtent<'gcx>>)
                                           -> ParameterEnvironment<'gcx>
    {
        //
        // Construct the free substs.
        //

        let free_substs = self.construct_free_substs(def_id, free_id_outlive);

        //
        // Compute the bounds on Self and the type parameters.
        //

        let tcx = self.global_tcx();
        let generic_predicates = tcx.predicates_of(def_id);
        let bounds = generic_predicates.instantiate(tcx, free_substs);
        let bounds = tcx.liberate_late_bound_regions(free_id_outlive, &ty::Binder(bounds));
        let predicates = bounds.predicates;

        // Finally, we have to normalize the bounds in the environment, in
        // case they contain any associated type projections. This process
        // can yield errors if the put in illegal associated types, like
        // `<i32 as Foo>::Bar` where `i32` does not implement `Foo`. We
        // report these errors right here; this doesn't actually feel
        // right to me, because constructing the environment feels like a
        // kind of a "idempotent" action, but I'm not sure where would be
        // a better place. In practice, we construct environments for
        // every fn once during type checking, and we'll abort if there
        // are any errors at that point, so after type checking you can be
        // sure that this will succeed without errors anyway.
        //

        let unnormalized_env = ty::ParameterEnvironment {
            free_substs: free_substs,
            implicit_region_bound: free_id_outlive.map(|f| tcx.mk_region(ty::ReScope(f))),
            caller_bounds: tcx.intern_predicates(&predicates),
            free_id_outlive: free_id_outlive,
            is_copy_cache: RefCell::new(FxHashMap()),
            is_sized_cache: RefCell::new(FxHashMap()),
            is_freeze_cache: RefCell::new(FxHashMap()),
        };

        let body_id = free_id_outlive.map(|f| f.node_id())
                                     .unwrap_or(DUMMY_NODE_ID);
        let cause = traits::ObligationCause::misc(span, body_id);
        traits::normalize_param_env_or_error(tcx, def_id, unnormalized_env, cause)
    }

    pub fn node_scope_region(self, id: NodeId) -> Region<'tcx> {
        self.mk_region(ty::ReScope(self.node_extent(id)))
    }

    pub fn visit_all_item_likes_in_krate<V,F>(self,
                                              dep_node_fn: F,
                                              visitor: &mut V)
        where F: FnMut(DefId) -> DepNode<DefId>, V: ItemLikeVisitor<'gcx>
    {
        dep_graph::visit_all_item_likes_in_krate(self.global_tcx(), dep_node_fn, visitor);
    }

    /// Looks up the span of `impl_did` if the impl is local; otherwise returns `Err`
    /// with the name of the crate containing the impl.
    pub fn span_of_impl(self, impl_did: DefId) -> Result<Span, Symbol> {
        if impl_did.is_local() {
            let node_id = self.hir.as_local_node_id(impl_did).unwrap();
            Ok(self.hir.span(node_id))
        } else {
            Err(self.sess.cstore.crate_name(impl_did.krate))
        }
    }
}

impl<'a, 'gcx, 'tcx> TyCtxt<'a, 'gcx, 'tcx> {
    pub fn with_freevars<T, F>(self, fid: NodeId, f: F) -> T where
        F: FnOnce(&[hir::Freevar]) -> T,
    {
        match self.freevars.borrow().get(&fid) {
            None => f(&[]),
            Some(d) => f(&d[..])
        }
    }
}

fn associated_item<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>, def_id: DefId)
    -> AssociatedItem
{
    let id = tcx.hir.as_local_node_id(def_id).unwrap();
    let parent_id = tcx.hir.get_parent(id);
    let parent_def_id = tcx.hir.local_def_id(parent_id);
    let parent_item = tcx.hir.expect_item(parent_id);
    match parent_item.node {
        hir::ItemImpl(.., ref impl_item_refs) => {
            if let Some(impl_item_ref) = impl_item_refs.iter().find(|i| i.id.node_id == id) {
                let assoc_item = tcx.associated_item_from_impl_item_ref(parent_def_id,
                                                                        impl_item_ref);
                debug_assert_eq!(assoc_item.def_id, def_id);
                return assoc_item;
            }
        }

        hir::ItemTrait(.., ref trait_item_refs) => {
            if let Some(trait_item_ref) = trait_item_refs.iter().find(|i| i.id.node_id == id) {
                let assoc_item = tcx.associated_item_from_trait_item_ref(parent_def_id,
                                                                         &parent_item.vis,
                                                                         trait_item_ref);
                debug_assert_eq!(assoc_item.def_id, def_id);
                return assoc_item;
            }
        }

        _ => { }
    }

    span_bug!(parent_item.span,
              "unexpected parent of trait or impl item or item not found: {:?}",
              parent_item.node)
}

/// Calculates the Sized-constraint.
///
/// In fact, there are only a few options for the types in the constraint:
///     - an obviously-unsized type
///     - a type parameter or projection whose Sizedness can't be known
///     - a tuple of type parameters or projections, if there are multiple
///       such.
///     - a TyError, if a type contained itself. The representability
///       check should catch this case.
fn adt_sized_constraint<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>,
                                  def_id: DefId)
                                  -> &'tcx [Ty<'tcx>] {
    let def = tcx.adt_def(def_id);

    let result = tcx.intern_type_list(&def.variants.iter().flat_map(|v| {
        v.fields.last()
    }).flat_map(|f| {
        def.sized_constraint_for_ty(tcx, tcx.type_of(f.did))
    }).collect::<Vec<_>>());

    debug!("adt_sized_constraint: {:?} => {:?}", def, result);

    result
}

/// Calculates the dtorck constraint for a type.
fn adt_dtorck_constraint<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>,
                                   def_id: DefId)
                                   -> DtorckConstraint<'tcx> {
    let def = tcx.adt_def(def_id);
    let span = tcx.def_span(def_id);
    debug!("dtorck_constraint: {:?}", def);

    if def.is_phantom_data() {
        let result = DtorckConstraint {
            outlives: vec![],
            dtorck_types: vec![
                tcx.mk_param_from_def(&tcx.generics_of(def_id).types[0])
           ]
        };
        debug!("dtorck_constraint: {:?} => {:?}", def, result);
        return result;
    }

    let mut result = def.all_fields()
        .map(|field| tcx.type_of(field.did))
        .map(|fty| tcx.dtorck_constraint_for_ty(span, fty, 0, fty))
        .collect::<Result<DtorckConstraint, ErrorReported>>()
        .unwrap_or(DtorckConstraint::empty());
    result.outlives.extend(tcx.destructor_constraints(def));
    result.dedup();

    debug!("dtorck_constraint: {:?} => {:?}", def, result);

    result
}

fn associated_item_def_ids<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>,
                                     def_id: DefId)
                                     -> Rc<Vec<DefId>> {
    let id = tcx.hir.as_local_node_id(def_id).unwrap();
    let item = tcx.hir.expect_item(id);
    let vec: Vec<_> = match item.node {
        hir::ItemTrait(.., ref trait_item_refs) => {
            trait_item_refs.iter()
                           .map(|trait_item_ref| trait_item_ref.id)
                           .map(|id| tcx.hir.local_def_id(id.node_id))
                           .collect()
        }
        hir::ItemImpl(.., ref impl_item_refs) => {
            impl_item_refs.iter()
                          .map(|impl_item_ref| impl_item_ref.id)
                          .map(|id| tcx.hir.local_def_id(id.node_id))
                          .collect()
        }
        _ => span_bug!(item.span, "associated_item_def_ids: not impl or trait")
    };
    Rc::new(vec)
}

fn def_span<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>, def_id: DefId) -> Span {
    tcx.hir.span_if_local(def_id).unwrap()
}

pub fn provide(providers: &mut ty::maps::Providers) {
    *providers = ty::maps::Providers {
        associated_item,
        associated_item_def_ids,
        adt_sized_constraint,
        adt_dtorck_constraint,
        def_span,
        ..*providers
    };
}

pub fn provide_extern(providers: &mut ty::maps::Providers) {
    *providers = ty::maps::Providers {
        adt_sized_constraint,
        adt_dtorck_constraint,
        ..*providers
    };
}


/// A map for the local crate mapping each type to a vector of its
/// inherent impls. This is not meant to be used outside of coherence;
/// rather, you should request the vector for a specific type via
/// `tcx.inherent_impls(def_id)` so as to minimize your dependencies
/// (constructing this map requires touching the entire crate).
#[derive(Clone, Debug)]
pub struct CrateInherentImpls {
    pub inherent_impls: DefIdMap<Rc<Vec<DefId>>>,
}

/// A set of constraints that need to be satisfied in order for
/// a type to be valid for destruction.
#[derive(Clone, Debug)]
pub struct DtorckConstraint<'tcx> {
    /// Types that are required to be alive in order for this
    /// type to be valid for destruction.
    pub outlives: Vec<ty::subst::Kind<'tcx>>,
    /// Types that could not be resolved: projections and params.
    pub dtorck_types: Vec<Ty<'tcx>>,
}

impl<'tcx> FromIterator<DtorckConstraint<'tcx>> for DtorckConstraint<'tcx>
{
    fn from_iter<I: IntoIterator<Item=DtorckConstraint<'tcx>>>(iter: I) -> Self {
        let mut result = Self::empty();

        for constraint in iter {
            result.outlives.extend(constraint.outlives);
            result.dtorck_types.extend(constraint.dtorck_types);
        }

        result
    }
}


impl<'tcx> DtorckConstraint<'tcx> {
    fn empty() -> DtorckConstraint<'tcx> {
        DtorckConstraint {
            outlives: vec![],
            dtorck_types: vec![]
        }
    }

    fn dedup<'a>(&mut self) {
        let mut outlives = FxHashSet();
        let mut dtorck_types = FxHashSet();

        self.outlives.retain(|&val| outlives.replace(val).is_none());
        self.dtorck_types.retain(|&val| dtorck_types.replace(val).is_none());
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SymbolName {
    // FIXME: we don't rely on interning or equality here - better have
    // this be a `&'tcx str`.
    pub name: InternedString
}

impl Deref for SymbolName {
    type Target = str;

    fn deref(&self) -> &str { &self.name }
}

impl fmt::Display for SymbolName {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.name, fmt)
    }
}
