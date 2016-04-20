// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::fmt::Debug;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, RustcEncodable, RustcDecodable)]
pub enum DepNode<D: Clone + Debug> {
    // The `D` type is "how definitions are identified".
    // During compilation, it is always `DefId`, but when serializing
    // it is mapped to `DefPath`.

    // Represents the `Krate` as a whole (the `hir::Krate` value) (as
    // distinct from the krate module). This is basically a hash of
    // the entire krate, so if you read from `Krate` (e.g., by calling
    // `tcx.map.krate()`), we will have to assume that any change
    // means that you need to be recompiled. This is because the
    // `Krate` value gives you access to all other items. To avoid
    // this fate, do not call `tcx.map.krate()`; instead, prefer
    // wrappers like `tcx.visit_all_items_in_krate()`.  If there is no
    // suitable wrapper, you can use `tcx.dep_graph.ignore()` to gain
    // access to the krate, but you must remember to add suitable
    // edges yourself for the individual items that you read.
    Krate,

    // Represents the HIR node with the given node-id
    Hir(D),

    // Represents different phases in the compiler.
    CrateReader,
    CollectLanguageItems,
    CheckStaticRecursion,
    ResolveLifetimes,
    RegionResolveCrate,
    CheckLoops,
    PluginRegistrar,
    StabilityIndex,
    CollectItem(D),
    Coherence,
    EffectCheck,
    Liveness,
    Resolve,
    EntryPoint,
    CheckEntryFn,
    CoherenceCheckImpl(D),
    CoherenceOverlapCheck(D),
    CoherenceOverlapCheckSpecial(D),
    CoherenceOverlapInherentCheck(D),
    CoherenceOrphanCheck(D),
    Variance,
    WfCheck(D),
    TypeckItemType(D),
    TypeckItemBody(D),
    Dropck,
    DropckImpl(D),
    CheckConst(D),
    Privacy,
    IntrinsicCheck(D),
    MatchCheck(D),
    MirMapConstruction(D),
    MirTypeck(D),
    BorrowCheck(D),
    RvalueCheck(D),
    Reachability,
    DeadCheck,
    StabilityCheck,
    LateLintCheck,
    TransCrate,
    TransCrateItem(D),
    TransInlinedItem(D),
    TransWriteMetadata,

    // Nodes representing bits of computed IR in the tcx. Each shared
    // table in the tcx (or elsewhere) maps to one of these
    // nodes. Often we map multiple tables to the same node if there
    // is no point in distinguishing them (e.g., both the type and
    // predicates for an item wind up in `ItemSignature`). Other
    // times, such as `ImplItems` vs `TraitItemDefIds`, tables which
    // might be mergable are kept distinct because the sets of def-ids
    // to which they apply are disjoint, and hence we might as well
    // have distinct labels for easier debugging.
    ImplOrTraitItems(D),
    ItemSignature(D),
    FieldTy(D),
    TraitItemDefIds(D),
    InherentImpls(D),
    ImplItems(D),

    // The set of impls for a given trait. Ultimately, it would be
    // nice to get more fine-grained here (e.g., to include a
    // simplified type), but we can't do that until we restructure the
    // HIR to distinguish the *header* of an impl from its body.  This
    // is because changes to the header may change the self-type of
    // the impl and hence would require us to be more conservative
    // than changes in the impl body.
    TraitImpls(D),

    // Nodes representing caches. To properly handle a true cache, we
    // don't use a DepTrackingMap, but rather we push a task node.
    // Otherwise the write into the map would be incorrectly
    // attributed to the first task that happened to fill the cache,
    // which would yield an overly conservative dep-graph.
    TraitItems(D),
    ReprHints(D),
    TraitSelect(D),
}

impl<D: Clone + Debug> DepNode<D> {
    /// Used in testing
    pub fn from_label_string(label: &str, data: D) -> Result<DepNode<D>, ()> {
        macro_rules! check {
            ($($name:ident,)*) => {
                match label {
                    $(stringify!($name) => Ok(DepNode::$name(data)),)*
                    _ => Err(())
                }
            }
        }

        check! {
            CollectItem,
            BorrowCheck,
            TransCrateItem,
            TypeckItemType,
            TypeckItemBody,
            ImplOrTraitItems,
            ItemSignature,
            FieldTy,
            TraitItemDefIds,
            InherentImpls,
            ImplItems,
            TraitImpls,
            ReprHints,
        }
    }

    pub fn map_def<E, OP>(&self, mut op: OP) -> Option<DepNode<E>>
        where OP: FnMut(&D) -> Option<E>, E: Clone + Debug
    {
        use self::DepNode::*;

        match *self {
            Krate => Some(Krate),
            CrateReader => Some(CrateReader),
            CollectLanguageItems => Some(CollectLanguageItems),
            CheckStaticRecursion => Some(CheckStaticRecursion),
            ResolveLifetimes => Some(ResolveLifetimes),
            RegionResolveCrate => Some(RegionResolveCrate),
            CheckLoops => Some(CheckLoops),
            PluginRegistrar => Some(PluginRegistrar),
            StabilityIndex => Some(StabilityIndex),
            Coherence => Some(Coherence),
            EffectCheck => Some(EffectCheck),
            Liveness => Some(Liveness),
            Resolve => Some(Resolve),
            EntryPoint => Some(EntryPoint),
            CheckEntryFn => Some(CheckEntryFn),
            Variance => Some(Variance),
            Dropck => Some(Dropck),
            Privacy => Some(Privacy),
            Reachability => Some(Reachability),
            DeadCheck => Some(DeadCheck),
            StabilityCheck => Some(StabilityCheck),
            LateLintCheck => Some(LateLintCheck),
            TransCrate => Some(TransCrate),
            TransWriteMetadata => Some(TransWriteMetadata),
            Hir(ref d) => op(d).map(Hir),
            CollectItem(ref d) => op(d).map(CollectItem),
            CoherenceCheckImpl(ref d) => op(d).map(CoherenceCheckImpl),
            CoherenceOverlapCheck(ref d) => op(d).map(CoherenceOverlapCheck),
            CoherenceOverlapCheckSpecial(ref d) => op(d).map(CoherenceOverlapCheckSpecial),
            CoherenceOverlapInherentCheck(ref d) => op(d).map(CoherenceOverlapInherentCheck),
            CoherenceOrphanCheck(ref d) => op(d).map(CoherenceOrphanCheck),
            WfCheck(ref d) => op(d).map(WfCheck),
            TypeckItemType(ref d) => op(d).map(TypeckItemType),
            TypeckItemBody(ref d) => op(d).map(TypeckItemBody),
            DropckImpl(ref d) => op(d).map(DropckImpl),
            CheckConst(ref d) => op(d).map(CheckConst),
            IntrinsicCheck(ref d) => op(d).map(IntrinsicCheck),
            MatchCheck(ref d) => op(d).map(MatchCheck),
            MirMapConstruction(ref d) => op(d).map(MirMapConstruction),
            MirTypeck(ref d) => op(d).map(MirTypeck),
            BorrowCheck(ref d) => op(d).map(BorrowCheck),
            RvalueCheck(ref d) => op(d).map(RvalueCheck),
            TransCrateItem(ref d) => op(d).map(TransCrateItem),
            TransInlinedItem(ref d) => op(d).map(TransInlinedItem),
            ImplOrTraitItems(ref d) => op(d).map(ImplOrTraitItems),
            ItemSignature(ref d) => op(d).map(ItemSignature),
            FieldTy(ref d) => op(d).map(FieldTy),
            TraitItemDefIds(ref d) => op(d).map(TraitItemDefIds),
            InherentImpls(ref d) => op(d).map(InherentImpls),
            ImplItems(ref d) => op(d).map(ImplItems),
            TraitImpls(ref d) => op(d).map(TraitImpls),
            TraitItems(ref d) => op(d).map(TraitItems),
            ReprHints(ref d) => op(d).map(ReprHints),
            TraitSelect(ref d) => op(d).map(TraitSelect),
        }
    }
}
