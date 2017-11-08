//! Change representation.
//!
//! This module provides data types to represent, store and record changes found in various
//! analysis passes. We distinguish between path changes and regular changes, which represent
//! changes to the export structure of the crate and to specific items, respectively. The
//! ordering of changes and output generation is performed using the span information contained
//! in these data structures. This means that we try to use the old span only when no other span
//! is available, which leads to (complete) removals being displayed first. Matters are further
//! complicated by the fact that we still group changes by the item they refer to, even if it's
//! path changes.

use rustc::hir::def_id::DefId;
use rustc::session::Session;
use rustc::ty::Predicate;
use rustc::ty::error::TypeError;

use semver::Version;

use std::collections::{BTreeSet, BTreeMap, HashMap};
use std::cmp::Ordering;
use std::fmt;

use syntax::symbol::Symbol;

use syntax_pos::Span;

/// The categories we use when analyzing changes between crate versions.
///
/// These directly correspond to the semantic versioning spec, with the exception that some
/// breaking changes are categorized as "technically breaking" - that is, [1] defines them as
/// non-breaking when introduced to the standard libraries, because they only cause breakage in
/// exotic and/or unlikely scenarios, while we have a separate category for them.
///
/// [1]: https://github.com/rust-lang/rfcs/blob/master/text/1105-api-evolution.md
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChangeCategory {
    /// A patch-level change - no change to the public API of a crate.
    Patch,
    /// A non-breaking, backwards-compatible change.
    NonBreaking,
    /// A breaking change that only causes breakage in well-known exotic cases.
    TechnicallyBreaking,
    /// A breaking, backwards-incompatible change.
    Breaking,
}

pub use self::ChangeCategory::*;

impl<'a> Default for ChangeCategory {
    fn default() -> ChangeCategory {
        Patch
    }
}

impl<'a> fmt::Display for ChangeCategory {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let desc = match *self {
            Patch => "patch",
            NonBreaking => "non-breaking",
            TechnicallyBreaking => "technically breaking",
            Breaking => "breaking",
        };

        write!(f, "{}", desc)
    }
}

/// Different ways to refer to a changed item.
///
/// Used in the header of a change description to identify an item that was subject to change.
pub enum Name {
    /// The changed item's name.
    Symbol(Symbol),
    /// A textutal description of the item, used for trait impls.
    ImplDesc(String),
}

impl<'a> fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Name::Symbol(name) => write!(f, "`{}`", name),
            Name::ImplDesc(ref desc) => {
                write!(f, "`{}`", desc)
            },
        }
    }
}

/// A change record of newly introduced or removed paths to an item.
///
/// NB: `Eq` and `Ord` instances are constructed to only regard the span of the associated item
/// definition. All other spans are only present for later display of the change record.
pub struct PathChange {
    /// The name of the item - this doesn't use `Name` because this change structure only gets
    /// generated for removals and additions of named items, not impls.
    name: Symbol,
    /// The definition span of the item.
    def_span: Span,
    /// The set of spans of added exports of the item.
    additions: BTreeSet<Span>,
    /// The set of spans of removed exports of the item.
    removals: BTreeSet<Span>,
}

impl PathChange {
    /// Construct a new empty path change record for an item.
    fn new(name: Symbol, def_span: Span) -> PathChange {
        PathChange {
            name: name,
            def_span: def_span,
            additions: BTreeSet::new(),
            removals: BTreeSet::new(),
        }
    }

    /// Insert a new span addition or deletion into an existing path change record.
    fn insert(&mut self, span: Span, add: bool) {
        if add {
            self.additions.insert(span);
        } else {
            self.removals.insert(span);
        }
    }

    /// Get the change's category.
    pub fn to_category(&self) -> ChangeCategory {
        if !self.removals.is_empty() {
            Breaking
        } else if !self.additions.is_empty() {
            TechnicallyBreaking
        } else {
            Patch
        }
    }

    /// Get the change item's definition span.
    pub fn span(&self) -> &Span {
        &self.def_span
    }

    /// Report the change in a structured manner, using rustc's error reporting capabilities.
    fn report(&self, session: &Session) {
        let cat = self.to_category();
        if cat == Patch {
            return;
        }

        let msg = format!("path changes to `{}`", self.name);
        let mut builder = if cat == Breaking {
            session.struct_span_err(self.def_span, &msg)
        } else {
            session.struct_span_warn(self.def_span, &msg)
        };

        for removed_span in &self.removals {
            if *removed_span == self.def_span {
                builder.warn("removed definition (breaking)");
            } else {
                builder.span_warn(*removed_span, "removed path (breaking)");
            }
        }

        for added_span in &self.additions {
            if *added_span == self.def_span {
                builder.note("added definition (technically breaking)");
            } else {
                builder.span_note(*added_span, "added path (technically breaking)");
            }
        }

        builder.emit();
    }
}

impl PartialEq for PathChange {
    fn eq(&self, other: &PathChange) -> bool {
        self.span() == other.span()
    }
}

impl Eq for PathChange {}

impl PartialOrd for PathChange {
    fn partial_cmp(&self, other: &PathChange) -> Option<Ordering> {
        self.span().partial_cmp(other.span())
    }
}

impl Ord for PathChange {
    fn cmp(&self, other: &PathChange) -> Ordering {
        self.span().cmp(other.span())
    }
}

/// The types of changes we identify between items present in both crate versions.
#[derive(Clone, Debug)]
pub enum ChangeType<'tcx> {
    /// An item has been made public.
    ItemMadePublic,
    /// An item has been made private.
    ItemMadePrivate,
    /// An item has changed it's kind.
    KindDifference,
    /// A `static` item changed it's mutablity.
    StaticMutabilityChanged { now_mut: bool },
    /// The variance of a type or region parameter has gone from invariant to co- or
    /// contravariant or to bivariant.
    VarianceLoosened,
    /// The variance of a type or region parameter has gone from bivariant to co- or
    /// contravariant or to invariant.
    VarianceTightened,
    /// The variance of a type or region parameter has changed from covariant to contravariant
    /// or vice-versa.
    VarianceChanged { now_contravariant: bool },
    /// A region parameter has been added to an item.
    RegionParameterAdded,
    /// A region parameter has been removed from an item.
    RegionParameterRemoved,
    /// A possibly defaulted type parameter has been added to an item.
    TypeParameterAdded { defaulted: bool },
    /// A possibly defaulted type parameter has been removed from an item.
    TypeParameterRemoved { defaulted: bool },
    /// A variant has been added to an enum.
    VariantAdded,
    /// A variant has been removed from an enum.
    VariantRemoved,
    /// A possibly public field has been added to a variant or struct.
    ///
    /// This also records whether all fields are public were public before the change.
    VariantFieldAdded { public: bool, total_public: bool },
    /// A possibly public field has been removed from a variant or struct.
    ///
    /// This also records whether all fields are public were public before the change.
    VariantFieldRemoved { public: bool, total_public: bool },
    /// A variant or struct has changed it's style.
    ///
    /// The style could have been changed from a tuple variant/struct to a regular
    /// struct/struct variant or vice versa. Whether all fields were private prior to the change
    /// is also recorded.
    VariantStyleChanged { now_struct: bool, total_private: bool },
    /// A function has changed it's constness.
    FnConstChanged { now_const: bool },
    /// A method either gained or lost a `self` parameter.
    MethodSelfChanged { now_self: bool },
    /// A trait's definition added a possibly defaulted item.
    TraitItemAdded { defaulted: bool },
    /// A trait's definition removed a possibly defaulted item.
    TraitItemRemoved { defaulted: bool },
    /// A trait's definition changed it's unsafety.
    TraitUnsafetyChanged { now_unsafe: bool },
    /// An item's type has changed.
    TypeChanged { error: TypeError<'tcx> },
    /// An item's (trait) bounds have been tightened.
    BoundsTightened { pred: Predicate<'tcx> },
    /// An item's (trait) bounds have been loosened.
    ///
    /// This includes information on whether the affected item is a trait definition, since
    /// removing trait bounds on those is *breaking* (as it invalidates the assumption that a
    /// supertrait is implemented for each type implementing the traits).
    BoundsLoosened { pred: Predicate<'tcx>, trait_def: bool },
    /// A trait impl has been specialized or removed for some type(s).
    TraitImplTightened,
    /// A trait impl has been generalized or newly added for some type(s).
    TraitImplLoosened,
    /// An associated item has been newly added to some inherent impls.
    AssociatedItemAdded,
    /// An associated item has been removed from some inherent impls.
    AssociatedItemRemoved,
    /// An unknown change we don't yet explicitly handle.
    Unknown,
}

pub use self::ChangeType::*;

impl<'tcx> ChangeType<'tcx> {
    /// Get the change type's category.
    pub fn to_category(&self) -> ChangeCategory {
        // TODO: slightly messy and unreadable.
        match *self {
            ItemMadePrivate |
            KindDifference |
            StaticMutabilityChanged { now_mut: false } |
            VarianceTightened |
            VarianceChanged { .. } |
            RegionParameterAdded |
            RegionParameterRemoved |
            TypeParameterAdded { defaulted: false } |
            TypeParameterRemoved { .. } |
            VariantAdded |
            VariantRemoved |
            VariantFieldAdded { .. } |
            VariantFieldRemoved { .. } |
            VariantStyleChanged { .. } |
            TypeChanged { .. } |
            FnConstChanged { now_const: false } |
            MethodSelfChanged { now_self: false } |
            TraitItemAdded { defaulted: false } |
            TraitItemRemoved { .. } |
            TraitUnsafetyChanged { .. } |
            BoundsTightened { .. } |
            BoundsLoosened { trait_def: true, .. } |
            TraitImplTightened |
            AssociatedItemRemoved |
            Unknown => Breaking,
            MethodSelfChanged { now_self: true } |
            TraitItemAdded { defaulted: true } |
            BoundsLoosened { trait_def: false, .. } |
            TraitImplLoosened |
            AssociatedItemAdded |
            ItemMadePublic => TechnicallyBreaking,
            StaticMutabilityChanged { now_mut: true } |
            VarianceLoosened |
            TypeParameterAdded { defaulted: true } |
            FnConstChanged { now_const: true } => NonBreaking,
        }
    }
}

impl<'a> fmt::Display for ChangeType<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let desc = match *self {
            ItemMadePublic => "item made public",
            ItemMadePrivate => "item made private",
            KindDifference => "item kind changed",
            StaticMutabilityChanged { now_mut: true } => "static item made mutable",
            StaticMutabilityChanged { now_mut: false } => "static item made immutable",
            VarianceLoosened => "variance loosened",
            VarianceTightened => "variance tightened",
            VarianceChanged { now_contravariant: true } =>
                "variance changed from co- to contravariant",
            VarianceChanged { now_contravariant: false } =>
                "variance changed from contra- to covariant",
            RegionParameterAdded => "region parameter added",
            RegionParameterRemoved => "region parameter removed",
            TypeParameterAdded { defaulted: true } => "defaulted type parameter added",
            TypeParameterAdded { defaulted: false } => "type parameter added",
            TypeParameterRemoved { defaulted: true } => "defaulted type parameter removed",
            TypeParameterRemoved { defaulted: false } => "type parameter removed",
            VariantAdded => "enum variant added",
            VariantRemoved => "enum variant removed",
            VariantFieldAdded { public: true, total_public: true } =>
                "public variant field added to variant with no private fields",
            VariantFieldAdded { public: true, total_public: false } =>
                "public variant field added to variant with private fields",
            VariantFieldAdded { public: false, total_public: true } =>
                "variant field added to variant with no private fields",
            VariantFieldAdded { public: false, total_public: false } =>
                "variant field added to variant with private fields",
            VariantFieldRemoved { public: true, total_public: true } =>
                "public variant field removed from variant with no private fields",
            VariantFieldRemoved { public: true, total_public: false } =>
                "public variant field removed from variant with private fields",
            VariantFieldRemoved { public: false, total_public: true } =>
                "variant field removed from variant with no private fields",
            VariantFieldRemoved { public: false, total_public: false } =>
                "variant field removed from variant with private fields",
            VariantStyleChanged { now_struct: true, total_private: true } =>
                "variant with no public fields changed to a struct variant",
            VariantStyleChanged { now_struct: true, total_private: false } =>
                "variant changed to a struct variant",
            VariantStyleChanged { now_struct: false, total_private: true } =>
                "variant with no public fields changed to a tuple variant",
            VariantStyleChanged { now_struct: false, total_private: false } =>
                "variant changed to a tuple variant",
            FnConstChanged { now_const: true } => "fn item made const",
            FnConstChanged { now_const: false } => "fn item made non-const",
            MethodSelfChanged { now_self: true } => "added self-argument to method",
            MethodSelfChanged { now_self: false } => "removed self-argument from method",
            TraitItemAdded { defaulted: true } => "added defaulted item to trait",
            TraitItemAdded { defaulted: false } => "added item to trait",
            TraitItemRemoved { defaulted: true } => "removed defaulted item from trait",
            TraitItemRemoved { defaulted: false } => "removed item from trait",
            TraitUnsafetyChanged { now_unsafe: true } => "trait made unsafe",
            TraitUnsafetyChanged { now_unsafe: false } => "trait no longer unsafe",
            TypeChanged { ref error } => return write!(f, "type error: {}", error),
            BoundsTightened { ref pred } => return write!(f, "added bound: `{}`", pred),
            BoundsLoosened { ref pred, trait_def } =>
                if trait_def {
                    return write!(f, "removed bound on trait definition: `{}`", pred)
                } else {
                    return write!(f, "removed bound: `{}`", pred)
                },
            TraitImplTightened => "trait impl specialized or removed",
            TraitImplLoosened => "trait impl generalized or newly added",
            AssociatedItemAdded => "added item in inherent impl",
            AssociatedItemRemoved => "removed item in inherent impl",
            Unknown => "unknown change",
        };
        write!(f, "{}", desc)
    }
}

/// A change record of an item present in both crate versions.
///
/// NB: `Eq` and `Ord` instances are constucted to only regard the *new* span of the associated
/// item definition. This allows us to sort them by appearance in the *new* source.
pub struct Change<'tcx> {
    /// The types of changes affecting the item, with optional subspans.
    changes: Vec<(ChangeType<'tcx>, Option<Span>)>,
    /// The most severe change category already recorded for the item.
    max: ChangeCategory,
    /// The name of the item.
    name: Name,
    /// The new definition span of the item.
    new_span: Span,
    /// Whether to output changes. Used to distinguish all-private items.
    output: bool
}

impl<'tcx> Change<'tcx> {
    /// Construct a new empty change record for an item.
    fn new(name: Name, span: Span, output: bool) -> Change<'tcx> {
        Change {
            changes: Vec::new(),
            max: ChangeCategory::default(),
            name: name,
            new_span: span,
            output: output,
        }
    }

    /// Insert another change type into an existing path change record.
    fn insert(&mut self, type_: ChangeType<'tcx>, span: Option<Span>) {
        let cat = type_.to_category();

        if cat > self.max {
            self.max = cat;
        }

        self.changes.push((type_, span));
    }

    /// Check whether a trait item contains breaking changes preventing further analysis of it's
    /// child items.
    ///
    /// NB: The invariant that the item in question is actually a trait item isn't checked.
    fn trait_item_breaking(&self) -> bool {
        for change in &self.changes {
            match change.0 {
                ItemMadePrivate |
                KindDifference |
                RegionParameterRemoved |
                TypeParameterRemoved { .. } |
                VariantAdded |
                VariantRemoved |
                VariantFieldAdded { .. } |
                VariantFieldRemoved { .. } |
                VariantStyleChanged { .. } |
                TypeChanged { .. } |
                FnConstChanged { now_const: false } |
                MethodSelfChanged { now_self: false } |
                Unknown => return true,
                StaticMutabilityChanged { .. } |
                RegionParameterAdded |
                MethodSelfChanged { now_self: true } |
                TraitItemAdded { .. } |
                TraitItemRemoved { .. } |
                ItemMadePublic |
                VarianceLoosened |
                VarianceTightened |
                VarianceChanged { .. } |
                TypeParameterAdded { .. } |
                TraitUnsafetyChanged { .. } |
                FnConstChanged { now_const: true } |
                BoundsTightened { .. } |
                BoundsLoosened { .. } |
                TraitImplTightened |
                TraitImplLoosened |
                AssociatedItemAdded |
                AssociatedItemRemoved => (),
            }
        }

        false
    }

    /// Get the change's category.
    fn to_category(&self) -> ChangeCategory {
        self.max
    }

    /// Get the new span of the change item.
    fn new_span(&self) -> &Span {
        &self.new_span
    }

    /// Report the change in a structured manner, using rustc's error reporting capabilities.
    fn report(&self, session: &Session) {
        if self.max == Patch || !self.output {
            return;
        }

        let msg = format!("{} changes in {}", self.max, self.name);
        let mut builder = if self.max == Breaking {
            session.struct_span_err(self.new_span, &msg)
        } else {
            session.struct_span_warn(self.new_span, &msg)
        };

        for change in &self.changes {
            let cat = change.0.to_category();
            let sub_msg = format!("{} ({})", change.0, cat);
            if let Some(span) = change.1 {
                if cat == Breaking {
                    builder.span_warn(span, &sub_msg);
                } else {
                    builder.span_note(span, &sub_msg,);
                }
            } else if cat == Breaking { // change.1 == None from here on.
                builder.warn(&sub_msg);
            } else {
                builder.note(&sub_msg);
            }
        }

        builder.emit();
    }
}

impl<'tcx> PartialEq for Change<'tcx> {
    fn eq(&self, other: &Change) -> bool {
        self.new_span() == other.new_span()
    }
}

impl<'tcx> Eq for Change<'tcx> {}

impl<'tcx> PartialOrd for Change<'tcx> {
    fn partial_cmp(&self, other: &Change<'tcx>) -> Option<Ordering> {
        self.new_span().partial_cmp(other.new_span())
    }
}

impl<'tcx> Ord for Change<'tcx> {
    fn cmp(&self, other: &Change<'tcx>) -> Ordering {
        self.new_span().cmp(other.new_span())
    }
}

/// The total set of changes recorded for two crate versions.
#[derive(Default)]
pub struct ChangeSet<'tcx> {
    /// The set of currently recorded path changes.
    path_changes: HashMap<DefId, PathChange>,
    /// The set of currently recorded regular changes.
    changes: HashMap<DefId, Change<'tcx>>,
    /// The mapping of spans to changes, for ordering purposes.
    spans: BTreeMap<Span, DefId>,
    /// The most severe change category already recorded.
    max: ChangeCategory,
}

impl<'tcx> ChangeSet<'tcx> {
    /// Add a new path change entry for the given item.
    pub fn new_path_change(&mut self, old: DefId, name: Symbol, def_span: Span) {
        self.spans
            .entry(def_span)
            .or_insert_with(|| old);
        self.path_changes
            .entry(old)
            .or_insert_with(|| PathChange::new(name, def_span));
    }

    /// Add a new path addition to an already existing entry.
    pub fn add_path_addition(&mut self, old: DefId, span: Span) {
        self.add_path(old, span, true);
    }

    /// Add a new path removal to an already existing entry.
    pub fn add_path_removal(&mut self, old: DefId, span: Span) {
        self.add_path(old, span, false);
    }

    /// Add a new path change to an already existing entry.
    fn add_path(&mut self, old: DefId, span: Span, add: bool) {
        let cat = if add { TechnicallyBreaking } else { Breaking };

        if cat > self.max {
            self.max = cat;
        }

        self.path_changes.get_mut(&old).unwrap().insert(span, add);
    }

    /// Add a new change entry for the given item pair.
    pub fn new_change(&mut self,
                      old_def_id: DefId,
                      new_def_id: DefId,
                      name: Symbol,
                      old_span: Span,
                      new_span: Span,
                      output: bool) {
        let change = Change::new(Name::Symbol(name), new_span, output);

        self.spans.insert(old_span, old_def_id);
        self.spans.insert(new_span, new_def_id);
        self.changes.insert(old_def_id, change);
    }

    /// Add a new change entry for the given trait impl.
    pub fn new_change_impl(&mut self,
                           def_id: DefId,
                           desc: String,
                           span: Span) {
        let change = Change::new(Name::ImplDesc(desc), span, true);

        self.spans.insert(span, def_id);
        self.changes.insert(def_id, change);
    }

    /// Add a new change to an already existing entry.
    pub fn add_change(&mut self, type_: ChangeType<'tcx>, old: DefId, span: Option<Span>) {
        let cat = type_.to_category();

        if cat > self.max && self.get_output(old) {
            self.max = cat;
        }

        self.changes.get_mut(&old).unwrap().insert(type_, span);
    }

    /// Check whether the changes associated with a `DefId` will be reported.
    pub fn get_output(&self, old: DefId) -> bool {
        self.changes.get(&old).map_or(true, |change| change.output)
    }

    /// Set up reporting for the changes associated with a given `DefId`.
    pub fn set_output(&mut self, old: DefId) {
        let max = &mut self.max;
        self.changes
            .get_mut(&old)
            .map(|change| {
                let cat = change.to_category();

                if cat > *max {
                    *max = cat;
                }

                change.output = true
            });
    }

    /// Check whether an item with the given id has undergone breaking changes.
    ///
    /// The expected `DefId` is obviously an *old* one.
    pub fn item_breaking(&self, old: DefId) -> bool {
        // we only care about items that were present in both versions.
        self.changes
            .get(&old)
            .map(|change| change.to_category() == Breaking)
            .unwrap_or(false)
    }

    /// Check whether a trait item contains breaking changes preventing further analysis of it's
    /// child items.
    pub fn trait_item_breaking(&self, old: DefId) -> bool {
        self.changes
            .get(&old)
            .map(|change| change.trait_item_breaking())
            .unwrap_or(false)
    }

    /// Format the contents of a change set for user output.
    pub fn output(&self, session: &Session, version: &str) {
        if let Ok(mut new_version) = Version::parse(version) {
            match self.max {
                Patch => new_version.increment_patch(),
                NonBreaking | TechnicallyBreaking => new_version.increment_minor(),
                Breaking => new_version.increment_major(),
            }

            println!("version bump: {} -> ({}) -> {}", version, self.max, new_version);
        } else {
            println!("max change: {}, could not parse {}", self.max, version);
        }

        for key in self.spans.values() {
            if let Some(change) = self.path_changes.get(key) {
                change.report(session);
            }

            if let Some(change) = self.changes.get(key) {
                change.report(session);
            }
        }
    }
}

#[cfg(test)]
pub mod tests {
    use quickcheck::*;
    pub use super::*;

    use rustc::hir::def_id::DefId;

    use std::cmp::{max, min};

    use syntax_pos::BytePos;
    use syntax_pos::hygiene::SyntaxContext;
    use syntax_pos::symbol::Interner;

    /// A wrapper for `Span` that can be randomly generated.
    #[derive(Clone, Debug)]
    pub struct Span_(u32, u32);

    impl Span_ {
        pub fn inner(self) -> Span {
            Span::new(BytePos(self.0), BytePos(self.1), SyntaxContext::empty())
        }
    }

    impl Arbitrary for Span_ {
        fn arbitrary<G: Gen>(g: &mut G) -> Span_ {
            let a: u32 = Arbitrary::arbitrary(g);
            let b: u32 = Arbitrary::arbitrary(g);
            Span_(min(a, b), max(a, b))
        }
    }

    /// A wrapper for `DefId` that can be randomly generated.
    #[derive(Clone, Debug)]
    pub struct DefId_(DefId);

    impl DefId_ {
        pub fn inner(self) -> DefId {
            self.0
        }
    }

    impl Arbitrary for DefId_ {
        fn arbitrary<G: Gen>(g: &mut G) -> DefId_ {
            use rustc::hir::def_id::{DefId, CrateNum, DefIndex};

            let a: u32 = Arbitrary::arbitrary(g);
            let b: u32 = Arbitrary::arbitrary(g);
            DefId_(DefId {
                krate: CrateNum::new(a as usize),
                index: DefIndex::from_u32(b),
            })
        }
    }

    /// a rip-off of the real `ChangeType` that can be randomly generated.
    #[derive(Clone, Debug)]
    pub enum ChangeType_ {
        ItemMadePublic,
        ItemMadePrivate,
        KindDifference,
        RegionParameterAdded,
        RegionParameterRemoved,
        TypeParameterAdded { defaulted: bool },
        TypeParameterRemoved { defaulted: bool },
        VariantAdded,
        VariantRemoved,
        VariantFieldAdded { public: bool, total_public: bool },
        VariantFieldRemoved { public: bool, total_public: bool },
        VariantStyleChanged { now_struct: bool, total_private: bool },
        FnConstChanged { now_const: bool },
        MethodSelfChanged { now_self: bool },
        TraitItemAdded { defaulted: bool },
        TraitItemRemoved { defaulted: bool },
        TraitUnsafetyChanged { now_unsafe: bool },
        Unknown,
    }

    impl ChangeType_ {
        fn inner<'a>(&self) -> ChangeType<'a> {
            match *self {
                ChangeType_::ItemMadePublic => ItemMadePublic,
                ChangeType_::ItemMadePrivate => ItemMadePrivate,
                ChangeType_::KindDifference => KindDifference,
                ChangeType_::RegionParameterAdded => RegionParameterAdded,
                ChangeType_::RegionParameterRemoved => RegionParameterRemoved,
                ChangeType_::TypeParameterAdded { defaulted } =>
                    TypeParameterAdded { defaulted },
                ChangeType_::TypeParameterRemoved { defaulted } =>
                    TypeParameterRemoved { defaulted },
                ChangeType_::VariantAdded => VariantAdded,
                ChangeType_::VariantRemoved => VariantRemoved,
                ChangeType_::VariantFieldAdded { public, total_public } =>
                    VariantFieldAdded { public, total_public },
                ChangeType_::VariantFieldRemoved { public, total_public } =>
                    VariantFieldRemoved { public, total_public },
                ChangeType_::VariantStyleChanged { now_struct, total_private } =>
                    VariantStyleChanged { now_struct, total_private },
                ChangeType_::FnConstChanged { now_const } =>
                    FnConstChanged { now_const },
                ChangeType_::MethodSelfChanged { now_self } =>
                    MethodSelfChanged { now_self },
                ChangeType_::TraitItemAdded { defaulted } =>
                    TraitItemAdded { defaulted },
                ChangeType_::TraitItemRemoved { defaulted } =>
                    TraitItemRemoved { defaulted },
                ChangeType_::TraitUnsafetyChanged { now_unsafe } =>
                    TraitUnsafetyChanged { now_unsafe },
                ChangeType_::Unknown => Unknown,
            }
        }
    }

    impl Arbitrary for ChangeType_ {
        fn arbitrary<G: Gen>(g: &mut G) -> ChangeType_ {
            use self::ChangeType_::*;

            let b1 = Arbitrary::arbitrary(g);
            let b2 = Arbitrary::arbitrary(g);

            g.choose(&[ItemMadePublic,
                       ItemMadePrivate,
                       KindDifference,
                       RegionParameterAdded,
                       RegionParameterRemoved,
                       TypeParameterAdded { defaulted: b1 },
                       TypeParameterRemoved { defaulted: b1 },
                       VariantAdded,
                       VariantRemoved,
                       VariantFieldAdded { public: b1, total_public: b2 },
                       VariantFieldRemoved { public: b1, total_public: b2 },
                       VariantStyleChanged { now_struct: b1, total_private: b2 },
                       FnConstChanged { now_const: b1 },
                       MethodSelfChanged { now_self: b1 },
                       TraitItemAdded { defaulted: b1 },
                       TraitItemRemoved { defaulted: b1 },
                       TraitUnsafetyChanged { now_unsafe: b1 },
                       Unknown]).unwrap().clone()
        }
    }

    /// A wrapper type used to construct `Change`s.
    pub type Change_ = (DefId_, DefId_, Span_, Span_, bool, Vec<(ChangeType_, Option<Span_>)>);

    /// Construct `Change`s from things that can be generated.
    fn build_change<'a>(s1: Span, output: bool, changes: Vec<(ChangeType_, Option<Span_>)>)
        -> Change<'a>
    {
        let mut interner = Interner::new();
        let mut change = Change::new(Name::Symbol(interner.intern("test")), s1, output);

        for (type_, span) in changes {
            change.insert(type_.inner(), span.map(|s| s.inner()));
        }

        change
    }

    /// A wrapper type used to construct `PathChange`s.
    pub type PathChange_ = (DefId_, Span_, Vec<(bool, Span_)>);

    /// Construct `PathChange`s from things that can be generated.
    fn build_path_change(s1: Span, spans: Vec<(bool, Span)>) -> PathChange {
        let mut interner = Interner::new();
        let mut change = PathChange::new(interner.intern("test"), s1);

        for (add, span) in spans {
            change.insert(span, add);
        }

        change
    }

    quickcheck! {
        /// The `Ord` instance of `PathChange` is transitive.
        fn ord_pchange_transitive(c1: PathChange_, c2: PathChange_, c3: PathChange_) -> bool {
            let s1 = c1.2.iter().map(|&(add, ref s)| (add, s.clone().inner())).collect();
            let s2 = c2.2.iter().map(|&(add, ref s)| (add, s.clone().inner())).collect();
            let s3 = c3.2.iter().map(|&(add, ref s)| (add, s.clone().inner())).collect();

            let ch1 = build_path_change(c1.1.inner(), s1);
            let ch2 = build_path_change(c2.1.inner(), s2);
            let ch3 = build_path_change(c3.1.inner(), s3);

            let mut res = true;

            if ch1 < ch2 && ch2 < ch3 {
                res &= ch1 < ch3;
            }

            if ch1 == ch2 && ch2 == ch3 {
                res &= ch1 == ch3;
            }

            if ch1 > ch2 && ch2 > ch3 {
                res &= ch1 > ch3;
            }

            res
        }

        /// The `Ord` instance of `Change` is transitive.
        fn ord_change_transitive(c1: Change_, c2: Change_, c3: Change_) -> bool {
            let ch1 = build_change(c1.3.inner(), c1.4, c1.5);
            let ch2 = build_change(c2.3.inner(), c2.4, c2.5);
            let ch3 = build_change(c3.3.inner(), c3.4, c3.5);

            let mut res = true;

            if ch1 < ch2 && ch2 < ch3 {
                res &= ch1 < ch3;
            }

            if ch1 == ch2 && ch2 == ch3 {
                res &= ch1 == ch3;
            }

            if ch1 > ch2 && ch2 > ch3 {
                res &= ch1 > ch3;
            }

            res
        }

        /// The maximal change category for a change set with regular changes only gets computed
        /// correctly.
        fn max_pchange(changes: Vec<PathChange_>) -> bool {
            let mut set = ChangeSet::default();

            let mut interner = Interner::new();
            let name = interner.intern("test");

            let max = changes
                .iter()
                .flat_map(|change| change.2.iter())
                .map(|&(c, _)| if c { TechnicallyBreaking } else { Breaking })
                .max()
                .unwrap_or(Patch);

            for &(ref did, ref span, ref spans) in &changes {
                let def_id = did.clone().inner();
                set.new_path_change(def_id, name, span.clone().inner());

                for &(add, ref span) in spans {
                    if add {
                        set.add_path_addition(def_id, span.clone().inner());
                    } else {
                        set.add_path_removal(def_id, span.clone().inner());
                    }
                }
            }

            set.max == max
        }

        /// The maximal change category for a change set with path changes only gets computed
        /// correctly.
        fn max_change(changes: Vec<Change_>) -> bool {
            let mut set = ChangeSet::default();

            let mut interner = Interner::new();
            let name = interner.intern("test");

            let max = changes
                .iter()
                .filter(|change| change.4)
                .flat_map(|change| change.5.iter())
                .map(|&(ref type_, _)| type_.inner().to_category())
                .max()
                .unwrap_or(Patch);

            for &(ref o_def_id, ref n_def_id, ref o_span, ref n_span, out, ref sub) in &changes {
                let old_def_id = o_def_id.clone().inner();
                set.new_change(old_def_id,
                               n_def_id.clone().inner(),
                               name,
                               o_span.clone().inner(),
                               n_span.clone().inner(),
                               out);

                for &(ref type_, ref span_) in sub {
                    set.add_change(type_.clone().inner(),
                                   old_def_id,
                                   span_.clone().map(|s| s.inner()));
                }
            }

            set.max == max
        }

        fn max_pchange_or_change(pchanges: Vec<PathChange_>, changes: Vec<Change_>) -> bool {
            let mut set = ChangeSet::default();

            let mut interner = Interner::new();
            let name = interner.intern("test");

            let max = pchanges
                .iter()
                .flat_map(|change| change.2.iter())
                .map(|&(c, _)| if c { TechnicallyBreaking } else { Breaking })
                .chain(changes
                    .iter()
                    .filter(|change| change.4)
                    .flat_map(|change| change.5.iter())
                    .map(|&(ref type_, _)| type_.inner().to_category()))
                .max()
                .unwrap_or(Patch);

            for &(ref did, ref span, ref spans) in &pchanges {
                let def_id = did.clone().inner();
                set.new_path_change(def_id, name, span.clone().inner());

                for &(add, ref span) in spans {
                    if add {
                        set.add_path_addition(def_id, span.clone().inner());
                    } else {
                        set.add_path_removal(def_id, span.clone().inner());
                    }
                }
            }

            for &(ref o_def_id, ref n_def_id, ref o_span, ref n_span, out, ref sub) in &changes {
                let old_def_id = o_def_id.clone().inner();
                set.new_change(old_def_id,
                               n_def_id.clone().inner(),
                               name,
                               o_span.clone().inner(),
                               n_span.clone().inner(),
                               out);

                for &(ref type_, ref span_) in sub {
                    set.add_change(type_.clone().inner(),
                                   old_def_id,
                                   span_.clone().map(|s| s.inner()));
                }
            }

            set.max == max
        }

        /// Difference in spans implies difference in `PathChange`s.
        fn pchange_span_neq(c1: PathChange_, c2: PathChange_) -> bool {
            let v1 = c1.2.iter().map(|&(add, ref s)| (add, s.clone().inner())).collect();
            let v2 = c2.2.iter().map(|&(add, ref s)| (add, s.clone().inner())).collect();

            let s1 = c1.1.clone().inner();
            let s2 = c2.1.clone().inner();

            if s1 != s2 {
                let ch1 = build_path_change(s1, v1);
                let ch2 = build_path_change(s2, v2);

                ch1 != ch2
            } else {
                true
            }
        }

        /// Difference in spans implies difference in `Change`s.
        fn bchange_span_neq(c1: Change_, c2: Change_) -> bool {
            let s1 = c1.3.clone().inner();
            let s2 = c2.3.clone().inner();

            if s1 != s2 {
                let ch1 = build_change(c1.3.inner(), c1.4, c1.5);
                let ch2 = build_change(c2.3.inner(), c2.4, c2.5);

                ch1 != ch2
            } else {
                true
            }
        }
    }
}
