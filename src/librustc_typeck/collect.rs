//! "Collection" is the process of determining the type and other external
//! details of each item in Rust. Collection is specifically concerned
//! with *inter-procedural* things -- for example, for a function
//! definition, collection will figure out the type and signature of the
//! function, but it will not visit the *body* of the function in any way,
//! nor examine type annotations on local variables (that's the job of
//! type *checking*).
//!
//! Collecting is ultimately defined by a bundle of queries that
//! inquire after various facts about the items in the crate (e.g.,
//! `type_of`, `generics_of`, `predicates_of`, etc). See the `provide` function
//! for the full set.
//!
//! At present, however, we do run collection across all items in the
//! crate as a kind of pass. This should eventually be factored away.

use crate::astconv::{AstConv, Bounds, SizedByDefault};
use crate::check::intrinsic::intrinsic_operation_unsafety;
use crate::constrained_generic_params as cgp;
use crate::middle::lang_items;
use crate::middle::resolve_lifetime as rl;
use rustc::hir::map::blocks::FnLikeNode;
use rustc::hir::map::Map;
use rustc::middle::codegen_fn_attrs::{CodegenFnAttrFlags, CodegenFnAttrs};
use rustc::mir::mono::Linkage;
use rustc::ty::query::Providers;
use rustc::ty::subst::{InternalSubsts, Subst};
use rustc::ty::util::Discr;
use rustc::ty::util::IntTypeExt;
use rustc::ty::{self, AdtKind, Const, ToPolyTraitRef, Ty, TyCtxt};
use rustc::ty::{ReprOptions, ToPredicate, WithConstness};
use rustc_ast::ast;
use rustc_ast::ast::{Ident, MetaItemKind};
use rustc_attr::{list_contains_name, mark_used, InlineAttr, OptimizeAttr};
use rustc_data_structures::captures::Captures;
use rustc_data_structures::fx::{FxHashMap, FxHashSet};
use rustc_errors::{struct_span_err, Applicability};
use rustc_hir as hir;
use rustc_hir::def::{CtorKind, DefKind, Res};
use rustc_hir::def_id::{DefId, LOCAL_CRATE};
use rustc_hir::intravisit::{self, NestedVisitorMap, Visitor};
use rustc_hir::{GenericParamKind, Node, Unsafety};
use rustc_session::lint;
use rustc_session::parse::feature_err;
use rustc_span::symbol::{kw, sym, Symbol};
use rustc_span::{Span, DUMMY_SP};
use rustc_target::spec::abi;

mod type_of;

struct OnlySelfBounds(bool);

///////////////////////////////////////////////////////////////////////////
// Main entry point

fn collect_mod_item_types(tcx: TyCtxt<'_>, module_def_id: DefId) {
    tcx.hir().visit_item_likes_in_module(
        module_def_id,
        &mut CollectItemTypesVisitor { tcx }.as_deep_visitor(),
    );
}

pub fn provide(providers: &mut Providers<'_>) {
    *providers = Providers {
        type_of: type_of::type_of,
        generics_of,
        predicates_of,
        predicates_defined_on,
        explicit_predicates_of,
        super_predicates_of,
        type_param_predicates,
        trait_def,
        adt_def,
        fn_sig,
        impl_trait_ref,
        impl_polarity,
        is_foreign_item,
        static_mutability,
        generator_kind,
        codegen_fn_attrs,
        collect_mod_item_types,
        ..*providers
    };
}

///////////////////////////////////////////////////////////////////////////

/// Context specific to some particular item. This is what implements
/// `AstConv`. It has information about the predicates that are defined
/// on the trait. Unfortunately, this predicate information is
/// available in various different forms at various points in the
/// process. So we can't just store a pointer to e.g., the AST or the
/// parsed ty form, we have to be more flexible. To this end, the
/// `ItemCtxt` is parameterized by a `DefId` that it uses to satisfy
/// `get_type_parameter_bounds` requests, drawing the information from
/// the AST (`hir::Generics`), recursively.
pub struct ItemCtxt<'tcx> {
    tcx: TyCtxt<'tcx>,
    item_def_id: DefId,
}

///////////////////////////////////////////////////////////////////////////

#[derive(Default)]
crate struct PlaceholderHirTyCollector(crate Vec<Span>);

impl<'v> Visitor<'v> for PlaceholderHirTyCollector {
    type Map = intravisit::ErasedMap<'v>;

    fn nested_visit_map(&mut self) -> NestedVisitorMap<Self::Map> {
        NestedVisitorMap::None
    }
    fn visit_ty(&mut self, t: &'v hir::Ty<'v>) {
        if let hir::TyKind::Infer = t.kind {
            self.0.push(t.span);
        }
        intravisit::walk_ty(self, t)
    }
}

struct CollectItemTypesVisitor<'tcx> {
    tcx: TyCtxt<'tcx>,
}

/// If there are any placeholder types (`_`), emit an error explaining that this is not allowed
/// and suggest adding type parameters in the appropriate place, taking into consideration any and
/// all already existing generic type parameters to avoid suggesting a name that is already in use.
crate fn placeholder_type_error(
    tcx: TyCtxt<'tcx>,
    span: Span,
    generics: &[hir::GenericParam<'_>],
    placeholder_types: Vec<Span>,
    suggest: bool,
) {
    if placeholder_types.is_empty() {
        return;
    }
    // This is the whitelist of possible parameter names that we might suggest.
    let possible_names = ["T", "K", "L", "A", "B", "C"];
    let used_names = generics
        .iter()
        .filter_map(|p| match p.name {
            hir::ParamName::Plain(ident) => Some(ident.name),
            _ => None,
        })
        .collect::<Vec<_>>();

    let type_name = possible_names
        .iter()
        .find(|n| !used_names.contains(&Symbol::intern(n)))
        .unwrap_or(&"ParamName");

    let mut sugg: Vec<_> =
        placeholder_types.iter().map(|sp| (*sp, (*type_name).to_string())).collect();
    if generics.is_empty() {
        sugg.push((span, format!("<{}>", type_name)));
    } else if let Some(arg) = generics.iter().find(|arg| match arg.name {
        hir::ParamName::Plain(Ident { name: kw::Underscore, .. }) => true,
        _ => false,
    }) {
        // Account for `_` already present in cases like `struct S<_>(_);` and suggest
        // `struct S<T>(T);` instead of `struct S<_, T>(T);`.
        sugg.push((arg.span, (*type_name).to_string()));
    } else {
        sugg.push((
            generics.iter().last().unwrap().span.shrink_to_hi(),
            format!(", {}", type_name),
        ));
    }
    let mut err = bad_placeholder_type(tcx, placeholder_types);
    if suggest {
        err.multipart_suggestion(
            "use type parameters instead",
            sugg,
            Applicability::HasPlaceholders,
        );
    }
    err.emit();
}

fn reject_placeholder_type_signatures_in_item(tcx: TyCtxt<'tcx>, item: &'tcx hir::Item<'tcx>) {
    let (generics, suggest) = match &item.kind {
        hir::ItemKind::Union(_, generics)
        | hir::ItemKind::Enum(_, generics)
        | hir::ItemKind::TraitAlias(generics, _)
        | hir::ItemKind::Trait(_, _, generics, ..)
        | hir::ItemKind::Impl { generics, .. }
        | hir::ItemKind::Struct(_, generics) => (generics, true),
        hir::ItemKind::OpaqueTy(hir::OpaqueTy { generics, .. })
        | hir::ItemKind::TyAlias(_, generics) => (generics, false),
        // `static`, `fn` and `const` are handled elsewhere to suggest appropriate type.
        _ => return,
    };

    let mut visitor = PlaceholderHirTyCollector::default();
    visitor.visit_item(item);

    placeholder_type_error(tcx, generics.span, &generics.params[..], visitor.0, suggest);
}

impl Visitor<'tcx> for CollectItemTypesVisitor<'tcx> {
    type Map = Map<'tcx>;

    fn nested_visit_map(&mut self) -> NestedVisitorMap<Self::Map> {
        NestedVisitorMap::OnlyBodies(self.tcx.hir())
    }

    fn visit_item(&mut self, item: &'tcx hir::Item<'tcx>) {
        convert_item(self.tcx, item.hir_id);
        reject_placeholder_type_signatures_in_item(self.tcx, item);
        intravisit::walk_item(self, item);
    }

    fn visit_generics(&mut self, generics: &'tcx hir::Generics<'tcx>) {
        for param in generics.params {
            match param.kind {
                hir::GenericParamKind::Lifetime { .. } => {}
                hir::GenericParamKind::Type { default: Some(_), .. } => {
                    let def_id = self.tcx.hir().local_def_id(param.hir_id);
                    self.tcx.type_of(def_id);
                }
                hir::GenericParamKind::Type { .. } => {}
                hir::GenericParamKind::Const { .. } => {
                    let def_id = self.tcx.hir().local_def_id(param.hir_id);
                    self.tcx.type_of(def_id);
                }
            }
        }
        intravisit::walk_generics(self, generics);
    }

    fn visit_expr(&mut self, expr: &'tcx hir::Expr<'tcx>) {
        if let hir::ExprKind::Closure(..) = expr.kind {
            let def_id = self.tcx.hir().local_def_id(expr.hir_id);
            self.tcx.generics_of(def_id);
            self.tcx.type_of(def_id);
        }
        intravisit::walk_expr(self, expr);
    }

    fn visit_trait_item(&mut self, trait_item: &'tcx hir::TraitItem<'tcx>) {
        convert_trait_item(self.tcx, trait_item.hir_id);
        intravisit::walk_trait_item(self, trait_item);
    }

    fn visit_impl_item(&mut self, impl_item: &'tcx hir::ImplItem<'tcx>) {
        convert_impl_item(self.tcx, impl_item.hir_id);
        intravisit::walk_impl_item(self, impl_item);
    }
}

///////////////////////////////////////////////////////////////////////////
// Utility types and common code for the above passes.

fn bad_placeholder_type(
    tcx: TyCtxt<'tcx>,
    mut spans: Vec<Span>,
) -> rustc_errors::DiagnosticBuilder<'tcx> {
    spans.sort();
    let mut err = struct_span_err!(
        tcx.sess,
        spans.clone(),
        E0121,
        "the type placeholder `_` is not allowed within types on item signatures",
    );
    for span in spans {
        err.span_label(span, "not allowed in type signatures");
    }
    err
}

impl ItemCtxt<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>, item_def_id: DefId) -> ItemCtxt<'tcx> {
        ItemCtxt { tcx, item_def_id }
    }

    pub fn to_ty(&self, ast_ty: &'tcx hir::Ty<'tcx>) -> Ty<'tcx> {
        AstConv::ast_ty_to_ty(self, ast_ty)
    }

    pub fn hir_id(&self) -> hir::HirId {
        self.tcx
            .hir()
            .as_local_hir_id(self.item_def_id)
            .expect("Non-local call to local provider is_const_fn")
    }

    pub fn node(&self) -> hir::Node<'tcx> {
        self.tcx.hir().get(self.hir_id())
    }
}

impl AstConv<'tcx> for ItemCtxt<'tcx> {
    fn tcx(&self) -> TyCtxt<'tcx> {
        self.tcx
    }

    fn item_def_id(&self) -> Option<DefId> {
        Some(self.item_def_id)
    }

    fn default_constness_for_trait_bounds(&self) -> hir::Constness {
        if let Some(fn_like) = FnLikeNode::from_node(self.node()) {
            fn_like.constness()
        } else {
            hir::Constness::NotConst
        }
    }

    fn get_type_parameter_bounds(&self, span: Span, def_id: DefId) -> ty::GenericPredicates<'tcx> {
        self.tcx.at(span).type_param_predicates((self.item_def_id, def_id))
    }

    fn re_infer(&self, _: Option<&ty::GenericParamDef>, _: Span) -> Option<ty::Region<'tcx>> {
        None
    }

    fn allow_ty_infer(&self) -> bool {
        false
    }

    fn ty_infer(&self, _: Option<&ty::GenericParamDef>, span: Span) -> Ty<'tcx> {
        self.tcx().sess.delay_span_bug(span, "bad placeholder type");
        self.tcx().types.err
    }

    fn ct_infer(
        &self,
        _: Ty<'tcx>,
        _: Option<&ty::GenericParamDef>,
        span: Span,
    ) -> &'tcx Const<'tcx> {
        bad_placeholder_type(self.tcx(), vec![span]).emit();

        self.tcx().consts.err
    }

    fn projected_ty_from_poly_trait_ref(
        &self,
        span: Span,
        item_def_id: DefId,
        item_segment: &hir::PathSegment<'_>,
        poly_trait_ref: ty::PolyTraitRef<'tcx>,
    ) -> Ty<'tcx> {
        if let Some(trait_ref) = poly_trait_ref.no_bound_vars() {
            let item_substs = <dyn AstConv<'tcx>>::create_substs_for_associated_item(
                self,
                self.tcx,
                span,
                item_def_id,
                item_segment,
                trait_ref.substs,
            );
            self.tcx().mk_projection(item_def_id, item_substs)
        } else {
            // There are no late-bound regions; we can just ignore the binder.
            let mut err = struct_span_err!(
                self.tcx().sess,
                span,
                E0212,
                "cannot extract an associated type from a higher-ranked trait bound \
                 in this context"
            );

            match self.node() {
                hir::Node::Field(_) | hir::Node::Ctor(_) | hir::Node::Variant(_) => {
                    let item =
                        self.tcx.hir().expect_item(self.tcx.hir().get_parent_item(self.hir_id()));
                    match &item.kind {
                        hir::ItemKind::Enum(_, generics)
                        | hir::ItemKind::Struct(_, generics)
                        | hir::ItemKind::Union(_, generics) => {
                            let lt_name = get_new_lifetime_name(self.tcx, poly_trait_ref, generics);
                            let (lt_sp, sugg) = match &generics.params[..] {
                                [] => (generics.span, format!("<{}>", lt_name)),
                                [bound, ..] => {
                                    (bound.span.shrink_to_lo(), format!("{}, ", lt_name))
                                }
                            };
                            let suggestions = vec![
                                (lt_sp, sugg),
                                (
                                    span,
                                    format!(
                                        "{}::{}",
                                        // Replace the existing lifetimes with a new named lifetime.
                                        self.tcx
                                            .replace_late_bound_regions(&poly_trait_ref, |_| {
                                                self.tcx.mk_region(ty::ReEarlyBound(
                                                    ty::EarlyBoundRegion {
                                                        def_id: item_def_id,
                                                        index: 0,
                                                        name: Symbol::intern(&lt_name),
                                                    },
                                                ))
                                            })
                                            .0,
                                        item_segment.ident
                                    ),
                                ),
                            ];
                            err.multipart_suggestion(
                                "use a fully qualified path with explicit lifetimes",
                                suggestions,
                                Applicability::MaybeIncorrect,
                            );
                        }
                        _ => {}
                    }
                }
                hir::Node::Item(hir::Item { kind: hir::ItemKind::Struct(..), .. })
                | hir::Node::Item(hir::Item { kind: hir::ItemKind::Enum(..), .. })
                | hir::Node::Item(hir::Item { kind: hir::ItemKind::Union(..), .. }) => {}
                hir::Node::Item(_)
                | hir::Node::ForeignItem(_)
                | hir::Node::TraitItem(_)
                | hir::Node::ImplItem(_) => {
                    err.span_suggestion(
                        span,
                        "use a fully qualified path with inferred lifetimes",
                        format!(
                            "{}::{}",
                            // Erase named lt, we want `<A as B<'_>::C`, not `<A as B<'a>::C`.
                            self.tcx.anonymize_late_bound_regions(&poly_trait_ref).skip_binder(),
                            item_segment.ident
                        ),
                        Applicability::MaybeIncorrect,
                    );
                }
                _ => {}
            }
            err.emit();
            self.tcx().types.err
        }
    }

    fn normalize_ty(&self, _span: Span, ty: Ty<'tcx>) -> Ty<'tcx> {
        // Types in item signatures are not normalized to avoid undue dependencies.
        ty
    }

    fn set_tainted_by_errors(&self) {
        // There's no obvious place to track this, so just let it go.
    }

    fn record_ty(&self, _hir_id: hir::HirId, _ty: Ty<'tcx>, _span: Span) {
        // There's no place to record types from signatures?
    }
}

/// Synthesize a new lifetime name that doesn't clash with any of the lifetimes already present.
fn get_new_lifetime_name<'tcx>(
    tcx: TyCtxt<'tcx>,
    poly_trait_ref: ty::PolyTraitRef<'tcx>,
    generics: &hir::Generics<'tcx>,
) -> String {
    let existing_lifetimes = tcx
        .collect_referenced_late_bound_regions(&poly_trait_ref)
        .into_iter()
        .filter_map(|lt| {
            if let ty::BoundRegion::BrNamed(_, name) = lt {
                Some(name.as_str().to_string())
            } else {
                None
            }
        })
        .chain(generics.params.iter().filter_map(|param| {
            if let hir::GenericParamKind::Lifetime { .. } = &param.kind {
                Some(param.name.ident().as_str().to_string())
            } else {
                None
            }
        }))
        .collect::<FxHashSet<String>>();

    let a_to_z_repeat_n = |n| {
        (b'a'..=b'z').map(move |c| {
            let mut s = '\''.to_string();
            s.extend(std::iter::repeat(char::from(c)).take(n));
            s
        })
    };

    // If all single char lifetime names are present, we wrap around and double the chars.
    (1..).flat_map(a_to_z_repeat_n).find(|lt| !existing_lifetimes.contains(lt.as_str())).unwrap()
}

/// Returns the predicates defined on `item_def_id` of the form
/// `X: Foo` where `X` is the type parameter `def_id`.
fn type_param_predicates(
    tcx: TyCtxt<'_>,
    (item_def_id, def_id): (DefId, DefId),
) -> ty::GenericPredicates<'_> {
    use rustc_hir::*;

    // In the AST, bounds can derive from two places. Either
    // written inline like `<T: Foo>` or in a where-clause like
    // `where T: Foo`.

    let param_id = tcx.hir().as_local_hir_id(def_id).unwrap();
    let param_owner = tcx.hir().ty_param_owner(param_id);
    let param_owner_def_id = tcx.hir().local_def_id(param_owner);
    let generics = tcx.generics_of(param_owner_def_id);
    let index = generics.param_def_id_to_index[&def_id];
    let ty = tcx.mk_ty_param(index, tcx.hir().ty_param_name(param_id));

    // Don't look for bounds where the type parameter isn't in scope.
    let parent =
        if item_def_id == param_owner_def_id { None } else { tcx.generics_of(item_def_id).parent };

    let mut result = parent
        .map(|parent| {
            let icx = ItemCtxt::new(tcx, parent);
            icx.get_type_parameter_bounds(DUMMY_SP, def_id)
        })
        .unwrap_or_default();
    let mut extend = None;

    let item_hir_id = tcx.hir().as_local_hir_id(item_def_id).unwrap();
    let ast_generics = match tcx.hir().get(item_hir_id) {
        Node::TraitItem(item) => &item.generics,

        Node::ImplItem(item) => &item.generics,

        Node::Item(item) => {
            match item.kind {
                ItemKind::Fn(.., ref generics, _)
                | ItemKind::Impl { ref generics, .. }
                | ItemKind::TyAlias(_, ref generics)
                | ItemKind::OpaqueTy(OpaqueTy { ref generics, impl_trait_fn: None, .. })
                | ItemKind::Enum(_, ref generics)
                | ItemKind::Struct(_, ref generics)
                | ItemKind::Union(_, ref generics) => generics,
                ItemKind::Trait(_, _, ref generics, ..) => {
                    // Implied `Self: Trait` and supertrait bounds.
                    if param_id == item_hir_id {
                        let identity_trait_ref = ty::TraitRef::identity(tcx, item_def_id);
                        extend =
                            Some((identity_trait_ref.without_const().to_predicate(), item.span));
                    }
                    generics
                }
                _ => return result,
            }
        }

        Node::ForeignItem(item) => match item.kind {
            ForeignItemKind::Fn(_, _, ref generics) => generics,
            _ => return result,
        },

        _ => return result,
    };

    let icx = ItemCtxt::new(tcx, item_def_id);
    let extra_predicates = extend.into_iter().chain(
        icx.type_parameter_bounds_in_generics(ast_generics, param_id, ty, OnlySelfBounds(true))
            .into_iter()
            .filter(|(predicate, _)| match predicate {
                ty::Predicate::Trait(ref data, _) => data.skip_binder().self_ty().is_param(index),
                _ => false,
            }),
    );
    result.predicates =
        tcx.arena.alloc_from_iter(result.predicates.iter().copied().chain(extra_predicates));
    result
}

impl ItemCtxt<'tcx> {
    /// Finds bounds from `hir::Generics`. This requires scanning through the
    /// AST. We do this to avoid having to convert *all* the bounds, which
    /// would create artificial cycles. Instead, we can only convert the
    /// bounds for a type parameter `X` if `X::Foo` is used.
    fn type_parameter_bounds_in_generics(
        &self,
        ast_generics: &'tcx hir::Generics<'tcx>,
        param_id: hir::HirId,
        ty: Ty<'tcx>,
        only_self_bounds: OnlySelfBounds,
    ) -> Vec<(ty::Predicate<'tcx>, Span)> {
        let constness = self.default_constness_for_trait_bounds();
        let from_ty_params = ast_generics
            .params
            .iter()
            .filter_map(|param| match param.kind {
                GenericParamKind::Type { .. } if param.hir_id == param_id => Some(&param.bounds),
                _ => None,
            })
            .flat_map(|bounds| bounds.iter())
            .flat_map(|b| predicates_from_bound(self, ty, b, constness));

        let from_where_clauses = ast_generics
            .where_clause
            .predicates
            .iter()
            .filter_map(|wp| match *wp {
                hir::WherePredicate::BoundPredicate(ref bp) => Some(bp),
                _ => None,
            })
            .flat_map(|bp| {
                let bt = if is_param(self.tcx, &bp.bounded_ty, param_id) {
                    Some(ty)
                } else if !only_self_bounds.0 {
                    Some(self.to_ty(&bp.bounded_ty))
                } else {
                    None
                };
                bp.bounds.iter().filter_map(move |b| bt.map(|bt| (bt, b)))
            })
            .flat_map(|(bt, b)| predicates_from_bound(self, bt, b, constness));

        from_ty_params.chain(from_where_clauses).collect()
    }
}

/// Tests whether this is the AST for a reference to the type
/// parameter with ID `param_id`. We use this so as to avoid running
/// `ast_ty_to_ty`, because we want to avoid triggering an all-out
/// conversion of the type to avoid inducing unnecessary cycles.
fn is_param(tcx: TyCtxt<'_>, ast_ty: &hir::Ty<'_>, param_id: hir::HirId) -> bool {
    if let hir::TyKind::Path(hir::QPath::Resolved(None, ref path)) = ast_ty.kind {
        match path.res {
            Res::SelfTy(Some(def_id), None) | Res::Def(DefKind::TyParam, def_id) => {
                def_id == tcx.hir().local_def_id(param_id)
            }
            _ => false,
        }
    } else {
        false
    }
}

fn convert_item(tcx: TyCtxt<'_>, item_id: hir::HirId) {
    let it = tcx.hir().expect_item(item_id);
    debug!("convert: item {} with id {}", it.ident, it.hir_id);
    let def_id = tcx.hir().local_def_id(item_id);
    match it.kind {
        // These don't define types.
        hir::ItemKind::ExternCrate(_)
        | hir::ItemKind::Use(..)
        | hir::ItemKind::Mod(_)
        | hir::ItemKind::GlobalAsm(_) => {}
        hir::ItemKind::ForeignMod(ref foreign_mod) => {
            for item in foreign_mod.items {
                let def_id = tcx.hir().local_def_id(item.hir_id);
                tcx.generics_of(def_id);
                tcx.type_of(def_id);
                tcx.predicates_of(def_id);
                if let hir::ForeignItemKind::Fn(..) = item.kind {
                    tcx.fn_sig(def_id);
                }
            }
        }
        hir::ItemKind::Enum(ref enum_definition, _) => {
            tcx.generics_of(def_id);
            tcx.type_of(def_id);
            tcx.predicates_of(def_id);
            convert_enum_variant_types(tcx, def_id, &enum_definition.variants);
        }
        hir::ItemKind::Impl { .. } => {
            tcx.generics_of(def_id);
            tcx.type_of(def_id);
            tcx.impl_trait_ref(def_id);
            tcx.predicates_of(def_id);
        }
        hir::ItemKind::Trait(..) => {
            tcx.generics_of(def_id);
            tcx.trait_def(def_id);
            tcx.at(it.span).super_predicates_of(def_id);
            tcx.predicates_of(def_id);
        }
        hir::ItemKind::TraitAlias(..) => {
            tcx.generics_of(def_id);
            tcx.at(it.span).super_predicates_of(def_id);
            tcx.predicates_of(def_id);
        }
        hir::ItemKind::Struct(ref struct_def, _) | hir::ItemKind::Union(ref struct_def, _) => {
            tcx.generics_of(def_id);
            tcx.type_of(def_id);
            tcx.predicates_of(def_id);

            for f in struct_def.fields() {
                let def_id = tcx.hir().local_def_id(f.hir_id);
                tcx.generics_of(def_id);
                tcx.type_of(def_id);
                tcx.predicates_of(def_id);
            }

            if let Some(ctor_hir_id) = struct_def.ctor_hir_id() {
                convert_variant_ctor(tcx, ctor_hir_id);
            }
        }

        // Desugared from `impl Trait`, so visited by the function's return type.
        hir::ItemKind::OpaqueTy(hir::OpaqueTy { impl_trait_fn: Some(_), .. }) => {}

        hir::ItemKind::OpaqueTy(..)
        | hir::ItemKind::TyAlias(..)
        | hir::ItemKind::Static(..)
        | hir::ItemKind::Const(..)
        | hir::ItemKind::Fn(..) => {
            tcx.generics_of(def_id);
            tcx.type_of(def_id);
            tcx.predicates_of(def_id);
            if let hir::ItemKind::Fn(..) = it.kind {
                tcx.fn_sig(def_id);
            }
        }
    }
}

fn convert_trait_item(tcx: TyCtxt<'_>, trait_item_id: hir::HirId) {
    let trait_item = tcx.hir().expect_trait_item(trait_item_id);
    let def_id = tcx.hir().local_def_id(trait_item.hir_id);
    tcx.generics_of(def_id);

    match trait_item.kind {
        hir::TraitItemKind::Fn(..) => {
            tcx.type_of(def_id);
            tcx.fn_sig(def_id);
        }

        hir::TraitItemKind::Const(.., Some(_)) => {
            tcx.type_of(def_id);
        }

        hir::TraitItemKind::Const(..) | hir::TraitItemKind::Type(_, Some(_)) => {
            tcx.type_of(def_id);
            // Account for `const C: _;` and `type T = _;`.
            let mut visitor = PlaceholderHirTyCollector::default();
            visitor.visit_trait_item(trait_item);
            placeholder_type_error(tcx, DUMMY_SP, &[], visitor.0, false);
        }

        hir::TraitItemKind::Type(_, None) => {}
    };

    tcx.predicates_of(def_id);
}

fn convert_impl_item(tcx: TyCtxt<'_>, impl_item_id: hir::HirId) {
    let def_id = tcx.hir().local_def_id(impl_item_id);
    tcx.generics_of(def_id);
    tcx.type_of(def_id);
    tcx.predicates_of(def_id);
    let impl_item = tcx.hir().expect_impl_item(impl_item_id);
    match impl_item.kind {
        hir::ImplItemKind::Fn(..) => {
            tcx.fn_sig(def_id);
        }
        hir::ImplItemKind::TyAlias(_) | hir::ImplItemKind::OpaqueTy(_) => {
            // Account for `type T = _;`
            let mut visitor = PlaceholderHirTyCollector::default();
            visitor.visit_impl_item(impl_item);
            placeholder_type_error(tcx, DUMMY_SP, &[], visitor.0, false);
        }
        hir::ImplItemKind::Const(..) => {}
    }
}

fn convert_variant_ctor(tcx: TyCtxt<'_>, ctor_id: hir::HirId) {
    let def_id = tcx.hir().local_def_id(ctor_id);
    tcx.generics_of(def_id);
    tcx.type_of(def_id);
    tcx.predicates_of(def_id);
}

fn convert_enum_variant_types(tcx: TyCtxt<'_>, def_id: DefId, variants: &[hir::Variant<'_>]) {
    let def = tcx.adt_def(def_id);
    let repr_type = def.repr.discr_type();
    let initial = repr_type.initial_discriminant(tcx);
    let mut prev_discr = None::<Discr<'_>>;

    // fill the discriminant values and field types
    for variant in variants {
        let wrapped_discr = prev_discr.map_or(initial, |d| d.wrap_incr(tcx));
        prev_discr = Some(
            if let Some(ref e) = variant.disr_expr {
                let expr_did = tcx.hir().local_def_id(e.hir_id);
                def.eval_explicit_discr(tcx, expr_did)
            } else if let Some(discr) = repr_type.disr_incr(tcx, prev_discr) {
                Some(discr)
            } else {
                struct_span_err!(tcx.sess, variant.span, E0370, "enum discriminant overflowed")
                    .span_label(
                        variant.span,
                        format!("overflowed on value after {}", prev_discr.unwrap()),
                    )
                    .note(&format!(
                        "explicitly set `{} = {}` if that is desired outcome",
                        variant.ident, wrapped_discr
                    ))
                    .emit();
                None
            }
            .unwrap_or(wrapped_discr),
        );

        for f in variant.data.fields() {
            let def_id = tcx.hir().local_def_id(f.hir_id);
            tcx.generics_of(def_id);
            tcx.type_of(def_id);
            tcx.predicates_of(def_id);
        }

        // Convert the ctor, if any. This also registers the variant as
        // an item.
        if let Some(ctor_hir_id) = variant.data.ctor_hir_id() {
            convert_variant_ctor(tcx, ctor_hir_id);
        }
    }
}

fn convert_variant(
    tcx: TyCtxt<'_>,
    variant_did: Option<DefId>,
    ctor_did: Option<DefId>,
    ident: Ident,
    discr: ty::VariantDiscr,
    def: &hir::VariantData<'_>,
    adt_kind: ty::AdtKind,
    parent_did: DefId,
) -> ty::VariantDef {
    let mut seen_fields: FxHashMap<ast::Ident, Span> = Default::default();
    let hir_id = tcx.hir().as_local_hir_id(variant_did.unwrap_or(parent_did)).unwrap();
    let fields = def
        .fields()
        .iter()
        .map(|f| {
            let fid = tcx.hir().local_def_id(f.hir_id);
            let dup_span = seen_fields.get(&f.ident.normalize_to_macros_2_0()).cloned();
            if let Some(prev_span) = dup_span {
                struct_span_err!(
                    tcx.sess,
                    f.span,
                    E0124,
                    "field `{}` is already declared",
                    f.ident
                )
                .span_label(f.span, "field already declared")
                .span_label(prev_span, format!("`{}` first declared here", f.ident))
                .emit();
            } else {
                seen_fields.insert(f.ident.normalize_to_macros_2_0(), f.span);
            }

            ty::FieldDef {
                did: fid,
                ident: f.ident,
                vis: ty::Visibility::from_hir(&f.vis, hir_id, tcx),
            }
        })
        .collect();
    let recovered = match def {
        hir::VariantData::Struct(_, r) => *r,
        _ => false,
    };
    ty::VariantDef::new(
        tcx,
        ident,
        variant_did,
        ctor_did,
        discr,
        fields,
        CtorKind::from_hir(def),
        adt_kind,
        parent_did,
        recovered,
    )
}

fn adt_def(tcx: TyCtxt<'_>, def_id: DefId) -> &ty::AdtDef {
    use rustc_hir::*;

    let hir_id = tcx.hir().as_local_hir_id(def_id).unwrap();
    let item = match tcx.hir().get(hir_id) {
        Node::Item(item) => item,
        _ => bug!(),
    };

    let repr = ReprOptions::new(tcx, def_id);
    let (kind, variants) = match item.kind {
        ItemKind::Enum(ref def, _) => {
            let mut distance_from_explicit = 0;
            let variants = def
                .variants
                .iter()
                .map(|v| {
                    let variant_did = Some(tcx.hir().local_def_id(v.id));
                    let ctor_did =
                        v.data.ctor_hir_id().map(|hir_id| tcx.hir().local_def_id(hir_id));

                    let discr = if let Some(ref e) = v.disr_expr {
                        distance_from_explicit = 0;
                        ty::VariantDiscr::Explicit(tcx.hir().local_def_id(e.hir_id))
                    } else {
                        ty::VariantDiscr::Relative(distance_from_explicit)
                    };
                    distance_from_explicit += 1;

                    convert_variant(
                        tcx,
                        variant_did,
                        ctor_did,
                        v.ident,
                        discr,
                        &v.data,
                        AdtKind::Enum,
                        def_id,
                    )
                })
                .collect();

            (AdtKind::Enum, variants)
        }
        ItemKind::Struct(ref def, _) => {
            let variant_did = None;
            let ctor_did = def.ctor_hir_id().map(|hir_id| tcx.hir().local_def_id(hir_id));

            let variants = std::iter::once(convert_variant(
                tcx,
                variant_did,
                ctor_did,
                item.ident,
                ty::VariantDiscr::Relative(0),
                def,
                AdtKind::Struct,
                def_id,
            ))
            .collect();

            (AdtKind::Struct, variants)
        }
        ItemKind::Union(ref def, _) => {
            let variant_did = None;
            let ctor_did = def.ctor_hir_id().map(|hir_id| tcx.hir().local_def_id(hir_id));

            let variants = std::iter::once(convert_variant(
                tcx,
                variant_did,
                ctor_did,
                item.ident,
                ty::VariantDiscr::Relative(0),
                def,
                AdtKind::Union,
                def_id,
            ))
            .collect();

            (AdtKind::Union, variants)
        }
        _ => bug!(),
    };
    tcx.alloc_adt_def(def_id, kind, variants, repr)
}

/// Ensures that the super-predicates of the trait with a `DefId`
/// of `trait_def_id` are converted and stored. This also ensures that
/// the transitive super-predicates are converted.
fn super_predicates_of(tcx: TyCtxt<'_>, trait_def_id: DefId) -> ty::GenericPredicates<'_> {
    debug!("super_predicates(trait_def_id={:?})", trait_def_id);
    let trait_hir_id = tcx.hir().as_local_hir_id(trait_def_id).unwrap();

    let item = match tcx.hir().get(trait_hir_id) {
        Node::Item(item) => item,
        _ => bug!("trait_node_id {} is not an item", trait_hir_id),
    };

    let (generics, bounds) = match item.kind {
        hir::ItemKind::Trait(.., ref generics, ref supertraits, _) => (generics, supertraits),
        hir::ItemKind::TraitAlias(ref generics, ref supertraits) => (generics, supertraits),
        _ => span_bug!(item.span, "super_predicates invoked on non-trait"),
    };

    let icx = ItemCtxt::new(tcx, trait_def_id);

    // Convert the bounds that follow the colon, e.g., `Bar + Zed` in `trait Foo: Bar + Zed`.
    let self_param_ty = tcx.types.self_param;
    let superbounds1 =
        AstConv::compute_bounds(&icx, self_param_ty, bounds, SizedByDefault::No, item.span);

    let superbounds1 = superbounds1.predicates(tcx, self_param_ty);

    // Convert any explicit superbounds in the where-clause,
    // e.g., `trait Foo where Self: Bar`.
    // In the case of trait aliases, however, we include all bounds in the where-clause,
    // so e.g., `trait Foo = where u32: PartialEq<Self>` would include `u32: PartialEq<Self>`
    // as one of its "superpredicates".
    let is_trait_alias = tcx.is_trait_alias(trait_def_id);
    let superbounds2 = icx.type_parameter_bounds_in_generics(
        generics,
        item.hir_id,
        self_param_ty,
        OnlySelfBounds(!is_trait_alias),
    );

    // Combine the two lists to form the complete set of superbounds:
    let superbounds = &*tcx.arena.alloc_from_iter(superbounds1.into_iter().chain(superbounds2));

    // Now require that immediate supertraits are converted,
    // which will, in turn, reach indirect supertraits.
    for &(pred, span) in superbounds {
        debug!("superbound: {:?}", pred);
        if let ty::Predicate::Trait(bound, _) = pred {
            tcx.at(span).super_predicates_of(bound.def_id());
        }
    }

    ty::GenericPredicates { parent: None, predicates: superbounds }
}

fn trait_def(tcx: TyCtxt<'_>, def_id: DefId) -> &ty::TraitDef {
    let hir_id = tcx.hir().as_local_hir_id(def_id).unwrap();
    let item = tcx.hir().expect_item(hir_id);

    let (is_auto, unsafety) = match item.kind {
        hir::ItemKind::Trait(is_auto, unsafety, ..) => (is_auto == hir::IsAuto::Yes, unsafety),
        hir::ItemKind::TraitAlias(..) => (false, hir::Unsafety::Normal),
        _ => span_bug!(item.span, "trait_def_of_item invoked on non-trait"),
    };

    let paren_sugar = tcx.has_attr(def_id, sym::rustc_paren_sugar);
    if paren_sugar && !tcx.features().unboxed_closures {
        tcx.sess
            .struct_span_err(
                item.span,
                "the `#[rustc_paren_sugar]` attribute is a temporary means of controlling \
                 which traits can use parenthetical notation",
            )
            .help("add `#![feature(unboxed_closures)]` to the crate attributes to use it")
            .emit();
    }

    let is_marker = tcx.has_attr(def_id, sym::marker);
    let spec_kind = if tcx.has_attr(def_id, sym::rustc_unsafe_specialization_marker) {
        ty::trait_def::TraitSpecializationKind::Marker
    } else if tcx.has_attr(def_id, sym::rustc_specialization_trait) {
        ty::trait_def::TraitSpecializationKind::AlwaysApplicable
    } else {
        ty::trait_def::TraitSpecializationKind::None
    };
    let def_path_hash = tcx.def_path_hash(def_id);
    let def = ty::TraitDef::new(
        def_id,
        unsafety,
        paren_sugar,
        is_auto,
        is_marker,
        spec_kind,
        def_path_hash,
    );
    tcx.arena.alloc(def)
}

fn has_late_bound_regions<'tcx>(tcx: TyCtxt<'tcx>, node: Node<'tcx>) -> Option<Span> {
    struct LateBoundRegionsDetector<'tcx> {
        tcx: TyCtxt<'tcx>,
        outer_index: ty::DebruijnIndex,
        has_late_bound_regions: Option<Span>,
    }

    impl Visitor<'tcx> for LateBoundRegionsDetector<'tcx> {
        type Map = intravisit::ErasedMap<'tcx>;

        fn nested_visit_map(&mut self) -> NestedVisitorMap<Self::Map> {
            NestedVisitorMap::None
        }

        fn visit_ty(&mut self, ty: &'tcx hir::Ty<'tcx>) {
            if self.has_late_bound_regions.is_some() {
                return;
            }
            match ty.kind {
                hir::TyKind::BareFn(..) => {
                    self.outer_index.shift_in(1);
                    intravisit::walk_ty(self, ty);
                    self.outer_index.shift_out(1);
                }
                _ => intravisit::walk_ty(self, ty),
            }
        }

        fn visit_poly_trait_ref(
            &mut self,
            tr: &'tcx hir::PolyTraitRef<'tcx>,
            m: hir::TraitBoundModifier,
        ) {
            if self.has_late_bound_regions.is_some() {
                return;
            }
            self.outer_index.shift_in(1);
            intravisit::walk_poly_trait_ref(self, tr, m);
            self.outer_index.shift_out(1);
        }

        fn visit_lifetime(&mut self, lt: &'tcx hir::Lifetime) {
            if self.has_late_bound_regions.is_some() {
                return;
            }

            match self.tcx.named_region(lt.hir_id) {
                Some(rl::Region::Static) | Some(rl::Region::EarlyBound(..)) => {}
                Some(rl::Region::LateBound(debruijn, _, _))
                | Some(rl::Region::LateBoundAnon(debruijn, _))
                    if debruijn < self.outer_index => {}
                Some(rl::Region::LateBound(..))
                | Some(rl::Region::LateBoundAnon(..))
                | Some(rl::Region::Free(..))
                | None => {
                    self.has_late_bound_regions = Some(lt.span);
                }
            }
        }
    }

    fn has_late_bound_regions<'tcx>(
        tcx: TyCtxt<'tcx>,
        generics: &'tcx hir::Generics<'tcx>,
        decl: &'tcx hir::FnDecl<'tcx>,
    ) -> Option<Span> {
        let mut visitor = LateBoundRegionsDetector {
            tcx,
            outer_index: ty::INNERMOST,
            has_late_bound_regions: None,
        };
        for param in generics.params {
            if let GenericParamKind::Lifetime { .. } = param.kind {
                if tcx.is_late_bound(param.hir_id) {
                    return Some(param.span);
                }
            }
        }
        visitor.visit_fn_decl(decl);
        visitor.has_late_bound_regions
    }

    match node {
        Node::TraitItem(item) => match item.kind {
            hir::TraitItemKind::Fn(ref sig, _) => {
                has_late_bound_regions(tcx, &item.generics, &sig.decl)
            }
            _ => None,
        },
        Node::ImplItem(item) => match item.kind {
            hir::ImplItemKind::Fn(ref sig, _) => {
                has_late_bound_regions(tcx, &item.generics, &sig.decl)
            }
            _ => None,
        },
        Node::ForeignItem(item) => match item.kind {
            hir::ForeignItemKind::Fn(ref fn_decl, _, ref generics) => {
                has_late_bound_regions(tcx, generics, fn_decl)
            }
            _ => None,
        },
        Node::Item(item) => match item.kind {
            hir::ItemKind::Fn(ref sig, .., ref generics, _) => {
                has_late_bound_regions(tcx, generics, &sig.decl)
            }
            _ => None,
        },
        _ => None,
    }
}

fn generics_of(tcx: TyCtxt<'_>, def_id: DefId) -> &ty::Generics {
    use rustc_hir::*;

    let hir_id = tcx.hir().as_local_hir_id(def_id).unwrap();

    let node = tcx.hir().get(hir_id);
    let parent_def_id = match node {
        Node::ImplItem(_)
        | Node::TraitItem(_)
        | Node::Variant(_)
        | Node::Ctor(..)
        | Node::Field(_) => {
            let parent_id = tcx.hir().get_parent_item(hir_id);
            Some(tcx.hir().local_def_id(parent_id))
        }
        // FIXME(#43408) enable this always when we get lazy normalization.
        Node::AnonConst(_) => {
            // HACK(eddyb) this provides the correct generics when
            // `feature(const_generics)` is enabled, so that const expressions
            // used with const generics, e.g. `Foo<{N+1}>`, can work at all.
            if tcx.features().const_generics {
                let parent_id = tcx.hir().get_parent_item(hir_id);
                Some(tcx.hir().local_def_id(parent_id))
            } else {
                None
            }
        }
        Node::Expr(&hir::Expr { kind: hir::ExprKind::Closure(..), .. }) => {
            Some(tcx.closure_base_def_id(def_id))
        }
        Node::Item(item) => match item.kind {
            ItemKind::OpaqueTy(hir::OpaqueTy { impl_trait_fn, .. }) => {
                impl_trait_fn.or_else(|| {
                    let parent_id = tcx.hir().get_parent_item(hir_id);
                    if parent_id != hir_id && parent_id != CRATE_HIR_ID {
                        debug!("generics_of: parent of opaque ty {:?} is {:?}", def_id, parent_id);
                        // If this 'impl Trait' is nested inside another 'impl Trait'
                        // (e.g. `impl Foo<MyType = impl Bar<A>>`), we need to use the 'parent'
                        // 'impl Trait' for its generic parameters, since we can reference them
                        // from the 'child' 'impl Trait'
                        if let Node::Item(hir::Item { kind: ItemKind::OpaqueTy(..), .. }) =
                            tcx.hir().get(parent_id)
                        {
                            Some(tcx.hir().local_def_id(parent_id))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
            }
            _ => None,
        },
        _ => None,
    };

    let mut opt_self = None;
    let mut allow_defaults = false;

    let no_generics = hir::Generics::empty();
    let ast_generics = match node {
        Node::TraitItem(item) => &item.generics,

        Node::ImplItem(item) => &item.generics,

        Node::Item(item) => {
            match item.kind {
                ItemKind::Fn(.., ref generics, _) | ItemKind::Impl { ref generics, .. } => generics,

                ItemKind::TyAlias(_, ref generics)
                | ItemKind::Enum(_, ref generics)
                | ItemKind::Struct(_, ref generics)
                | ItemKind::OpaqueTy(hir::OpaqueTy { ref generics, .. })
                | ItemKind::Union(_, ref generics) => {
                    allow_defaults = true;
                    generics
                }

                ItemKind::Trait(_, _, ref generics, ..)
                | ItemKind::TraitAlias(ref generics, ..) => {
                    // Add in the self type parameter.
                    //
                    // Something of a hack: use the node id for the trait, also as
                    // the node id for the Self type parameter.
                    let param_id = item.hir_id;

                    opt_self = Some(ty::GenericParamDef {
                        index: 0,
                        name: kw::SelfUpper,
                        def_id: tcx.hir().local_def_id(param_id),
                        pure_wrt_drop: false,
                        kind: ty::GenericParamDefKind::Type {
                            has_default: false,
                            object_lifetime_default: rl::Set1::Empty,
                            synthetic: None,
                        },
                    });

                    allow_defaults = true;
                    generics
                }

                _ => &no_generics,
            }
        }

        Node::ForeignItem(item) => match item.kind {
            ForeignItemKind::Static(..) => &no_generics,
            ForeignItemKind::Fn(_, _, ref generics) => generics,
            ForeignItemKind::Type => &no_generics,
        },

        _ => &no_generics,
    };

    let has_self = opt_self.is_some();
    let mut parent_has_self = false;
    let mut own_start = has_self as u32;
    let parent_count = parent_def_id.map_or(0, |def_id| {
        let generics = tcx.generics_of(def_id);
        assert_eq!(has_self, false);
        parent_has_self = generics.has_self;
        own_start = generics.count() as u32;
        generics.parent_count + generics.params.len()
    });

    let mut params: Vec<_> = opt_self.into_iter().collect();

    let early_lifetimes = early_bound_lifetimes_from_generics(tcx, ast_generics);
    params.extend(early_lifetimes.enumerate().map(|(i, param)| ty::GenericParamDef {
        name: param.name.ident().name,
        index: own_start + i as u32,
        def_id: tcx.hir().local_def_id(param.hir_id),
        pure_wrt_drop: param.pure_wrt_drop,
        kind: ty::GenericParamDefKind::Lifetime,
    }));

    let object_lifetime_defaults = tcx.object_lifetime_defaults(hir_id);

    // Now create the real type and const parameters.
    let type_start = own_start - has_self as u32 + params.len() as u32;
    let mut i = 0;

    // FIXME(const_generics): a few places in the compiler expect generic params
    // to be in the order lifetimes, then type params, then const params.
    //
    // To prevent internal errors in case const parameters are supplied before
    // type parameters we first add all type params, then all const params.
    params.extend(ast_generics.params.iter().filter_map(|param| {
        if let GenericParamKind::Type { ref default, synthetic, .. } = param.kind {
            if !allow_defaults && default.is_some() {
                if !tcx.features().default_type_parameter_fallback {
                    tcx.struct_span_lint_hir(
                        lint::builtin::INVALID_TYPE_PARAM_DEFAULT,
                        param.hir_id,
                        param.span,
                        |lint| {
                            lint.build(
                                "defaults for type parameters are only allowed in \
                                        `struct`, `enum`, `type`, or `trait` definitions.",
                            )
                            .emit();
                        },
                    );
                }
            }

            let kind = ty::GenericParamDefKind::Type {
                has_default: default.is_some(),
                object_lifetime_default: object_lifetime_defaults
                    .as_ref()
                    .map_or(rl::Set1::Empty, |o| o[i]),
                synthetic,
            };

            let param_def = ty::GenericParamDef {
                index: type_start + i as u32,
                name: param.name.ident().name,
                def_id: tcx.hir().local_def_id(param.hir_id),
                pure_wrt_drop: param.pure_wrt_drop,
                kind,
            };
            i += 1;
            Some(param_def)
        } else {
            None
        }
    }));

    params.extend(ast_generics.params.iter().filter_map(|param| {
        if let GenericParamKind::Const { .. } = param.kind {
            let param_def = ty::GenericParamDef {
                index: type_start + i as u32,
                name: param.name.ident().name,
                def_id: tcx.hir().local_def_id(param.hir_id),
                pure_wrt_drop: param.pure_wrt_drop,
                kind: ty::GenericParamDefKind::Const,
            };
            i += 1;
            Some(param_def)
        } else {
            None
        }
    }));

    // provide junk type parameter defs - the only place that
    // cares about anything but the length is instantiation,
    // and we don't do that for closures.
    if let Node::Expr(&hir::Expr { kind: hir::ExprKind::Closure(.., gen), .. }) = node {
        let dummy_args = if gen.is_some() {
            &["<resume_ty>", "<yield_ty>", "<return_ty>", "<witness>", "<upvars>"][..]
        } else {
            &["<closure_kind>", "<closure_signature>", "<upvars>"][..]
        };

        params.extend(dummy_args.iter().enumerate().map(|(i, &arg)| ty::GenericParamDef {
            index: type_start + i as u32,
            name: Symbol::intern(arg),
            def_id,
            pure_wrt_drop: false,
            kind: ty::GenericParamDefKind::Type {
                has_default: false,
                object_lifetime_default: rl::Set1::Empty,
                synthetic: None,
            },
        }));
    }

    let param_def_id_to_index = params.iter().map(|param| (param.def_id, param.index)).collect();

    tcx.arena.alloc(ty::Generics {
        parent: parent_def_id,
        parent_count,
        params,
        param_def_id_to_index,
        has_self: has_self || parent_has_self,
        has_late_bound_regions: has_late_bound_regions(tcx, node),
    })
}

fn are_suggestable_generic_args(generic_args: &[hir::GenericArg<'_>]) -> bool {
    generic_args
        .iter()
        .filter_map(|arg| match arg {
            hir::GenericArg::Type(ty) => Some(ty),
            _ => None,
        })
        .any(is_suggestable_infer_ty)
}

/// Whether `ty` is a type with `_` placeholders that can be inferred. Used in diagnostics only to
/// use inference to provide suggestions for the appropriate type if possible.
fn is_suggestable_infer_ty(ty: &hir::Ty<'_>) -> bool {
    use hir::TyKind::*;
    match &ty.kind {
        Infer => true,
        Slice(ty) | Array(ty, _) => is_suggestable_infer_ty(ty),
        Tup(tys) => tys.iter().any(is_suggestable_infer_ty),
        Ptr(mut_ty) | Rptr(_, mut_ty) => is_suggestable_infer_ty(mut_ty.ty),
        Def(_, generic_args) => are_suggestable_generic_args(generic_args),
        Path(hir::QPath::TypeRelative(ty, segment)) => {
            is_suggestable_infer_ty(ty) || are_suggestable_generic_args(segment.generic_args().args)
        }
        Path(hir::QPath::Resolved(ty_opt, hir::Path { segments, .. })) => {
            ty_opt.map_or(false, is_suggestable_infer_ty)
                || segments
                    .iter()
                    .any(|segment| are_suggestable_generic_args(segment.generic_args().args))
        }
        _ => false,
    }
}

pub fn get_infer_ret_ty(output: &'hir hir::FnRetTy<'hir>) -> Option<&'hir hir::Ty<'hir>> {
    if let hir::FnRetTy::Return(ref ty) = output {
        if is_suggestable_infer_ty(ty) {
            return Some(&**ty);
        }
    }
    None
}

fn fn_sig(tcx: TyCtxt<'_>, def_id: DefId) -> ty::PolyFnSig<'_> {
    use rustc_hir::Node::*;
    use rustc_hir::*;

    let hir_id = tcx.hir().as_local_hir_id(def_id).unwrap();

    let icx = ItemCtxt::new(tcx, def_id);

    match tcx.hir().get(hir_id) {
        TraitItem(hir::TraitItem {
            kind: TraitItemKind::Fn(sig, TraitFn::Provided(_)),
            ident,
            generics,
            ..
        })
        | ImplItem(hir::ImplItem { kind: ImplItemKind::Fn(sig, _), ident, generics, .. })
        | Item(hir::Item { kind: ItemKind::Fn(sig, generics, _), ident, .. }) => {
            match get_infer_ret_ty(&sig.decl.output) {
                Some(ty) => {
                    let fn_sig = tcx.typeck_tables_of(def_id).liberated_fn_sigs()[hir_id];
                    let mut visitor = PlaceholderHirTyCollector::default();
                    visitor.visit_ty(ty);
                    let mut diag = bad_placeholder_type(tcx, visitor.0);
                    let ret_ty = fn_sig.output();
                    if ret_ty != tcx.types.err {
                        diag.span_suggestion(
                            ty.span,
                            "replace with the correct return type",
                            ret_ty.to_string(),
                            Applicability::MaybeIncorrect,
                        );
                    }
                    diag.emit();
                    ty::Binder::bind(fn_sig)
                }
                None => AstConv::ty_of_fn(
                    &icx,
                    sig.header.unsafety,
                    sig.header.abi,
                    &sig.decl,
                    &generics,
                    Some(ident.span),
                ),
            }
        }

        TraitItem(hir::TraitItem {
            kind: TraitItemKind::Fn(FnSig { header, decl }, _),
            ident,
            generics,
            ..
        }) => {
            AstConv::ty_of_fn(&icx, header.unsafety, header.abi, decl, &generics, Some(ident.span))
        }

        ForeignItem(&hir::ForeignItem { kind: ForeignItemKind::Fn(ref fn_decl, _, _), .. }) => {
            let abi = tcx.hir().get_foreign_abi(hir_id);
            compute_sig_of_foreign_fn_decl(tcx, def_id, fn_decl, abi)
        }

        Ctor(data) | Variant(hir::Variant { data, .. }) if data.ctor_hir_id().is_some() => {
            let ty = tcx.type_of(tcx.hir().get_parent_did(hir_id));
            let inputs =
                data.fields().iter().map(|f| tcx.type_of(tcx.hir().local_def_id(f.hir_id)));
            ty::Binder::bind(tcx.mk_fn_sig(
                inputs,
                ty,
                false,
                hir::Unsafety::Normal,
                abi::Abi::Rust,
            ))
        }

        Expr(&hir::Expr { kind: hir::ExprKind::Closure(..), .. }) => {
            // Closure signatures are not like other function
            // signatures and cannot be accessed through `fn_sig`. For
            // example, a closure signature excludes the `self`
            // argument. In any case they are embedded within the
            // closure type as part of the `ClosureSubsts`.
            //
            // To get the signature of a closure, you should use the
            // `sig` method on the `ClosureSubsts`:
            //
            //    substs.as_closure().sig(def_id, tcx)
            bug!(
                "to get the signature of a closure, use `substs.as_closure().sig()` not `fn_sig()`",
            );
        }

        x => {
            bug!("unexpected sort of node in fn_sig(): {:?}", x);
        }
    }
}

fn impl_trait_ref(tcx: TyCtxt<'_>, def_id: DefId) -> Option<ty::TraitRef<'_>> {
    let icx = ItemCtxt::new(tcx, def_id);

    let hir_id = tcx.hir().as_local_hir_id(def_id).unwrap();
    match tcx.hir().expect_item(hir_id).kind {
        hir::ItemKind::Impl { ref of_trait, .. } => of_trait.as_ref().map(|ast_trait_ref| {
            let selfty = tcx.type_of(def_id);
            AstConv::instantiate_mono_trait_ref(&icx, ast_trait_ref, selfty)
        }),
        _ => bug!(),
    }
}

fn impl_polarity(tcx: TyCtxt<'_>, def_id: DefId) -> ty::ImplPolarity {
    let hir_id = tcx.hir().as_local_hir_id(def_id).unwrap();
    let is_rustc_reservation = tcx.has_attr(def_id, sym::rustc_reservation_impl);
    let item = tcx.hir().expect_item(hir_id);
    match &item.kind {
        hir::ItemKind::Impl { polarity: hir::ImplPolarity::Negative(span), of_trait, .. } => {
            if is_rustc_reservation {
                let span = span.to(of_trait.as_ref().map(|t| t.path.span).unwrap_or(*span));
                tcx.sess.span_err(span, "reservation impls can't be negative");
            }
            ty::ImplPolarity::Negative
        }
        hir::ItemKind::Impl { polarity: hir::ImplPolarity::Positive, of_trait: None, .. } => {
            if is_rustc_reservation {
                tcx.sess.span_err(item.span, "reservation impls can't be inherent");
            }
            ty::ImplPolarity::Positive
        }
        hir::ItemKind::Impl {
            polarity: hir::ImplPolarity::Positive, of_trait: Some(_), ..
        } => {
            if is_rustc_reservation {
                ty::ImplPolarity::Reservation
            } else {
                ty::ImplPolarity::Positive
            }
        }
        ref item => bug!("impl_polarity: {:?} not an impl", item),
    }
}

/// Returns the early-bound lifetimes declared in this generics
/// listing. For anything other than fns/methods, this is just all
/// the lifetimes that are declared. For fns or methods, we have to
/// screen out those that do not appear in any where-clauses etc using
/// `resolve_lifetime::early_bound_lifetimes`.
fn early_bound_lifetimes_from_generics<'a, 'tcx: 'a>(
    tcx: TyCtxt<'tcx>,
    generics: &'a hir::Generics<'a>,
) -> impl Iterator<Item = &'a hir::GenericParam<'a>> + Captures<'tcx> {
    generics.params.iter().filter(move |param| match param.kind {
        GenericParamKind::Lifetime { .. } => !tcx.is_late_bound(param.hir_id),
        _ => false,
    })
}

/// Returns a list of type predicates for the definition with ID `def_id`, including inferred
/// lifetime constraints. This includes all predicates returned by `explicit_predicates_of`, plus
/// inferred constraints concerning which regions outlive other regions.
fn predicates_defined_on(tcx: TyCtxt<'_>, def_id: DefId) -> ty::GenericPredicates<'_> {
    debug!("predicates_defined_on({:?})", def_id);
    let mut result = tcx.explicit_predicates_of(def_id);
    debug!("predicates_defined_on: explicit_predicates_of({:?}) = {:?}", def_id, result,);
    let inferred_outlives = tcx.inferred_outlives_of(def_id);
    if !inferred_outlives.is_empty() {
        debug!(
            "predicates_defined_on: inferred_outlives_of({:?}) = {:?}",
            def_id, inferred_outlives,
        );
        if result.predicates.is_empty() {
            result.predicates = inferred_outlives;
        } else {
            result.predicates = tcx
                .arena
                .alloc_from_iter(result.predicates.iter().chain(inferred_outlives).copied());
        }
    }
    debug!("predicates_defined_on({:?}) = {:?}", def_id, result);
    result
}

/// Returns a list of all type predicates (explicit and implicit) for the definition with
/// ID `def_id`. This includes all predicates returned by `predicates_defined_on`, plus
/// `Self: Trait` predicates for traits.
fn predicates_of(tcx: TyCtxt<'_>, def_id: DefId) -> ty::GenericPredicates<'_> {
    let mut result = tcx.predicates_defined_on(def_id);

    if tcx.is_trait(def_id) {
        // For traits, add `Self: Trait` predicate. This is
        // not part of the predicates that a user writes, but it
        // is something that one must prove in order to invoke a
        // method or project an associated type.
        //
        // In the chalk setup, this predicate is not part of the
        // "predicates" for a trait item. But it is useful in
        // rustc because if you directly (e.g.) invoke a trait
        // method like `Trait::method(...)`, you must naturally
        // prove that the trait applies to the types that were
        // used, and adding the predicate into this list ensures
        // that this is done.
        let span = tcx.def_span(def_id);
        result.predicates =
            tcx.arena.alloc_from_iter(result.predicates.iter().copied().chain(std::iter::once((
                ty::TraitRef::identity(tcx, def_id).without_const().to_predicate(),
                span,
            ))));
    }
    debug!("predicates_of(def_id={:?}) = {:?}", def_id, result);
    result
}

/// Returns a list of user-specified type predicates for the definition with ID `def_id`.
/// N.B., this does not include any implied/inferred constraints.
fn explicit_predicates_of(tcx: TyCtxt<'_>, def_id: DefId) -> ty::GenericPredicates<'_> {
    use rustc_hir::*;

    debug!("explicit_predicates_of(def_id={:?})", def_id);

    /// A data structure with unique elements, which preserves order of insertion.
    /// Preserving the order of insertion is important here so as not to break
    /// compile-fail UI tests.
    // FIXME(eddyb) just use `IndexSet` from `indexmap`.
    struct UniquePredicates<'tcx> {
        predicates: Vec<(ty::Predicate<'tcx>, Span)>,
        uniques: FxHashSet<(ty::Predicate<'tcx>, Span)>,
    }

    impl<'tcx> UniquePredicates<'tcx> {
        fn new() -> Self {
            UniquePredicates { predicates: vec![], uniques: FxHashSet::default() }
        }

        fn push(&mut self, value: (ty::Predicate<'tcx>, Span)) {
            if self.uniques.insert(value) {
                self.predicates.push(value);
            }
        }

        fn extend<I: IntoIterator<Item = (ty::Predicate<'tcx>, Span)>>(&mut self, iter: I) {
            for value in iter {
                self.push(value);
            }
        }
    }

    let hir_id = tcx.hir().as_local_hir_id(def_id).unwrap();
    let node = tcx.hir().get(hir_id);

    let mut is_trait = None;
    let mut is_default_impl_trait = None;

    let icx = ItemCtxt::new(tcx, def_id);
    let constness = icx.default_constness_for_trait_bounds();

    const NO_GENERICS: &hir::Generics<'_> = &hir::Generics::empty();

    let mut predicates = UniquePredicates::new();

    let ast_generics = match node {
        Node::TraitItem(item) => &item.generics,

        Node::ImplItem(item) => match item.kind {
            ImplItemKind::OpaqueTy(ref bounds) => {
                ty::print::with_no_queries(|| {
                    let substs = InternalSubsts::identity_for_item(tcx, def_id);
                    let opaque_ty = tcx.mk_opaque(def_id, substs);
                    debug!(
                        "explicit_predicates_of({:?}): created opaque type {:?}",
                        def_id, opaque_ty
                    );

                    // Collect the bounds, i.e., the `A + B + 'c` in `impl A + B + 'c`.
                    let bounds = AstConv::compute_bounds(
                        &icx,
                        opaque_ty,
                        bounds,
                        SizedByDefault::Yes,
                        tcx.def_span(def_id),
                    );

                    predicates.extend(bounds.predicates(tcx, opaque_ty));
                    &item.generics
                })
            }
            _ => &item.generics,
        },

        Node::Item(item) => {
            match item.kind {
                ItemKind::Impl { defaultness, ref generics, .. } => {
                    if defaultness.is_default() {
                        is_default_impl_trait = tcx.impl_trait_ref(def_id);
                    }
                    generics
                }
                ItemKind::Fn(.., ref generics, _)
                | ItemKind::TyAlias(_, ref generics)
                | ItemKind::Enum(_, ref generics)
                | ItemKind::Struct(_, ref generics)
                | ItemKind::Union(_, ref generics) => generics,

                ItemKind::Trait(_, _, ref generics, .., items) => {
                    is_trait = Some((ty::TraitRef::identity(tcx, def_id), items));
                    generics
                }
                ItemKind::TraitAlias(ref generics, _) => {
                    is_trait = Some((ty::TraitRef::identity(tcx, def_id), &[]));
                    generics
                }
                ItemKind::OpaqueTy(OpaqueTy {
                    ref bounds,
                    impl_trait_fn,
                    ref generics,
                    origin: _,
                }) => {
                    let bounds_predicates = ty::print::with_no_queries(|| {
                        let substs = InternalSubsts::identity_for_item(tcx, def_id);
                        let opaque_ty = tcx.mk_opaque(def_id, substs);

                        // Collect the bounds, i.e., the `A + B + 'c` in `impl A + B + 'c`.
                        let bounds = AstConv::compute_bounds(
                            &icx,
                            opaque_ty,
                            bounds,
                            SizedByDefault::Yes,
                            tcx.def_span(def_id),
                        );

                        bounds.predicates(tcx, opaque_ty)
                    });
                    if impl_trait_fn.is_some() {
                        // opaque types
                        return ty::GenericPredicates {
                            parent: None,
                            predicates: tcx.arena.alloc_from_iter(bounds_predicates),
                        };
                    } else {
                        // named opaque types
                        predicates.extend(bounds_predicates);
                        generics
                    }
                }

                _ => NO_GENERICS,
            }
        }

        Node::ForeignItem(item) => match item.kind {
            ForeignItemKind::Static(..) => NO_GENERICS,
            ForeignItemKind::Fn(_, _, ref generics) => generics,
            ForeignItemKind::Type => NO_GENERICS,
        },

        _ => NO_GENERICS,
    };

    let generics = tcx.generics_of(def_id);
    let parent_count = generics.parent_count as u32;
    let has_own_self = generics.has_self && parent_count == 0;

    // Below we'll consider the bounds on the type parameters (including `Self`)
    // and the explicit where-clauses, but to get the full set of predicates
    // on a trait we need to add in the supertrait bounds and bounds found on
    // associated types.
    if let Some((_trait_ref, _)) = is_trait {
        predicates.extend(tcx.super_predicates_of(def_id).predicates.iter().cloned());
    }

    // In default impls, we can assume that the self type implements
    // the trait. So in:
    //
    //     default impl Foo for Bar { .. }
    //
    // we add a default where clause `Foo: Bar`. We do a similar thing for traits
    // (see below). Recall that a default impl is not itself an impl, but rather a
    // set of defaults that can be incorporated into another impl.
    if let Some(trait_ref) = is_default_impl_trait {
        predicates.push((
            trait_ref.to_poly_trait_ref().without_const().to_predicate(),
            tcx.def_span(def_id),
        ));
    }

    // Collect the region predicates that were declared inline as
    // well. In the case of parameters declared on a fn or method, we
    // have to be careful to only iterate over early-bound regions.
    let mut index = parent_count + has_own_self as u32;
    for param in early_bound_lifetimes_from_generics(tcx, ast_generics) {
        let region = tcx.mk_region(ty::ReEarlyBound(ty::EarlyBoundRegion {
            def_id: tcx.hir().local_def_id(param.hir_id),
            index,
            name: param.name.ident().name,
        }));
        index += 1;

        match param.kind {
            GenericParamKind::Lifetime { .. } => {
                param.bounds.iter().for_each(|bound| match bound {
                    hir::GenericBound::Outlives(lt) => {
                        let bound = AstConv::ast_region_to_region(&icx, &lt, None);
                        let outlives = ty::Binder::bind(ty::OutlivesPredicate(region, bound));
                        predicates.push((outlives.to_predicate(), lt.span));
                    }
                    _ => bug!(),
                });
            }
            _ => bug!(),
        }
    }

    // Collect the predicates that were written inline by the user on each
    // type parameter (e.g., `<T: Foo>`).
    for param in ast_generics.params {
        if let GenericParamKind::Type { .. } = param.kind {
            let name = param.name.ident().name;
            let param_ty = ty::ParamTy::new(index, name).to_ty(tcx);
            index += 1;

            let sized = SizedByDefault::Yes;
            let bounds = AstConv::compute_bounds(&icx, param_ty, &param.bounds, sized, param.span);
            predicates.extend(bounds.predicates(tcx, param_ty));
        }
    }

    // Add in the bounds that appear in the where-clause.
    let where_clause = &ast_generics.where_clause;
    for predicate in where_clause.predicates {
        match predicate {
            &hir::WherePredicate::BoundPredicate(ref bound_pred) => {
                let ty = icx.to_ty(&bound_pred.bounded_ty);

                // Keep the type around in a dummy predicate, in case of no bounds.
                // That way, `where Ty:` is not a complete noop (see #53696) and `Ty`
                // is still checked for WF.
                if bound_pred.bounds.is_empty() {
                    if let ty::Param(_) = ty.kind {
                        // This is a `where T:`, which can be in the HIR from the
                        // transformation that moves `?Sized` to `T`'s declaration.
                        // We can skip the predicate because type parameters are
                        // trivially WF, but also we *should*, to avoid exposing
                        // users who never wrote `where Type:,` themselves, to
                        // compiler/tooling bugs from not handling WF predicates.
                    } else {
                        let span = bound_pred.bounded_ty.span;
                        let re_root_empty = tcx.lifetimes.re_root_empty;
                        let predicate = ty::OutlivesPredicate(ty, re_root_empty);
                        predicates.push((
                            ty::Predicate::TypeOutlives(ty::Binder::dummy(predicate)),
                            span,
                        ));
                    }
                }

                for bound in bound_pred.bounds.iter() {
                    match bound {
                        &hir::GenericBound::Trait(ref poly_trait_ref, modifier) => {
                            let constness = match modifier {
                                hir::TraitBoundModifier::MaybeConst => hir::Constness::NotConst,
                                hir::TraitBoundModifier::None => constness,
                                hir::TraitBoundModifier::Maybe => bug!("this wasn't handled"),
                            };

                            let mut bounds = Bounds::default();
                            let _ = AstConv::instantiate_poly_trait_ref(
                                &icx,
                                poly_trait_ref,
                                constness,
                                ty,
                                &mut bounds,
                            );
                            predicates.extend(bounds.predicates(tcx, ty));
                        }

                        &hir::GenericBound::Outlives(ref lifetime) => {
                            let region = AstConv::ast_region_to_region(&icx, lifetime, None);
                            let pred = ty::Binder::bind(ty::OutlivesPredicate(ty, region));
                            predicates.push((ty::Predicate::TypeOutlives(pred), lifetime.span))
                        }
                    }
                }
            }

            &hir::WherePredicate::RegionPredicate(ref region_pred) => {
                let r1 = AstConv::ast_region_to_region(&icx, &region_pred.lifetime, None);
                predicates.extend(region_pred.bounds.iter().map(|bound| {
                    let (r2, span) = match bound {
                        hir::GenericBound::Outlives(lt) => {
                            (AstConv::ast_region_to_region(&icx, lt, None), lt.span)
                        }
                        _ => bug!(),
                    };
                    let pred = ty::Binder::bind(ty::OutlivesPredicate(r1, r2));

                    (ty::Predicate::RegionOutlives(pred), span)
                }))
            }

            &hir::WherePredicate::EqPredicate(..) => {
                // FIXME(#20041)
            }
        }
    }

    // Add predicates from associated type bounds.
    if let Some((self_trait_ref, trait_items)) = is_trait {
        predicates.extend(trait_items.iter().flat_map(|trait_item_ref| {
            associated_item_predicates(tcx, def_id, self_trait_ref, trait_item_ref)
        }))
    }

    let mut predicates = predicates.predicates;

    // Subtle: before we store the predicates into the tcx, we
    // sort them so that predicates like `T: Foo<Item=U>` come
    // before uses of `U`.  This avoids false ambiguity errors
    // in trait checking. See `setup_constraining_predicates`
    // for details.
    if let Node::Item(&Item { kind: ItemKind::Impl { .. }, .. }) = node {
        let self_ty = tcx.type_of(def_id);
        let trait_ref = tcx.impl_trait_ref(def_id);
        cgp::setup_constraining_predicates(
            tcx,
            &mut predicates,
            trait_ref,
            &mut cgp::parameters_for_impl(self_ty, trait_ref),
        );
    }

    let result = ty::GenericPredicates {
        parent: generics.parent,
        predicates: tcx.arena.alloc_from_iter(predicates),
    };
    debug!("explicit_predicates_of(def_id={:?}) = {:?}", def_id, result);
    result
}

fn associated_item_predicates(
    tcx: TyCtxt<'tcx>,
    def_id: DefId,
    self_trait_ref: ty::TraitRef<'tcx>,
    trait_item_ref: &hir::TraitItemRef,
) -> Vec<(ty::Predicate<'tcx>, Span)> {
    let trait_item = tcx.hir().trait_item(trait_item_ref.id);
    let item_def_id = tcx.hir().local_def_id(trait_item_ref.id.hir_id);
    let bounds = match trait_item.kind {
        hir::TraitItemKind::Type(ref bounds, _) => bounds,
        _ => return Vec::new(),
    };

    let is_gat = !tcx.generics_of(item_def_id).params.is_empty();

    let mut had_error = false;

    let mut unimplemented_error = |arg_kind: &str| {
        if !had_error {
            tcx.sess
                .struct_span_err(
                    trait_item.span,
                    &format!("{}-generic associated types are not yet implemented", arg_kind),
                )
                .note(
                    "for more information, see issue #44265 \
                     <https://github.com/rust-lang/rust/issues/44265> for more information",
                )
                .emit();
            had_error = true;
        }
    };

    let mk_bound_param = |param: &ty::GenericParamDef, _: &_| {
        match param.kind {
            ty::GenericParamDefKind::Lifetime => tcx
                .mk_region(ty::RegionKind::ReLateBound(
                    ty::INNERMOST,
                    ty::BoundRegion::BrNamed(param.def_id, param.name),
                ))
                .into(),
            // FIXME(generic_associated_types): Use bound types and constants
            // once they are handled by the trait system.
            ty::GenericParamDefKind::Type { .. } => {
                unimplemented_error("type");
                tcx.types.err.into()
            }
            ty::GenericParamDefKind::Const => {
                unimplemented_error("const");
                tcx.consts.err.into()
            }
        }
    };

    let bound_substs = if is_gat {
        // Given:
        //
        // trait X<'a, B, const C: usize> {
        //     type T<'d, E, const F: usize>: Default;
        // }
        //
        // We need to create predicates on the trait:
        //
        // for<'d, E, const F: usize>
        // <Self as X<'a, B, const C: usize>>::T<'d, E, const F: usize>: Sized + Default
        //
        // We substitute escaping bound parameters for the generic
        // arguments to the associated type which are then bound by
        // the `Binder` around the the predicate.
        //
        // FIXME(generic_associated_types): Currently only lifetimes are handled.
        self_trait_ref.substs.extend_to(tcx, item_def_id, mk_bound_param)
    } else {
        self_trait_ref.substs
    };

    let assoc_ty = tcx.mk_projection(tcx.hir().local_def_id(trait_item.hir_id), bound_substs);

    let bounds = AstConv::compute_bounds(
        &ItemCtxt::new(tcx, def_id),
        assoc_ty,
        bounds,
        SizedByDefault::Yes,
        trait_item.span,
    );

    let predicates = bounds.predicates(tcx, assoc_ty);

    if is_gat {
        // We use shifts to get the regions that we're substituting to
        // be bound by the binders in the `Predicate`s rather that
        // escaping.
        let shifted_in = ty::fold::shift_vars(tcx, &predicates, 1);
        let substituted = shifted_in.subst(tcx, bound_substs);
        ty::fold::shift_out_vars(tcx, &substituted, 1)
    } else {
        predicates
    }
}

/// Converts a specific `GenericBound` from the AST into a set of
/// predicates that apply to the self type. A vector is returned
/// because this can be anywhere from zero predicates (`T: ?Sized` adds no
/// predicates) to one (`T: Foo`) to many (`T: Bar<X = i32>` adds `T: Bar`
/// and `<T as Bar>::X == i32`).
fn predicates_from_bound<'tcx>(
    astconv: &dyn AstConv<'tcx>,
    param_ty: Ty<'tcx>,
    bound: &'tcx hir::GenericBound<'tcx>,
    constness: hir::Constness,
) -> Vec<(ty::Predicate<'tcx>, Span)> {
    match *bound {
        hir::GenericBound::Trait(ref tr, modifier) => {
            let constness = match modifier {
                hir::TraitBoundModifier::Maybe => return vec![],
                hir::TraitBoundModifier::MaybeConst => hir::Constness::NotConst,
                hir::TraitBoundModifier::None => constness,
            };

            let mut bounds = Bounds::default();
            let _ = astconv.instantiate_poly_trait_ref(tr, constness, param_ty, &mut bounds);
            bounds.predicates(astconv.tcx(), param_ty)
        }
        hir::GenericBound::Outlives(ref lifetime) => {
            let region = astconv.ast_region_to_region(lifetime, None);
            let pred = ty::Binder::bind(ty::OutlivesPredicate(param_ty, region));
            vec![(ty::Predicate::TypeOutlives(pred), lifetime.span)]
        }
    }
}

fn compute_sig_of_foreign_fn_decl<'tcx>(
    tcx: TyCtxt<'tcx>,
    def_id: DefId,
    decl: &'tcx hir::FnDecl<'tcx>,
    abi: abi::Abi,
) -> ty::PolyFnSig<'tcx> {
    let unsafety = if abi == abi::Abi::RustIntrinsic {
        intrinsic_operation_unsafety(&tcx.item_name(def_id).as_str())
    } else {
        hir::Unsafety::Unsafe
    };
    let fty = AstConv::ty_of_fn(
        &ItemCtxt::new(tcx, def_id),
        unsafety,
        abi,
        decl,
        &hir::Generics::empty(),
        None,
    );

    // Feature gate SIMD types in FFI, since I am not sure that the
    // ABIs are handled at all correctly. -huonw
    if abi != abi::Abi::RustIntrinsic
        && abi != abi::Abi::PlatformIntrinsic
        && !tcx.features().simd_ffi
    {
        let check = |ast_ty: &hir::Ty<'_>, ty: Ty<'_>| {
            if ty.is_simd() {
                tcx.sess
                    .struct_span_err(
                        ast_ty.span,
                        &format!(
                            "use of SIMD type `{}` in FFI is highly experimental and \
                             may result in invalid code",
                            tcx.hir().hir_to_pretty_string(ast_ty.hir_id)
                        ),
                    )
                    .help("add `#![feature(simd_ffi)]` to the crate attributes to enable")
                    .emit();
            }
        };
        for (input, ty) in decl.inputs.iter().zip(*fty.inputs().skip_binder()) {
            check(&input, ty)
        }
        if let hir::FnRetTy::Return(ref ty) = decl.output {
            check(&ty, *fty.output().skip_binder())
        }
    }

    fty
}

fn is_foreign_item(tcx: TyCtxt<'_>, def_id: DefId) -> bool {
    match tcx.hir().get_if_local(def_id) {
        Some(Node::ForeignItem(..)) => true,
        Some(_) => false,
        _ => bug!("is_foreign_item applied to non-local def-id {:?}", def_id),
    }
}

fn static_mutability(tcx: TyCtxt<'_>, def_id: DefId) -> Option<hir::Mutability> {
    match tcx.hir().get_if_local(def_id) {
        Some(Node::Item(&hir::Item { kind: hir::ItemKind::Static(_, mutbl, _), .. }))
        | Some(Node::ForeignItem(&hir::ForeignItem {
            kind: hir::ForeignItemKind::Static(_, mutbl),
            ..
        })) => Some(mutbl),
        Some(_) => None,
        _ => bug!("static_mutability applied to non-local def-id {:?}", def_id),
    }
}

fn generator_kind(tcx: TyCtxt<'_>, def_id: DefId) -> Option<hir::GeneratorKind> {
    match tcx.hir().get_if_local(def_id) {
        Some(Node::Expr(&rustc_hir::Expr {
            kind: rustc_hir::ExprKind::Closure(_, _, body_id, _, _),
            ..
        })) => tcx.hir().body(body_id).generator_kind(),
        Some(_) => None,
        _ => bug!("generator_kind applied to non-local def-id {:?}", def_id),
    }
}

fn from_target_feature(
    tcx: TyCtxt<'_>,
    id: DefId,
    attr: &ast::Attribute,
    whitelist: &FxHashMap<String, Option<Symbol>>,
    target_features: &mut Vec<Symbol>,
) {
    let list = match attr.meta_item_list() {
        Some(list) => list,
        None => return,
    };
    let bad_item = |span| {
        let msg = "malformed `target_feature` attribute input";
        let code = "enable = \"..\"".to_owned();
        tcx.sess
            .struct_span_err(span, &msg)
            .span_suggestion(span, "must be of the form", code, Applicability::HasPlaceholders)
            .emit();
    };
    let rust_features = tcx.features();
    for item in list {
        // Only `enable = ...` is accepted in the meta-item list.
        if !item.check_name(sym::enable) {
            bad_item(item.span());
            continue;
        }

        // Must be of the form `enable = "..."` (a string).
        let value = match item.value_str() {
            Some(value) => value,
            None => {
                bad_item(item.span());
                continue;
            }
        };

        // We allow comma separation to enable multiple features.
        target_features.extend(value.as_str().split(',').filter_map(|feature| {
            // Only allow whitelisted features per platform.
            let feature_gate = match whitelist.get(feature) {
                Some(g) => g,
                None => {
                    let msg =
                        format!("the feature named `{}` is not valid for this target", feature);
                    let mut err = tcx.sess.struct_span_err(item.span(), &msg);
                    err.span_label(
                        item.span(),
                        format!("`{}` is not valid for this target", feature),
                    );
                    if feature.starts_with('+') {
                        let valid = whitelist.contains_key(&feature[1..]);
                        if valid {
                            err.help("consider removing the leading `+` in the feature name");
                        }
                    }
                    err.emit();
                    return None;
                }
            };

            // Only allow features whose feature gates have been enabled.
            let allowed = match feature_gate.as_ref().copied() {
                Some(sym::arm_target_feature) => rust_features.arm_target_feature,
                Some(sym::aarch64_target_feature) => rust_features.aarch64_target_feature,
                Some(sym::hexagon_target_feature) => rust_features.hexagon_target_feature,
                Some(sym::powerpc_target_feature) => rust_features.powerpc_target_feature,
                Some(sym::mips_target_feature) => rust_features.mips_target_feature,
                Some(sym::avx512_target_feature) => rust_features.avx512_target_feature,
                Some(sym::mmx_target_feature) => rust_features.mmx_target_feature,
                Some(sym::sse4a_target_feature) => rust_features.sse4a_target_feature,
                Some(sym::tbm_target_feature) => rust_features.tbm_target_feature,
                Some(sym::wasm_target_feature) => rust_features.wasm_target_feature,
                Some(sym::cmpxchg16b_target_feature) => rust_features.cmpxchg16b_target_feature,
                Some(sym::adx_target_feature) => rust_features.adx_target_feature,
                Some(sym::movbe_target_feature) => rust_features.movbe_target_feature,
                Some(sym::rtm_target_feature) => rust_features.rtm_target_feature,
                Some(sym::f16c_target_feature) => rust_features.f16c_target_feature,
                Some(name) => bug!("unknown target feature gate {}", name),
                None => true,
            };
            if !allowed && id.is_local() {
                feature_err(
                    &tcx.sess.parse_sess,
                    feature_gate.unwrap(),
                    item.span(),
                    &format!("the target feature `{}` is currently unstable", feature),
                )
                .emit();
            }
            Some(Symbol::intern(feature))
        }));
    }
}

fn linkage_by_name(tcx: TyCtxt<'_>, def_id: DefId, name: &str) -> Linkage {
    use rustc::mir::mono::Linkage::*;

    // Use the names from src/llvm/docs/LangRef.rst here. Most types are only
    // applicable to variable declarations and may not really make sense for
    // Rust code in the first place but whitelist them anyway and trust that
    // the user knows what s/he's doing. Who knows, unanticipated use cases
    // may pop up in the future.
    //
    // ghost, dllimport, dllexport and linkonce_odr_autohide are not supported
    // and don't have to be, LLVM treats them as no-ops.
    match name {
        "appending" => Appending,
        "available_externally" => AvailableExternally,
        "common" => Common,
        "extern_weak" => ExternalWeak,
        "external" => External,
        "internal" => Internal,
        "linkonce" => LinkOnceAny,
        "linkonce_odr" => LinkOnceODR,
        "private" => Private,
        "weak" => WeakAny,
        "weak_odr" => WeakODR,
        _ => {
            let span = tcx.hir().span_if_local(def_id);
            if let Some(span) = span {
                tcx.sess.span_fatal(span, "invalid linkage specified")
            } else {
                tcx.sess.fatal(&format!("invalid linkage specified: {}", name))
            }
        }
    }
}

fn codegen_fn_attrs(tcx: TyCtxt<'_>, id: DefId) -> CodegenFnAttrs {
    let attrs = tcx.get_attrs(id);

    let mut codegen_fn_attrs = CodegenFnAttrs::new();
    if should_inherit_track_caller(tcx, id) {
        codegen_fn_attrs.flags |= CodegenFnAttrFlags::TRACK_CALLER;
    }

    let whitelist = tcx.target_features_whitelist(LOCAL_CRATE);

    let mut inline_span = None;
    let mut link_ordinal_span = None;
    let mut no_sanitize_span = None;
    for attr in attrs.iter() {
        if attr.check_name(sym::cold) {
            codegen_fn_attrs.flags |= CodegenFnAttrFlags::COLD;
        } else if attr.check_name(sym::rustc_allocator) {
            codegen_fn_attrs.flags |= CodegenFnAttrFlags::ALLOCATOR;
        } else if attr.check_name(sym::unwind) {
            codegen_fn_attrs.flags |= CodegenFnAttrFlags::UNWIND;
        } else if attr.check_name(sym::ffi_returns_twice) {
            if tcx.is_foreign_item(id) {
                codegen_fn_attrs.flags |= CodegenFnAttrFlags::FFI_RETURNS_TWICE;
            } else {
                // `#[ffi_returns_twice]` is only allowed `extern fn`s.
                struct_span_err!(
                    tcx.sess,
                    attr.span,
                    E0724,
                    "`#[ffi_returns_twice]` may only be used on foreign functions"
                )
                .emit();
            }
        } else if attr.check_name(sym::rustc_allocator_nounwind) {
            codegen_fn_attrs.flags |= CodegenFnAttrFlags::RUSTC_ALLOCATOR_NOUNWIND;
        } else if attr.check_name(sym::naked) {
            codegen_fn_attrs.flags |= CodegenFnAttrFlags::NAKED;
        } else if attr.check_name(sym::no_mangle) {
            codegen_fn_attrs.flags |= CodegenFnAttrFlags::NO_MANGLE;
        } else if attr.check_name(sym::rustc_std_internal_symbol) {
            codegen_fn_attrs.flags |= CodegenFnAttrFlags::RUSTC_STD_INTERNAL_SYMBOL;
        } else if attr.check_name(sym::used) {
            codegen_fn_attrs.flags |= CodegenFnAttrFlags::USED;
        } else if attr.check_name(sym::thread_local) {
            codegen_fn_attrs.flags |= CodegenFnAttrFlags::THREAD_LOCAL;
        } else if attr.check_name(sym::track_caller) {
            if tcx.is_closure(id) || tcx.fn_sig(id).abi() != abi::Abi::Rust {
                struct_span_err!(tcx.sess, attr.span, E0737, "`#[track_caller]` requires Rust ABI")
                    .emit();
            }
            codegen_fn_attrs.flags |= CodegenFnAttrFlags::TRACK_CALLER;
        } else if attr.check_name(sym::export_name) {
            if let Some(s) = attr.value_str() {
                if s.as_str().contains('\0') {
                    // `#[export_name = ...]` will be converted to a null-terminated string,
                    // so it may not contain any null characters.
                    struct_span_err!(
                        tcx.sess,
                        attr.span,
                        E0648,
                        "`export_name` may not contain null characters"
                    )
                    .emit();
                }
                codegen_fn_attrs.export_name = Some(s);
            }
        } else if attr.check_name(sym::target_feature) {
            if tcx.is_closure(id) || tcx.fn_sig(id).unsafety() == Unsafety::Normal {
                let msg = "`#[target_feature(..)]` can only be applied to `unsafe` functions";
                tcx.sess
                    .struct_span_err(attr.span, msg)
                    .span_label(attr.span, "can only be applied to `unsafe` functions")
                    .span_label(tcx.def_span(id), "not an `unsafe` function")
                    .emit();
            }
            from_target_feature(tcx, id, attr, &whitelist, &mut codegen_fn_attrs.target_features);
        } else if attr.check_name(sym::linkage) {
            if let Some(val) = attr.value_str() {
                codegen_fn_attrs.linkage = Some(linkage_by_name(tcx, id, &val.as_str()));
            }
        } else if attr.check_name(sym::link_section) {
            if let Some(val) = attr.value_str() {
                if val.as_str().bytes().any(|b| b == 0) {
                    let msg = format!(
                        "illegal null byte in link_section \
                         value: `{}`",
                        &val
                    );
                    tcx.sess.span_err(attr.span, &msg);
                } else {
                    codegen_fn_attrs.link_section = Some(val);
                }
            }
        } else if attr.check_name(sym::link_name) {
            codegen_fn_attrs.link_name = attr.value_str();
        } else if attr.check_name(sym::link_ordinal) {
            link_ordinal_span = Some(attr.span);
            if let ordinal @ Some(_) = check_link_ordinal(tcx, attr) {
                codegen_fn_attrs.link_ordinal = ordinal;
            }
        } else if attr.check_name(sym::no_sanitize) {
            no_sanitize_span = Some(attr.span);
            if let Some(list) = attr.meta_item_list() {
                for item in list.iter() {
                    if item.check_name(sym::address) {
                        codegen_fn_attrs.flags |= CodegenFnAttrFlags::NO_SANITIZE_ADDRESS;
                    } else if item.check_name(sym::memory) {
                        codegen_fn_attrs.flags |= CodegenFnAttrFlags::NO_SANITIZE_MEMORY;
                    } else if item.check_name(sym::thread) {
                        codegen_fn_attrs.flags |= CodegenFnAttrFlags::NO_SANITIZE_THREAD;
                    } else {
                        tcx.sess
                            .struct_span_err(item.span(), "invalid argument for `no_sanitize`")
                            .note("expected one of: `address`, `memory` or `thread`")
                            .emit();
                    }
                }
            }
        }
    }

    codegen_fn_attrs.inline = attrs.iter().fold(InlineAttr::None, |ia, attr| {
        if !attr.has_name(sym::inline) {
            return ia;
        }
        match attr.meta().map(|i| i.kind) {
            Some(MetaItemKind::Word) => {
                mark_used(attr);
                InlineAttr::Hint
            }
            Some(MetaItemKind::List(ref items)) => {
                mark_used(attr);
                inline_span = Some(attr.span);
                if items.len() != 1 {
                    struct_span_err!(
                        tcx.sess.diagnostic(),
                        attr.span,
                        E0534,
                        "expected one argument"
                    )
                    .emit();
                    InlineAttr::None
                } else if list_contains_name(&items[..], sym::always) {
                    InlineAttr::Always
                } else if list_contains_name(&items[..], sym::never) {
                    InlineAttr::Never
                } else {
                    struct_span_err!(
                        tcx.sess.diagnostic(),
                        items[0].span(),
                        E0535,
                        "invalid argument"
                    )
                    .emit();

                    InlineAttr::None
                }
            }
            Some(MetaItemKind::NameValue(_)) => ia,
            None => ia,
        }
    });

    codegen_fn_attrs.optimize = attrs.iter().fold(OptimizeAttr::None, |ia, attr| {
        if !attr.has_name(sym::optimize) {
            return ia;
        }
        let err = |sp, s| struct_span_err!(tcx.sess.diagnostic(), sp, E0722, "{}", s).emit();
        match attr.meta().map(|i| i.kind) {
            Some(MetaItemKind::Word) => {
                err(attr.span, "expected one argument");
                ia
            }
            Some(MetaItemKind::List(ref items)) => {
                mark_used(attr);
                inline_span = Some(attr.span);
                if items.len() != 1 {
                    err(attr.span, "expected one argument");
                    OptimizeAttr::None
                } else if list_contains_name(&items[..], sym::size) {
                    OptimizeAttr::Size
                } else if list_contains_name(&items[..], sym::speed) {
                    OptimizeAttr::Speed
                } else {
                    err(items[0].span(), "invalid argument");
                    OptimizeAttr::None
                }
            }
            Some(MetaItemKind::NameValue(_)) => ia,
            None => ia,
        }
    });

    // If a function uses #[target_feature] it can't be inlined into general
    // purpose functions as they wouldn't have the right target features
    // enabled. For that reason we also forbid #[inline(always)] as it can't be
    // respected.
    if !codegen_fn_attrs.target_features.is_empty() {
        if codegen_fn_attrs.inline == InlineAttr::Always {
            if let Some(span) = inline_span {
                tcx.sess.span_err(
                    span,
                    "cannot use `#[inline(always)]` with \
                     `#[target_feature]`",
                );
            }
        }
    }

    if codegen_fn_attrs.flags.intersects(CodegenFnAttrFlags::NO_SANITIZE_ANY) {
        if codegen_fn_attrs.inline == InlineAttr::Always {
            if let (Some(no_sanitize_span), Some(inline_span)) = (no_sanitize_span, inline_span) {
                let hir_id = tcx.hir().as_local_hir_id(id).unwrap();
                tcx.struct_span_lint_hir(
                    lint::builtin::INLINE_NO_SANITIZE,
                    hir_id,
                    no_sanitize_span,
                    |lint| {
                        lint.build("`no_sanitize` will have no effect after inlining")
                            .span_note(inline_span, "inlining requested here")
                            .emit();
                    },
                )
            }
        }
    }

    // Weak lang items have the same semantics as "std internal" symbols in the
    // sense that they're preserved through all our LTO passes and only
    // strippable by the linker.
    //
    // Additionally weak lang items have predetermined symbol names.
    if tcx.is_weak_lang_item(id) {
        codegen_fn_attrs.flags |= CodegenFnAttrFlags::RUSTC_STD_INTERNAL_SYMBOL;
    }
    if let Some(name) = lang_items::link_name(&attrs) {
        codegen_fn_attrs.export_name = Some(name);
        codegen_fn_attrs.link_name = Some(name);
    }
    check_link_name_xor_ordinal(tcx, &codegen_fn_attrs, link_ordinal_span);

    // Internal symbols to the standard library all have no_mangle semantics in
    // that they have defined symbol names present in the function name. This
    // also applies to weak symbols where they all have known symbol names.
    if codegen_fn_attrs.flags.contains(CodegenFnAttrFlags::RUSTC_STD_INTERNAL_SYMBOL) {
        codegen_fn_attrs.flags |= CodegenFnAttrFlags::NO_MANGLE;
    }

    codegen_fn_attrs
}

/// Checks if the provided DefId is a method in a trait impl for a trait which has track_caller
/// applied to the method prototype.
fn should_inherit_track_caller(tcx: TyCtxt<'_>, def_id: DefId) -> bool {
    if let Some(impl_item) = tcx.opt_associated_item(def_id) {
        if let ty::AssocItemContainer::ImplContainer(impl_def_id) = impl_item.container {
            if let Some(trait_def_id) = tcx.trait_id_of_impl(impl_def_id) {
                if let Some(trait_item) = tcx
                    .associated_items(trait_def_id)
                    .filter_by_name_unhygienic(impl_item.ident.name)
                    .find(move |trait_item| {
                        trait_item.kind == ty::AssocKind::Method
                            && tcx.hygienic_eq(impl_item.ident, trait_item.ident, trait_def_id)
                    })
                {
                    return tcx
                        .codegen_fn_attrs(trait_item.def_id)
                        .flags
                        .intersects(CodegenFnAttrFlags::TRACK_CALLER);
                }
            }
        }
    }

    false
}

fn check_link_ordinal(tcx: TyCtxt<'_>, attr: &ast::Attribute) -> Option<usize> {
    use rustc_ast::ast::{Lit, LitIntType, LitKind};
    let meta_item_list = attr.meta_item_list();
    let meta_item_list: Option<&[ast::NestedMetaItem]> = meta_item_list.as_ref().map(Vec::as_ref);
    let sole_meta_list = match meta_item_list {
        Some([item]) => item.literal(),
        _ => None,
    };
    if let Some(Lit { kind: LitKind::Int(ordinal, LitIntType::Unsuffixed), .. }) = sole_meta_list {
        if *ordinal <= std::usize::MAX as u128 {
            Some(*ordinal as usize)
        } else {
            let msg = format!("ordinal value in `link_ordinal` is too large: `{}`", &ordinal);
            tcx.sess
                .struct_span_err(attr.span, &msg)
                .note("the value may not exceed `std::usize::MAX`")
                .emit();
            None
        }
    } else {
        tcx.sess
            .struct_span_err(attr.span, "illegal ordinal format in `link_ordinal`")
            .note("an unsuffixed integer value, e.g., `1`, is expected")
            .emit();
        None
    }
}

fn check_link_name_xor_ordinal(
    tcx: TyCtxt<'_>,
    codegen_fn_attrs: &CodegenFnAttrs,
    inline_span: Option<Span>,
) {
    if codegen_fn_attrs.link_name.is_none() || codegen_fn_attrs.link_ordinal.is_none() {
        return;
    }
    let msg = "cannot use `#[link_name]` with `#[link_ordinal]`";
    if let Some(span) = inline_span {
        tcx.sess.span_err(span, msg);
    } else {
        tcx.sess.err(msg);
    }
}
