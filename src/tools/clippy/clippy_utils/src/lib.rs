#![feature(box_patterns)]
#![feature(in_band_lifetimes)]
#![feature(or_patterns)]
#![feature(rustc_private)]
#![recursion_limit = "512"]
#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc, clippy::must_use_candidate)]

// FIXME: switch to something more ergonomic here, once available.
// (Currently there is no way to opt into sysroot crates without `extern crate`.)
extern crate rustc_ast;
extern crate rustc_ast_pretty;
extern crate rustc_data_structures;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_infer;
extern crate rustc_lexer;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_mir;
extern crate rustc_session;
extern crate rustc_span;
extern crate rustc_target;
extern crate rustc_trait_selection;
extern crate rustc_typeck;

#[macro_use]
pub mod sym_helper;

#[allow(clippy::module_name_repetitions)]
pub mod ast_utils;
pub mod attrs;
pub mod camel_case;
pub mod comparisons;
pub mod consts;
mod diagnostics;
pub mod eager_or_lazy;
pub mod higher;
mod hir_utils;
pub mod numeric_literal;
pub mod paths;
pub mod ptr;
pub mod qualify_min_const_fn;
pub mod sugg;
pub mod usage;
pub mod visitors;

pub use self::attrs::*;
pub use self::diagnostics::*;
pub use self::hir_utils::{both, eq_expr_value, over, SpanlessEq, SpanlessHash};

use std::borrow::Cow;
use std::collections::hash_map::Entry;
use std::hash::BuildHasherDefault;

use if_chain::if_chain;
use rustc_ast::ast::{self, Attribute, BorrowKind, LitKind, Mutability};
use rustc_data_structures::fx::FxHashMap;
use rustc_errors::Applicability;
use rustc_hir as hir;
use rustc_hir::def::{CtorKind, CtorOf, DefKind, Res};
use rustc_hir::def_id::{DefId, LOCAL_CRATE};
use rustc_hir::intravisit::{self, NestedVisitorMap, Visitor};
use rustc_hir::Node;
use rustc_hir::{
    def, Arm, Block, Body, Constness, Expr, ExprKind, FnDecl, GenericArgs, HirId, Impl, ImplItem, ImplItemKind, Item,
    ItemKind, LangItem, MatchSource, Param, Pat, PatKind, Path, PathSegment, QPath, TraitItem, TraitItemKind, TraitRef,
    TyKind, Unsafety,
};
use rustc_infer::infer::TyCtxtInferExt;
use rustc_lint::{LateContext, Level, Lint, LintContext};
use rustc_middle::hir::exports::Export;
use rustc_middle::hir::map::Map;
use rustc_middle::ty::subst::{GenericArg, GenericArgKind};
use rustc_middle::ty::{self, layout::IntegerExt, DefIdTree, IntTy, Ty, TyCtxt, TypeFoldable, UintTy};
use rustc_semver::RustcVersion;
use rustc_session::Session;
use rustc_span::hygiene::{self, ExpnKind, MacroKind};
use rustc_span::source_map::original_sp;
use rustc_span::sym;
use rustc_span::symbol::{kw, Symbol};
use rustc_span::{BytePos, Pos, Span, SyntaxContext, DUMMY_SP};
use rustc_target::abi::Integer;
use rustc_trait_selection::traits::query::normalize::AtExt;
use smallvec::SmallVec;

use crate::consts::{constant, Constant};
use std::collections::HashMap;

pub fn parse_msrv(msrv: &str, sess: Option<&Session>, span: Option<Span>) -> Option<RustcVersion> {
    if let Ok(version) = RustcVersion::parse(msrv) {
        return Some(version);
    } else if let Some(sess) = sess {
        if let Some(span) = span {
            sess.span_err(span, &format!("`{}` is not a valid Rust version", msrv));
        }
    }
    None
}

pub fn meets_msrv(msrv: Option<&RustcVersion>, lint_msrv: &RustcVersion) -> bool {
    msrv.map_or(true, |msrv| msrv.meets(*lint_msrv))
}

#[macro_export]
macro_rules! extract_msrv_attr {
    (LateContext) => {
        extract_msrv_attr!(@LateContext, ());
    };
    (EarlyContext) => {
        extract_msrv_attr!(@EarlyContext);
    };
    (@$context:ident$(, $call:tt)?) => {
        fn enter_lint_attrs(&mut self, cx: &rustc_lint::$context<'tcx>, attrs: &'tcx [rustc_ast::ast::Attribute]) {
            use $crate::get_unique_inner_attr;
            match get_unique_inner_attr(cx.sess$($call)?, attrs, "msrv") {
                Some(msrv_attr) => {
                    if let Some(msrv) = msrv_attr.value_str() {
                        self.msrv = $crate::parse_msrv(
                            &msrv.to_string(),
                            Some(cx.sess$($call)?),
                            Some(msrv_attr.span),
                        );
                    } else {
                        cx.sess$($call)?.span_err(msrv_attr.span, "bad clippy attribute");
                    }
                },
                _ => (),
            }
        }
    };
}

/// Returns `true` if the two spans come from differing expansions (i.e., one is
/// from a macro and one isn't).
#[must_use]
pub fn differing_macro_contexts(lhs: Span, rhs: Span) -> bool {
    rhs.ctxt() != lhs.ctxt()
}

/// Returns `true` if the given `NodeId` is inside a constant context
///
/// # Example
///
/// ```rust,ignore
/// if in_constant(cx, expr.hir_id) {
///     // Do something
/// }
/// ```
pub fn in_constant(cx: &LateContext<'_>, id: HirId) -> bool {
    let parent_id = cx.tcx.hir().get_parent_item(id);
    match cx.tcx.hir().get(parent_id) {
        Node::Item(&Item {
            kind: ItemKind::Const(..) | ItemKind::Static(..),
            ..
        })
        | Node::TraitItem(&TraitItem {
            kind: TraitItemKind::Const(..),
            ..
        })
        | Node::ImplItem(&ImplItem {
            kind: ImplItemKind::Const(..),
            ..
        })
        | Node::AnonConst(_) => true,
        Node::Item(&Item {
            kind: ItemKind::Fn(ref sig, ..),
            ..
        })
        | Node::ImplItem(&ImplItem {
            kind: ImplItemKind::Fn(ref sig, _),
            ..
        }) => sig.header.constness == Constness::Const,
        _ => false,
    }
}

/// Returns `true` if this `span` was expanded by any macro.
#[must_use]
pub fn in_macro(span: Span) -> bool {
    if span.from_expansion() {
        !matches!(span.ctxt().outer_expn_data().kind, ExpnKind::Desugaring(..))
    } else {
        false
    }
}

// If the snippet is empty, it's an attribute that was inserted during macro
// expansion and we want to ignore those, because they could come from external
// sources that the user has no control over.
// For some reason these attributes don't have any expansion info on them, so
// we have to check it this way until there is a better way.
pub fn is_present_in_source<T: LintContext>(cx: &T, span: Span) -> bool {
    if let Some(snippet) = snippet_opt(cx, span) {
        if snippet.is_empty() {
            return false;
        }
    }
    true
}

/// Checks if given pattern is a wildcard (`_`)
pub fn is_wild<'tcx>(pat: &impl std::ops::Deref<Target = Pat<'tcx>>) -> bool {
    matches!(pat.kind, PatKind::Wild)
}

/// Checks if type is struct, enum or union type with the given def path.
///
/// If the type is a diagnostic item, use `is_type_diagnostic_item` instead.
/// If you change the signature, remember to update the internal lint `MatchTypeOnDiagItem`
pub fn match_type(cx: &LateContext<'_>, ty: Ty<'_>, path: &[&str]) -> bool {
    match ty.kind() {
        ty::Adt(adt, _) => match_def_path(cx, adt.did, path),
        _ => false,
    }
}

/// Checks if the type is equal to a diagnostic item
///
/// If you change the signature, remember to update the internal lint `MatchTypeOnDiagItem`
pub fn is_type_diagnostic_item(cx: &LateContext<'_>, ty: Ty<'_>, diag_item: Symbol) -> bool {
    match ty.kind() {
        ty::Adt(adt, _) => cx.tcx.is_diagnostic_item(diag_item, adt.did),
        _ => false,
    }
}

/// Checks if the type is equal to a lang item
pub fn is_type_lang_item(cx: &LateContext<'_>, ty: Ty<'_>, lang_item: hir::LangItem) -> bool {
    match ty.kind() {
        ty::Adt(adt, _) => cx.tcx.lang_items().require(lang_item).unwrap() == adt.did,
        _ => false,
    }
}

/// Checks if the first type parameter is a lang item.
pub fn is_ty_param_lang_item(cx: &LateContext<'_>, qpath: &QPath<'tcx>, item: LangItem) -> Option<&'tcx hir::Ty<'tcx>> {
    let ty = get_qpath_generic_tys(qpath).next()?;

    if let TyKind::Path(qpath) = &ty.kind {
        cx.qpath_res(qpath, ty.hir_id)
            .opt_def_id()
            .map_or(false, |id| {
                cx.tcx.lang_items().require(item).map_or(false, |lang_id| id == lang_id)
            })
            .then(|| ty)
    } else {
        None
    }
}

/// Checks if the first type parameter is a diagnostic item.
pub fn is_ty_param_diagnostic_item(
    cx: &LateContext<'_>,
    qpath: &QPath<'tcx>,
    item: Symbol,
) -> Option<&'tcx hir::Ty<'tcx>> {
    let ty = get_qpath_generic_tys(qpath).next()?;

    if let TyKind::Path(qpath) = &ty.kind {
        cx.qpath_res(qpath, ty.hir_id)
            .opt_def_id()
            .map_or(false, |id| cx.tcx.is_diagnostic_item(item, id))
            .then(|| ty)
    } else {
        None
    }
}

/// Return `true` if the passed `typ` is `isize` or `usize`.
pub fn is_isize_or_usize(typ: Ty<'_>) -> bool {
    matches!(typ.kind(), ty::Int(IntTy::Isize) | ty::Uint(UintTy::Usize))
}

/// Checks if the method call given in `expr` belongs to the given trait.
pub fn match_trait_method(cx: &LateContext<'_>, expr: &Expr<'_>, path: &[&str]) -> bool {
    let def_id = cx.typeck_results().type_dependent_def_id(expr.hir_id).unwrap();
    let trt_id = cx.tcx.trait_of_item(def_id);
    trt_id.map_or(false, |trt_id| match_def_path(cx, trt_id, path))
}

/// Checks if the method call given in `expr` belongs to a trait or other container with a given
/// diagnostic item
pub fn is_diagnostic_assoc_item(cx: &LateContext<'_>, def_id: DefId, diag_item: Symbol) -> bool {
    cx.tcx
        .opt_associated_item(def_id)
        .and_then(|associated_item| match associated_item.container {
            ty::TraitContainer(assoc_def_id) => Some(assoc_def_id),
            ty::ImplContainer(assoc_def_id) => match cx.tcx.type_of(assoc_def_id).kind() {
                ty::Adt(adt, _) => Some(adt.did),
                ty::Slice(_) => cx.tcx.get_diagnostic_item(sym::slice), // this isn't perfect but it works
                _ => None,
            },
        })
        .map_or(false, |assoc_def_id| cx.tcx.is_diagnostic_item(diag_item, assoc_def_id))
}

/// Checks if an expression references a variable of the given name.
pub fn match_var(expr: &Expr<'_>, var: Symbol) -> bool {
    if let ExprKind::Path(QPath::Resolved(None, ref path)) = expr.kind {
        if let [p] = path.segments {
            return p.ident.name == var;
        }
    }
    false
}

pub fn last_path_segment<'tcx>(path: &QPath<'tcx>) -> &'tcx PathSegment<'tcx> {
    match *path {
        QPath::Resolved(_, ref path) => path.segments.last().expect("A path must have at least one segment"),
        QPath::TypeRelative(_, ref seg) => seg,
        QPath::LangItem(..) => panic!("last_path_segment: lang item has no path segments"),
    }
}

pub fn get_qpath_generics(path: &QPath<'tcx>) -> Option<&'tcx GenericArgs<'tcx>> {
    match path {
        QPath::Resolved(_, p) => p.segments.last().and_then(|s| s.args),
        QPath::TypeRelative(_, s) => s.args,
        QPath::LangItem(..) => None,
    }
}

pub fn get_qpath_generic_tys(path: &QPath<'tcx>) -> impl Iterator<Item = &'tcx hir::Ty<'tcx>> {
    get_qpath_generics(path)
        .map_or([].as_ref(), |a| a.args)
        .iter()
        .filter_map(|a| {
            if let hir::GenericArg::Type(ty) = a {
                Some(ty)
            } else {
                None
            }
        })
}

pub fn single_segment_path<'tcx>(path: &QPath<'tcx>) -> Option<&'tcx PathSegment<'tcx>> {
    match *path {
        QPath::Resolved(_, ref path) => path.segments.get(0),
        QPath::TypeRelative(_, ref seg) => Some(seg),
        QPath::LangItem(..) => None,
    }
}

/// Matches a `QPath` against a slice of segment string literals.
///
/// There is also `match_path` if you are dealing with a `rustc_hir::Path` instead of a
/// `rustc_hir::QPath`.
///
/// # Examples
/// ```rust,ignore
/// match_qpath(path, &["std", "rt", "begin_unwind"])
/// ```
pub fn match_qpath(path: &QPath<'_>, segments: &[&str]) -> bool {
    match *path {
        QPath::Resolved(_, ref path) => match_path(path, segments),
        QPath::TypeRelative(ref ty, ref segment) => match ty.kind {
            TyKind::Path(ref inner_path) => {
                if let [prefix @ .., end] = segments {
                    if match_qpath(inner_path, prefix) {
                        return segment.ident.name.as_str() == *end;
                    }
                }
                false
            },
            _ => false,
        },
        QPath::LangItem(..) => false,
    }
}

/// Matches a `Path` against a slice of segment string literals.
///
/// There is also `match_qpath` if you are dealing with a `rustc_hir::QPath` instead of a
/// `rustc_hir::Path`.
///
/// # Examples
///
/// ```rust,ignore
/// if match_path(&trait_ref.path, &paths::HASH) {
///     // This is the `std::hash::Hash` trait.
/// }
///
/// if match_path(ty_path, &["rustc", "lint", "Lint"]) {
///     // This is a `rustc_middle::lint::Lint`.
/// }
/// ```
pub fn match_path(path: &Path<'_>, segments: &[&str]) -> bool {
    path.segments
        .iter()
        .rev()
        .zip(segments.iter().rev())
        .all(|(a, b)| a.ident.name.as_str() == *b)
}

/// Matches a `Path` against a slice of segment string literals, e.g.
///
/// # Examples
/// ```rust,ignore
/// match_path_ast(path, &["std", "rt", "begin_unwind"])
/// ```
pub fn match_path_ast(path: &ast::Path, segments: &[&str]) -> bool {
    path.segments
        .iter()
        .rev()
        .zip(segments.iter().rev())
        .all(|(a, b)| a.ident.name.as_str() == *b)
}

/// If the expression is a path to a local, returns the canonical `HirId` of the local.
pub fn path_to_local(expr: &Expr<'_>) -> Option<HirId> {
    if let ExprKind::Path(QPath::Resolved(None, ref path)) = expr.kind {
        if let Res::Local(id) = path.res {
            return Some(id);
        }
    }
    None
}

/// Returns true if the expression is a path to a local with the specified `HirId`.
/// Use this function to see if an expression matches a function argument or a match binding.
pub fn path_to_local_id(expr: &Expr<'_>, id: HirId) -> bool {
    path_to_local(expr) == Some(id)
}

/// Gets the definition associated to a path.
#[allow(clippy::shadow_unrelated)] // false positive #6563
pub fn path_to_res(cx: &LateContext<'_>, path: &[&str]) -> Res {
    macro_rules! try_res {
        ($e:expr) => {
            match $e {
                Some(e) => e,
                None => return Res::Err,
            }
        };
    }
    fn item_child_by_name<'tcx>(tcx: TyCtxt<'tcx>, def_id: DefId, name: &str) -> Option<&'tcx Export<HirId>> {
        tcx.item_children(def_id)
            .iter()
            .find(|item| item.ident.name.as_str() == name)
    }

    let (krate, first, path) = match *path {
        [krate, first, ref path @ ..] => (krate, first, path),
        _ => return Res::Err,
    };
    let tcx = cx.tcx;
    let crates = tcx.crates();
    let krate = try_res!(crates.iter().find(|&&num| tcx.crate_name(num).as_str() == krate));
    let first = try_res!(item_child_by_name(tcx, krate.as_def_id(), first));
    let last = path
        .iter()
        .copied()
        // `get_def_path` seems to generate these empty segments for extern blocks.
        // We can just ignore them.
        .filter(|segment| !segment.is_empty())
        // for each segment, find the child item
        .try_fold(first, |item, segment| {
            let def_id = item.res.def_id();
            if let Some(item) = item_child_by_name(tcx, def_id, segment) {
                Some(item)
            } else if matches!(item.res, Res::Def(DefKind::Enum | DefKind::Struct, _)) {
                // it is not a child item so check inherent impl items
                tcx.inherent_impls(def_id)
                    .iter()
                    .find_map(|&impl_def_id| item_child_by_name(tcx, impl_def_id, segment))
            } else {
                None
            }
        });
    try_res!(last).res
}

/// Convenience function to get the `DefId` of a trait by path.
/// It could be a trait or trait alias.
pub fn get_trait_def_id(cx: &LateContext<'_>, path: &[&str]) -> Option<DefId> {
    match path_to_res(cx, path) {
        Res::Def(DefKind::Trait | DefKind::TraitAlias, trait_id) => Some(trait_id),
        _ => None,
    }
}

/// Checks whether a type implements a trait.
/// See also `get_trait_def_id`.
pub fn implements_trait<'tcx>(
    cx: &LateContext<'tcx>,
    ty: Ty<'tcx>,
    trait_id: DefId,
    ty_params: &[GenericArg<'tcx>],
) -> bool {
    // Do not check on infer_types to avoid panic in evaluate_obligation.
    if ty.has_infer_types() {
        return false;
    }
    let ty = cx.tcx.erase_regions(ty);
    if ty.has_escaping_bound_vars() {
        return false;
    }
    let ty_params = cx.tcx.mk_substs(ty_params.iter());
    cx.tcx.type_implements_trait((trait_id, ty, ty_params, cx.param_env))
}

/// Gets the `hir::TraitRef` of the trait the given method is implemented for.
///
/// Use this if you want to find the `TraitRef` of the `Add` trait in this example:
///
/// ```rust
/// struct Point(isize, isize);
///
/// impl std::ops::Add for Point {
///     type Output = Self;
///
///     fn add(self, other: Self) -> Self {
///         Point(0, 0)
///     }
/// }
/// ```
pub fn trait_ref_of_method<'tcx>(cx: &LateContext<'tcx>, hir_id: HirId) -> Option<&'tcx TraitRef<'tcx>> {
    // Get the implemented trait for the current function
    let parent_impl = cx.tcx.hir().get_parent_item(hir_id);
    if_chain! {
        if parent_impl != hir::CRATE_HIR_ID;
        if let hir::Node::Item(item) = cx.tcx.hir().get(parent_impl);
        if let hir::ItemKind::Impl(impl_) = &item.kind;
        then { return impl_.of_trait.as_ref(); }
    }
    None
}

/// Checks whether this type implements `Drop`.
pub fn has_drop<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> bool {
    match ty.ty_adt_def() {
        Some(def) => def.has_dtor(cx.tcx),
        None => false,
    }
}

/// Checks whether a type can be partially moved.
pub fn can_partially_move_ty(cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> bool {
    if has_drop(cx, ty) || is_copy(cx, ty) {
        return false;
    }
    match ty.kind() {
        ty::Param(_) => false,
        ty::Adt(def, subs) => def.all_fields().any(|f| !is_copy(cx, f.ty(cx.tcx, subs))),
        _ => true,
    }
}

/// Returns the method names and argument list of nested method call expressions that make up
/// `expr`. method/span lists are sorted with the most recent call first.
pub fn method_calls<'tcx>(
    cx: &LateContext<'_>,
    expr: &'tcx Expr<'tcx>,
    max_depth: usize,
) -> (Vec<Symbol>, Vec<&'tcx [Expr<'tcx>]>, Vec<Span>) {
    let mut method_names = Vec::with_capacity(max_depth);
    let mut arg_lists = Vec::with_capacity(max_depth);
    let mut spans = Vec::with_capacity(max_depth);

    let mut current = expr;
    for _ in 0..max_depth {
        if let ExprKind::MethodCall(path, span, args, _) = &current.kind {
            if args.iter().any(|e| cx.tcx.hir().span(e.hir_id).from_expansion()) {
                break;
            }
            method_names.push(path.ident.name);
            arg_lists.push(&**args);
            spans.push(*span);
            current = &args[0];
        } else {
            break;
        }
    }

    (method_names, arg_lists, spans)
}

/// Matches an `Expr` against a chain of methods, and return the matched `Expr`s.
///
/// For example, if `expr` represents the `.baz()` in `foo.bar().baz()`,
/// `method_chain_args(expr, &["bar", "baz"])` will return a `Vec`
/// containing the `Expr`s for
/// `.bar()` and `.baz()`
pub fn method_chain_args<'a>(
    cx: &LateContext<'_>,
    expr: &'a Expr<'_>,
    methods: &[&str],
) -> Option<Vec<&'a [Expr<'a>]>> {
    let mut current = expr;
    let mut matched = Vec::with_capacity(methods.len());
    for method_name in methods.iter().rev() {
        // method chains are stored last -> first
        if let ExprKind::MethodCall(ref path, _, ref args, _) = current.kind {
            if path.ident.name.as_str() == *method_name {
                if args.iter().any(|e| cx.tcx.hir().span(e.hir_id).from_expansion()) {
                    return None;
                }
                matched.push(&**args); // build up `matched` backwards
                current = &args[0] // go to parent expression
            } else {
                return None;
            }
        } else {
            return None;
        }
    }
    // Reverse `matched` so that it is in the same order as `methods`.
    matched.reverse();
    Some(matched)
}

/// Returns `true` if the provided `def_id` is an entrypoint to a program.
pub fn is_entrypoint_fn(cx: &LateContext<'_>, def_id: DefId) -> bool {
    cx.tcx
        .entry_fn(LOCAL_CRATE)
        .map_or(false, |(entry_fn_def_id, _)| def_id == entry_fn_def_id.to_def_id())
}

/// Returns `true` if the expression is in the program's `#[panic_handler]`.
pub fn is_in_panic_handler(cx: &LateContext<'_>, e: &Expr<'_>) -> bool {
    let parent = cx.tcx.hir().get_parent_item(e.hir_id);
    let def_id = cx.tcx.hir().local_def_id(parent).to_def_id();
    Some(def_id) == cx.tcx.lang_items().panic_impl()
}

/// Gets the name of the item the expression is in, if available.
pub fn get_item_name(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<Symbol> {
    let parent_id = cx.tcx.hir().get_parent_item(expr.hir_id);
    match cx.tcx.hir().find(parent_id) {
        Some(
            Node::Item(Item { ident, .. })
            | Node::TraitItem(TraitItem { ident, .. })
            | Node::ImplItem(ImplItem { ident, .. }),
        ) => Some(ident.name),
        _ => None,
    }
}

/// Gets the name of a `Pat`, if any.
pub fn get_pat_name(pat: &Pat<'_>) -> Option<Symbol> {
    match pat.kind {
        PatKind::Binding(.., ref spname, _) => Some(spname.name),
        PatKind::Path(ref qpath) => single_segment_path(qpath).map(|ps| ps.ident.name),
        PatKind::Box(ref p) | PatKind::Ref(ref p, _) => get_pat_name(&*p),
        _ => None,
    }
}

struct ContainsName {
    name: Symbol,
    result: bool,
}

impl<'tcx> Visitor<'tcx> for ContainsName {
    type Map = Map<'tcx>;

    fn visit_name(&mut self, name: Symbol) {
        if self.name == name {
            self.result = true;
        }
    }
    fn nested_visit_map(&mut self) -> NestedVisitorMap<Self::Map> {
        NestedVisitorMap::None
    }
}

/// Checks if an `Expr` contains a certain name.
pub fn contains_name(name: Symbol, expr: &Expr<'_>) -> bool {
    let mut cn = ContainsName { name, result: false };
    cn.visit_expr(expr);
    cn.result
}

/// Returns `true` if `expr` contains a return expression
pub fn contains_return(expr: &hir::Expr<'_>) -> bool {
    struct RetCallFinder {
        found: bool,
    }

    impl<'tcx> hir::intravisit::Visitor<'tcx> for RetCallFinder {
        type Map = Map<'tcx>;

        fn visit_expr(&mut self, expr: &'tcx hir::Expr<'_>) {
            if self.found {
                return;
            }
            if let hir::ExprKind::Ret(..) = &expr.kind {
                self.found = true;
            } else {
                hir::intravisit::walk_expr(self, expr);
            }
        }

        fn nested_visit_map(&mut self) -> hir::intravisit::NestedVisitorMap<Self::Map> {
            hir::intravisit::NestedVisitorMap::None
        }
    }

    let mut visitor = RetCallFinder { found: false };
    visitor.visit_expr(expr);
    visitor.found
}

struct FindMacroCalls<'tcx, 'a, 'b> {
    tcx: TyCtxt<'tcx>,
    names: &'a [&'b str],
    result: Vec<Span>,
}

impl<'a, 'b, 'tcx, 'hir> Visitor<'hir> for FindMacroCalls<'tcx, 'a, 'b> {
    type Map = Map<'hir>;

    fn visit_expr(&mut self, expr: &'hir Expr<'_>) {
        let expr_span = self.tcx.hir().span(expr.hir_id);
        if self.names.iter().any(|fun| is_expn_of(expr_span, fun).is_some()) {
            self.result.push(expr_span);
        }
        // and check sub-expressions
        intravisit::walk_expr(self, expr);
    }

    fn nested_visit_map(&mut self) -> NestedVisitorMap<Self::Map> {
        NestedVisitorMap::None
    }
}

/// Finds calls of the specified macros in a function body.
pub fn find_macro_calls(tcx: TyCtxt<'_>, names: &[&str], body: &Body<'_>) -> Vec<Span> {
    let mut fmc = FindMacroCalls {
        tcx,
        names,
        result: Vec::new(),
    };
    fmc.visit_expr(&body.value);
    fmc.result
}

/// Converts a span to a code snippet if available, otherwise use default.
///
/// This is useful if you want to provide suggestions for your lint or more generally, if you want
/// to convert a given `Span` to a `str`.
///
/// # Example
/// ```rust,ignore
/// snippet(cx, expr.span, "..")
/// ```
pub fn snippet<'a, T: LintContext>(cx: &T, span: Span, default: &'a str) -> Cow<'a, str> {
    snippet_opt(cx, span).map_or_else(|| Cow::Borrowed(default), From::from)
}

/// Same as `snippet`, but it adapts the applicability level by following rules:
///
/// - Applicability level `Unspecified` will never be changed.
/// - If the span is inside a macro, change the applicability level to `MaybeIncorrect`.
/// - If the default value is used and the applicability level is `MachineApplicable`, change it to
/// `HasPlaceholders`
pub fn snippet_with_applicability<'a, T: LintContext>(
    cx: &T,
    span: Span,
    default: &'a str,
    applicability: &mut Applicability,
) -> Cow<'a, str> {
    if *applicability != Applicability::Unspecified && span.from_expansion() {
        *applicability = Applicability::MaybeIncorrect;
    }
    snippet_opt(cx, span).map_or_else(
        || {
            if *applicability == Applicability::MachineApplicable {
                *applicability = Applicability::HasPlaceholders;
            }
            Cow::Borrowed(default)
        },
        From::from,
    )
}

/// Same as `snippet`, but should only be used when it's clear that the input span is
/// not a macro argument.
pub fn snippet_with_macro_callsite<'a, T: LintContext>(cx: &T, span: Span, default: &'a str) -> Cow<'a, str> {
    snippet(cx, span.source_callsite(), default)
}

/// Converts a span to a code snippet. Returns `None` if not available.
pub fn snippet_opt<T: LintContext>(cx: &T, span: Span) -> Option<String> {
    cx.sess().source_map().span_to_snippet(span).ok()
}

/// Converts a span (from a block) to a code snippet if available, otherwise use default.
///
/// This trims the code of indentation, except for the first line. Use it for blocks or block-like
/// things which need to be printed as such.
///
/// The `indent_relative_to` arg can be used, to provide a span, where the indentation of the
/// resulting snippet of the given span.
///
/// # Example
///
/// ```rust,ignore
/// snippet_block(cx, block.span, "..", None)
/// // where, `block` is the block of the if expr
///     if x {
///         y;
///     }
/// // will return the snippet
/// {
///     y;
/// }
/// ```
///
/// ```rust,ignore
/// snippet_block(cx, block.span, "..", Some(if_expr.span))
/// // where, `block` is the block of the if expr
///     if x {
///         y;
///     }
/// // will return the snippet
/// {
///         y;
///     } // aligned with `if`
/// ```
/// Note that the first line of the snippet always has 0 indentation.
pub fn snippet_block<'a, T: LintContext>(
    cx: &T,
    span: Span,
    default: &'a str,
    indent_relative_to: Option<Span>,
) -> Cow<'a, str> {
    let snip = snippet(cx, span, default);
    let indent = indent_relative_to.and_then(|s| indent_of(cx, s));
    reindent_multiline(snip, true, indent)
}

/// Same as `snippet_block`, but adapts the applicability level by the rules of
/// `snippet_with_applicability`.
pub fn snippet_block_with_applicability<'a, T: LintContext>(
    cx: &T,
    span: Span,
    default: &'a str,
    indent_relative_to: Option<Span>,
    applicability: &mut Applicability,
) -> Cow<'a, str> {
    let snip = snippet_with_applicability(cx, span, default, applicability);
    let indent = indent_relative_to.and_then(|s| indent_of(cx, s));
    reindent_multiline(snip, true, indent)
}

/// Same as `snippet_with_applicability`, but first walks the span up to the given context. This
/// will result in the macro call, rather then the expansion, if the span is from a child context.
/// If the span is not from a child context, it will be used directly instead.
///
/// e.g. Given the expression `&vec![]`, getting a snippet from the span for `vec![]` as a HIR node
/// would result in `box []`. If given the context of the address of expression, this function will
/// correctly get a snippet of `vec![]`.
pub fn snippet_with_context(
    cx: &LateContext<'_>,
    span: Span,
    outer: SyntaxContext,
    default: &'a str,
    applicability: &mut Applicability,
) -> Cow<'a, str> {
    let outer_span = hygiene::walk_chain(span, outer);
    let span = if outer_span.ctxt() == outer {
        outer_span
    } else {
        // The span is from a macro argument, and the outer context is the macro using the argument
        if *applicability != Applicability::Unspecified {
            *applicability = Applicability::MaybeIncorrect;
        }
        // TODO: get the argument span.
        span
    };

    snippet_with_applicability(cx, span, default, applicability)
}

/// Returns a new Span that extends the original Span to the first non-whitespace char of the first
/// line.
///
/// ```rust,ignore
///     let x = ();
/// //          ^^
/// // will be converted to
///     let x = ();
/// //  ^^^^^^^^^^
/// ```
pub fn first_line_of_span<T: LintContext>(cx: &T, span: Span) -> Span {
    first_char_in_first_line(cx, span).map_or(span, |first_char_pos| span.with_lo(first_char_pos))
}

fn first_char_in_first_line<T: LintContext>(cx: &T, span: Span) -> Option<BytePos> {
    let line_span = line_span(cx, span);
    snippet_opt(cx, line_span).and_then(|snip| {
        snip.find(|c: char| !c.is_whitespace())
            .map(|pos| line_span.lo() + BytePos::from_usize(pos))
    })
}

/// Returns the indentation of the line of a span
///
/// ```rust,ignore
/// let x = ();
/// //      ^^ -- will return 0
///     let x = ();
/// //          ^^ -- will return 4
/// ```
pub fn indent_of<T: LintContext>(cx: &T, span: Span) -> Option<usize> {
    snippet_opt(cx, line_span(cx, span)).and_then(|snip| snip.find(|c: char| !c.is_whitespace()))
}

/// Returns the positon just before rarrow
///
/// ```rust,ignore
/// fn into(self) -> () {}
///              ^
/// // in case of unformatted code
/// fn into2(self)-> () {}
///               ^
/// fn into3(self)   -> () {}
///               ^
/// ```
pub fn position_before_rarrow(s: &str) -> Option<usize> {
    s.rfind("->").map(|rpos| {
        let mut rpos = rpos;
        let chars: Vec<char> = s.chars().collect();
        while rpos > 1 {
            if let Some(c) = chars.get(rpos - 1) {
                if c.is_whitespace() {
                    rpos -= 1;
                    continue;
                }
            }
            break;
        }
        rpos
    })
}

/// Extends the span to the beginning of the spans line, incl. whitespaces.
///
/// ```rust,ignore
///        let x = ();
/// //             ^^
/// // will be converted to
///        let x = ();
/// // ^^^^^^^^^^^^^^
/// ```
fn line_span<T: LintContext>(cx: &T, span: Span) -> Span {
    let span = original_sp(span, DUMMY_SP);
    let source_map_and_line = cx.sess().source_map().lookup_line(span.lo()).unwrap();
    let line_no = source_map_and_line.line;
    let line_start = source_map_and_line.sf.lines[line_no];
    Span::new(line_start, span.hi(), span.ctxt())
}

/// Like `snippet_block`, but add braces if the expr is not an `ExprKind::Block`.
/// Also takes an `Option<String>` which can be put inside the braces.
pub fn expr_block<'a>(
    cx: &LateContext<'_>,
    expr: &Expr<'_>,
    option: Option<String>,
    default: &'a str,
    indent_relative_to: Option<Span>,
) -> Cow<'a, str> {
    let expr_span = cx.tcx.hir().span(expr.hir_id);
    let code = snippet_block(cx, expr_span, default, indent_relative_to);
    let string = option.unwrap_or_default();
    if expr_span.from_expansion() {
        Cow::Owned(format!("{{ {} }}", snippet_with_macro_callsite(cx, expr_span, default)))
    } else if let ExprKind::Block(_, _) = expr.kind {
        Cow::Owned(format!("{}{}", code, string))
    } else if string.is_empty() {
        Cow::Owned(format!("{{ {} }}", code))
    } else {
        Cow::Owned(format!("{{\n{};\n{}\n}}", code, string))
    }
}

/// Reindent a multiline string with possibility of ignoring the first line.
#[allow(clippy::needless_pass_by_value)]
pub fn reindent_multiline(s: Cow<'_, str>, ignore_first: bool, indent: Option<usize>) -> Cow<'_, str> {
    let s_space = reindent_multiline_inner(&s, ignore_first, indent, ' ');
    let s_tab = reindent_multiline_inner(&s_space, ignore_first, indent, '\t');
    reindent_multiline_inner(&s_tab, ignore_first, indent, ' ').into()
}

fn reindent_multiline_inner(s: &str, ignore_first: bool, indent: Option<usize>, ch: char) -> String {
    let x = s
        .lines()
        .skip(ignore_first as usize)
        .filter_map(|l| {
            if l.is_empty() {
                None
            } else {
                // ignore empty lines
                Some(l.char_indices().find(|&(_, x)| x != ch).unwrap_or((l.len(), ch)).0)
            }
        })
        .min()
        .unwrap_or(0);
    let indent = indent.unwrap_or(0);
    s.lines()
        .enumerate()
        .map(|(i, l)| {
            if (ignore_first && i == 0) || l.is_empty() {
                l.to_owned()
            } else if x > indent {
                l.split_at(x - indent).1.to_owned()
            } else {
                " ".repeat(indent - x) + l
            }
        })
        .collect::<Vec<String>>()
        .join("\n")
}

/// Gets the parent expression, if any –- this is useful to constrain a lint.
pub fn get_parent_expr<'tcx>(cx: &LateContext<'tcx>, e: &Expr<'_>) -> Option<&'tcx Expr<'tcx>> {
    let map = &cx.tcx.hir();
    let hir_id = e.hir_id;
    let parent_id = map.get_parent_node(hir_id);
    if hir_id == parent_id {
        return None;
    }
    map.find(parent_id).and_then(|node| {
        if let Node::Expr(parent) = node {
            Some(parent)
        } else {
            None
        }
    })
}

pub fn get_enclosing_block<'tcx>(cx: &LateContext<'tcx>, hir_id: HirId) -> Option<&'tcx Block<'tcx>> {
    let map = &cx.tcx.hir();
    let enclosing_node = map
        .get_enclosing_scope(hir_id)
        .and_then(|enclosing_id| map.find(enclosing_id));
    enclosing_node.and_then(|node| match node {
        Node::Block(block) => Some(block),
        Node::Item(&Item {
            kind: ItemKind::Fn(_, _, eid),
            ..
        })
        | Node::ImplItem(&ImplItem {
            kind: ImplItemKind::Fn(_, eid),
            ..
        }) => match cx.tcx.hir().body(eid).value.kind {
            ExprKind::Block(ref block, _) => Some(block),
            _ => None,
        },
        _ => None,
    })
}

/// Gets the parent node if it's an impl block.
pub fn get_parent_as_impl(tcx: TyCtxt<'_>, id: HirId) -> Option<&Impl<'_>> {
    let map = tcx.hir();
    match map.parent_iter(id).next() {
        Some((
            _,
            Node::Item(Item {
                kind: ItemKind::Impl(imp),
                ..
            }),
        )) => Some(imp),
        _ => None,
    }
}

/// Returns the base type for HIR references and pointers.
pub fn walk_ptrs_hir_ty<'tcx>(ty: &'tcx hir::Ty<'tcx>) -> &'tcx hir::Ty<'tcx> {
    match ty.kind {
        TyKind::Ptr(ref mut_ty) | TyKind::Rptr(_, ref mut_ty) => walk_ptrs_hir_ty(&mut_ty.ty),
        _ => ty,
    }
}

/// Returns the base type for references and raw pointers, and count reference
/// depth.
pub fn walk_ptrs_ty_depth(ty: Ty<'_>) -> (Ty<'_>, usize) {
    fn inner(ty: Ty<'_>, depth: usize) -> (Ty<'_>, usize) {
        match ty.kind() {
            ty::Ref(_, ty, _) => inner(ty, depth + 1),
            _ => (ty, depth),
        }
    }
    inner(ty, 0)
}

/// Checks whether the given expression is a constant integer of the given value.
/// unlike `is_integer_literal`, this version does const folding
pub fn is_integer_const(cx: &LateContext<'_>, e: &Expr<'_>, value: u128) -> bool {
    if is_integer_literal(e, value) {
        return true;
    }
    let map = cx.tcx.hir();
    let parent_item = map.get_parent_item(e.hir_id);
    if let Some((Constant::Int(v), _)) = map
        .maybe_body_owned_by(parent_item)
        .and_then(|body_id| constant(cx, cx.tcx.typeck_body(body_id), e))
    {
        value == v
    } else {
        false
    }
}

/// Checks whether the given expression is a constant literal of the given value.
pub fn is_integer_literal(expr: &Expr<'_>, value: u128) -> bool {
    // FIXME: use constant folding
    if let ExprKind::Lit(ref spanned) = expr.kind {
        if let LitKind::Int(v, _) = spanned.node {
            return v == value;
        }
    }
    false
}

/// Returns `true` if the given `Expr` has been coerced before.
///
/// Examples of coercions can be found in the Nomicon at
/// <https://doc.rust-lang.org/nomicon/coercions.html>.
///
/// See `rustc_middle::ty::adjustment::Adjustment` and `rustc_typeck::check::coercion` for more
/// information on adjustments and coercions.
pub fn is_adjusted(cx: &LateContext<'_>, e: &Expr<'_>) -> bool {
    cx.typeck_results().adjustments().get(e.hir_id).is_some()
}

/// Returns the pre-expansion span if is this comes from an expansion of the
/// macro `name`.
/// See also `is_direct_expn_of`.
#[must_use]
pub fn is_expn_of(mut span: Span, name: &str) -> Option<Span> {
    loop {
        if span.from_expansion() {
            let data = span.ctxt().outer_expn_data();
            let new_span = data.call_site;

            if let ExpnKind::Macro(MacroKind::Bang, mac_name) = data.kind {
                if mac_name.as_str() == name {
                    return Some(new_span);
                }
            }

            span = new_span;
        } else {
            return None;
        }
    }
}

/// Returns the pre-expansion span if the span directly comes from an expansion
/// of the macro `name`.
/// The difference with `is_expn_of` is that in
/// ```rust,ignore
/// foo!(bar!(42));
/// ```
/// `42` is considered expanded from `foo!` and `bar!` by `is_expn_of` but only
/// `bar!` by
/// `is_direct_expn_of`.
#[must_use]
pub fn is_direct_expn_of(span: Span, name: &str) -> Option<Span> {
    if span.from_expansion() {
        let data = span.ctxt().outer_expn_data();
        let new_span = data.call_site;

        if let ExpnKind::Macro(MacroKind::Bang, mac_name) = data.kind {
            if mac_name.as_str() == name {
                return Some(new_span);
            }
        }
    }

    None
}

/// Convenience function to get the return type of a function.
pub fn return_ty<'tcx>(cx: &LateContext<'tcx>, fn_item: hir::HirId) -> Ty<'tcx> {
    let fn_def_id = cx.tcx.hir().local_def_id(fn_item);
    let ret_ty = cx.tcx.fn_sig(fn_def_id).output();
    cx.tcx.erase_late_bound_regions(ret_ty)
}

/// Walks into `ty` and returns `true` if any inner type is the same as `other_ty`
pub fn contains_ty(ty: Ty<'_>, other_ty: Ty<'_>) -> bool {
    ty.walk().any(|inner| match inner.unpack() {
        GenericArgKind::Type(inner_ty) => ty::TyS::same_type(other_ty, inner_ty),
        GenericArgKind::Lifetime(_) | GenericArgKind::Const(_) => false,
    })
}

/// Returns `true` if the given type is an `unsafe` function.
pub fn type_is_unsafe_function<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> bool {
    match ty.kind() {
        ty::FnDef(..) | ty::FnPtr(_) => ty.fn_sig(cx.tcx).unsafety() == Unsafety::Unsafe,
        _ => false,
    }
}

pub fn is_copy<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> bool {
    ty.is_copy_modulo_regions(cx.tcx.at(DUMMY_SP), cx.param_env)
}

/// Checks if an expression is constructing a tuple-like enum variant or struct
pub fn is_ctor_or_promotable_const_function(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
    if let ExprKind::Call(ref fun, _) = expr.kind {
        if let ExprKind::Path(ref qp) = fun.kind {
            let res = cx.qpath_res(qp, fun.hir_id);
            return match res {
                def::Res::Def(DefKind::Variant | DefKind::Ctor(..), ..) => true,
                def::Res::Def(_, def_id) => cx.tcx.is_promotable_const_fn(def_id),
                _ => false,
            };
        }
    }
    false
}

/// Returns `true` if a pattern is refutable.
// TODO: should be implemented using rustc/mir_build/thir machinery
pub fn is_refutable(cx: &LateContext<'_>, pat: &Pat<'_>) -> bool {
    fn is_enum_variant(cx: &LateContext<'_>, qpath: &QPath<'_>, id: HirId) -> bool {
        matches!(
            cx.qpath_res(qpath, id),
            def::Res::Def(DefKind::Variant, ..) | Res::Def(DefKind::Ctor(def::CtorOf::Variant, _), _)
        )
    }

    fn are_refutable<'a, I: Iterator<Item = &'a Pat<'a>>>(cx: &LateContext<'_>, mut i: I) -> bool {
        i.any(|pat| is_refutable(cx, pat))
    }

    match pat.kind {
        PatKind::Wild => false,
        PatKind::Binding(_, _, _, pat) => pat.map_or(false, |pat| is_refutable(cx, pat)),
        PatKind::Box(ref pat) | PatKind::Ref(ref pat, _) => is_refutable(cx, pat),
        PatKind::Lit(..) | PatKind::Range(..) => true,
        PatKind::Path(ref qpath) => is_enum_variant(cx, qpath, pat.hir_id),
        PatKind::Or(ref pats) => {
            // TODO: should be the honest check, that pats is exhaustive set
            are_refutable(cx, pats.iter().map(|pat| &**pat))
        },
        PatKind::Tuple(ref pats, _) => are_refutable(cx, pats.iter().map(|pat| &**pat)),
        PatKind::Struct(ref qpath, ref fields, _) => {
            is_enum_variant(cx, qpath, pat.hir_id) || are_refutable(cx, fields.iter().map(|field| &*field.pat))
        },
        PatKind::TupleStruct(ref qpath, ref pats, _) => {
            is_enum_variant(cx, qpath, pat.hir_id) || are_refutable(cx, pats.iter().map(|pat| &**pat))
        },
        PatKind::Slice(ref head, ref middle, ref tail) => {
            match &cx.typeck_results().node_type(pat.hir_id).kind() {
                ty::Slice(..) => {
                    // [..] is the only irrefutable slice pattern.
                    !head.is_empty() || middle.is_none() || !tail.is_empty()
                },
                ty::Array(..) => are_refutable(cx, head.iter().chain(middle).chain(tail.iter()).map(|pat| &**pat)),
                _ => {
                    // unreachable!()
                    true
                },
            }
        },
    }
}

/// Checks for the `#[automatically_derived]` attribute all `#[derive]`d
/// implementations have.
pub fn is_automatically_derived(attrs: &[ast::Attribute]) -> bool {
    attrs.iter().any(|attr| attr.has_name(sym::automatically_derived))
}

/// Remove blocks around an expression.
///
/// Ie. `x`, `{ x }` and `{{{{ x }}}}` all give `x`. `{ x; y }` and `{}` return
/// themselves.
pub fn remove_blocks<'tcx>(mut expr: &'tcx Expr<'tcx>) -> &'tcx Expr<'tcx> {
    while let ExprKind::Block(ref block, ..) = expr.kind {
        match (block.stmts.is_empty(), block.expr.as_ref()) {
            (true, Some(e)) => expr = e,
            _ => break,
        }
    }
    expr
}

pub fn is_self(slf: &Param<'_>) -> bool {
    if let PatKind::Binding(.., name, _) = slf.pat.kind {
        name.name == kw::SelfLower
    } else {
        false
    }
}

pub fn is_self_ty(slf: &hir::Ty<'_>) -> bool {
    if_chain! {
        if let TyKind::Path(QPath::Resolved(None, ref path)) = slf.kind;
        if let Res::SelfTy(..) = path.res;
        then {
            return true
        }
    }
    false
}

pub fn iter_input_pats<'tcx>(decl: &FnDecl<'_>, body: &'tcx Body<'_>) -> impl Iterator<Item = &'tcx Param<'tcx>> {
    (0..decl.inputs.len()).map(move |i| &body.params[i])
}

/// Checks if a given expression is a match expression expanded from the `?`
/// operator or the `try` macro.
pub fn is_try<'tcx>(expr: &'tcx Expr<'tcx>) -> Option<&'tcx Expr<'tcx>> {
    fn is_ok(arm: &Arm<'_>) -> bool {
        if_chain! {
            if let PatKind::TupleStruct(ref path, ref pat, None) = arm.pat.kind;
            if match_qpath(path, &paths::RESULT_OK[1..]);
            if let PatKind::Binding(_, hir_id, _, None) = pat[0].kind;
            if path_to_local_id(arm.body, hir_id);
            then {
                return true;
            }
        }
        false
    }

    fn is_err(arm: &Arm<'_>) -> bool {
        if let PatKind::TupleStruct(ref path, _, _) = arm.pat.kind {
            match_qpath(path, &paths::RESULT_ERR[1..])
        } else {
            false
        }
    }

    if let ExprKind::Match(_, ref arms, ref source) = expr.kind {
        // desugared from a `?` operator
        if let MatchSource::TryDesugar = *source {
            return Some(expr);
        }

        if_chain! {
            if arms.len() == 2;
            if arms[0].guard.is_none();
            if arms[1].guard.is_none();
            if (is_ok(&arms[0]) && is_err(&arms[1])) ||
                (is_ok(&arms[1]) && is_err(&arms[0]));
            then {
                return Some(expr);
            }
        }
    }

    None
}

/// Returns `true` if the lint is allowed in the current context
///
/// Useful for skipping long running code when it's unnecessary
pub fn is_allowed(cx: &LateContext<'_>, lint: &'static Lint, id: HirId) -> bool {
    cx.tcx.lint_level_at_node(lint, id).0 == Level::Allow
}

pub fn strip_pat_refs<'hir>(mut pat: &'hir Pat<'hir>) -> &'hir Pat<'hir> {
    while let PatKind::Ref(subpat, _) = pat.kind {
        pat = subpat;
    }
    pat
}

pub fn int_bits(tcx: TyCtxt<'_>, ity: ty::IntTy) -> u64 {
    Integer::from_int_ty(&tcx, ity).size().bits()
}

#[allow(clippy::cast_possible_wrap)]
/// Turn a constant int byte representation into an i128
pub fn sext(tcx: TyCtxt<'_>, u: u128, ity: ty::IntTy) -> i128 {
    let amt = 128 - int_bits(tcx, ity);
    ((u as i128) << amt) >> amt
}

#[allow(clippy::cast_sign_loss)]
/// clip unused bytes
pub fn unsext(tcx: TyCtxt<'_>, u: i128, ity: ty::IntTy) -> u128 {
    let amt = 128 - int_bits(tcx, ity);
    ((u as u128) << amt) >> amt
}

/// clip unused bytes
pub fn clip(tcx: TyCtxt<'_>, u: u128, ity: ty::UintTy) -> u128 {
    let bits = Integer::from_uint_ty(&tcx, ity).size().bits();
    let amt = 128 - bits;
    (u << amt) >> amt
}

/// Removes block comments from the given `Vec` of lines.
///
/// # Examples
///
/// ```rust,ignore
/// without_block_comments(vec!["/*", "foo", "*/"]);
/// // => vec![]
///
/// without_block_comments(vec!["bar", "/*", "foo", "*/"]);
/// // => vec!["bar"]
/// ```
pub fn without_block_comments(lines: Vec<&str>) -> Vec<&str> {
    let mut without = vec![];

    let mut nest_level = 0;

    for line in lines {
        if line.contains("/*") {
            nest_level += 1;
            continue;
        } else if line.contains("*/") {
            nest_level -= 1;
            continue;
        }

        if nest_level == 0 {
            without.push(line);
        }
    }

    without
}

pub fn any_parent_is_automatically_derived(tcx: TyCtxt<'_>, node: HirId) -> bool {
    let map = &tcx.hir();
    let mut prev_enclosing_node = None;
    let mut enclosing_node = node;
    while Some(enclosing_node) != prev_enclosing_node {
        if is_automatically_derived(map.attrs(enclosing_node)) {
            return true;
        }
        prev_enclosing_node = Some(enclosing_node);
        enclosing_node = map.get_parent_item(enclosing_node);
    }
    false
}

/// Returns true if ty has `iter` or `iter_mut` methods
pub fn has_iter_method(cx: &LateContext<'_>, probably_ref_ty: Ty<'_>) -> Option<Symbol> {
    // FIXME: instead of this hard-coded list, we should check if `<adt>::iter`
    // exists and has the desired signature. Unfortunately FnCtxt is not exported
    // so we can't use its `lookup_method` method.
    let into_iter_collections: &[Symbol] = &[
        sym::vec_type,
        sym::option_type,
        sym::result_type,
        sym::BTreeMap,
        sym::BTreeSet,
        sym::vecdeque_type,
        sym::LinkedList,
        sym::BinaryHeap,
        sym::hashset_type,
        sym::hashmap_type,
        sym::PathBuf,
        sym::Path,
        sym::Receiver,
    ];

    let ty_to_check = match probably_ref_ty.kind() {
        ty::Ref(_, ty_to_check, _) => ty_to_check,
        _ => probably_ref_ty,
    };

    let def_id = match ty_to_check.kind() {
        ty::Array(..) => return Some(sym::array),
        ty::Slice(..) => return Some(sym::slice),
        ty::Adt(adt, _) => adt.did,
        _ => return None,
    };

    for &name in into_iter_collections {
        if cx.tcx.is_diagnostic_item(name, def_id) {
            return Some(cx.tcx.item_name(def_id));
        }
    }
    None
}

/// Matches a function call with the given path and returns the arguments.
///
/// Usage:
///
/// ```rust,ignore
/// if let Some(args) = match_function_call(cx, cmp_max_call, &paths::CMP_MAX);
/// ```
pub fn match_function_call<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'_>,
    path: &[&str],
) -> Option<&'tcx [Expr<'tcx>]> {
    if_chain! {
        if let ExprKind::Call(ref fun, ref args) = expr.kind;
        if let ExprKind::Path(ref qpath) = fun.kind;
        if let Some(fun_def_id) = cx.qpath_res(qpath, fun.hir_id).opt_def_id();
        if match_def_path(cx, fun_def_id, path);
        then {
            return Some(&args)
        }
    };
    None
}

// FIXME: Per https://doc.rust-lang.org/nightly/nightly-rustc/rustc_trait_selection/infer/at/struct.At.html#method.normalize
// this function can be removed once the `normalizie` method does not panic when normalization does
// not succeed
/// Checks if `Ty` is normalizable. This function is useful
/// to avoid crashes on `layout_of`.
pub fn is_normalizable<'tcx>(cx: &LateContext<'tcx>, param_env: ty::ParamEnv<'tcx>, ty: Ty<'tcx>) -> bool {
    is_normalizable_helper(cx, param_env, ty, &mut HashMap::new())
}

fn is_normalizable_helper<'tcx>(
    cx: &LateContext<'tcx>,
    param_env: ty::ParamEnv<'tcx>,
    ty: Ty<'tcx>,
    cache: &mut HashMap<Ty<'tcx>, bool>,
) -> bool {
    if let Some(&cached_result) = cache.get(ty) {
        return cached_result;
    }
    // prevent recursive loops, false-negative is better than endless loop leading to stack overflow
    cache.insert(ty, false);
    let result = cx.tcx.infer_ctxt().enter(|infcx| {
        let cause = rustc_middle::traits::ObligationCause::dummy();
        if infcx.at(&cause, param_env).normalize(ty).is_ok() {
            match ty.kind() {
                ty::Adt(def, substs) => def.variants.iter().all(|variant| {
                    variant
                        .fields
                        .iter()
                        .all(|field| is_normalizable_helper(cx, param_env, field.ty(cx.tcx, substs), cache))
                }),
                _ => ty.walk().all(|generic_arg| match generic_arg.unpack() {
                    GenericArgKind::Type(inner_ty) if inner_ty != ty => {
                        is_normalizable_helper(cx, param_env, inner_ty, cache)
                    },
                    _ => true, // if inner_ty == ty, we've already checked it
                }),
            }
        } else {
            false
        }
    });
    cache.insert(ty, result);
    result
}

pub fn match_def_path<'tcx>(cx: &LateContext<'tcx>, did: DefId, syms: &[&str]) -> bool {
    // We have to convert `syms` to `&[Symbol]` here because rustc's `match_def_path`
    // accepts only that. We should probably move to Symbols in Clippy as well.
    let syms = syms.iter().map(|p| Symbol::intern(p)).collect::<Vec<Symbol>>();
    cx.match_def_path(did, &syms)
}

pub fn match_panic_call<'tcx>(cx: &LateContext<'tcx>, expr: &'tcx Expr<'_>) -> Option<&'tcx [Expr<'tcx>]> {
    match_function_call(cx, expr, &paths::BEGIN_PANIC)
        .or_else(|| match_function_call(cx, expr, &paths::BEGIN_PANIC_FMT))
        .or_else(|| match_function_call(cx, expr, &paths::PANIC_ANY))
        .or_else(|| match_function_call(cx, expr, &paths::PANICKING_PANIC))
        .or_else(|| match_function_call(cx, expr, &paths::PANICKING_PANIC_FMT))
        .or_else(|| match_function_call(cx, expr, &paths::PANICKING_PANIC_STR))
}

pub fn match_panic_def_id(cx: &LateContext<'_>, did: DefId) -> bool {
    match_def_path(cx, did, &paths::BEGIN_PANIC)
        || match_def_path(cx, did, &paths::BEGIN_PANIC_FMT)
        || match_def_path(cx, did, &paths::PANIC_ANY)
        || match_def_path(cx, did, &paths::PANICKING_PANIC)
        || match_def_path(cx, did, &paths::PANICKING_PANIC_FMT)
        || match_def_path(cx, did, &paths::PANICKING_PANIC_STR)
}

/// Returns the list of condition expressions and the list of blocks in a
/// sequence of `if/else`.
/// E.g., this returns `([a, b], [c, d, e])` for the expression
/// `if a { c } else if b { d } else { e }`.
pub fn if_sequence<'tcx>(
    mut expr: &'tcx Expr<'tcx>,
) -> (SmallVec<[&'tcx Expr<'tcx>; 1]>, SmallVec<[&'tcx Block<'tcx>; 1]>) {
    let mut conds = SmallVec::new();
    let mut blocks: SmallVec<[&Block<'_>; 1]> = SmallVec::new();

    while let ExprKind::If(ref cond, ref then_expr, ref else_expr) = expr.kind {
        conds.push(&**cond);
        if let ExprKind::Block(ref block, _) = then_expr.kind {
            blocks.push(block);
        } else {
            panic!("ExprKind::If node is not an ExprKind::Block");
        }

        if let Some(ref else_expr) = *else_expr {
            expr = else_expr;
        } else {
            break;
        }
    }

    // final `else {..}`
    if !blocks.is_empty() {
        if let ExprKind::Block(ref block, _) = expr.kind {
            blocks.push(&**block);
        }
    }

    (conds, blocks)
}

pub fn parent_node_is_if_expr(expr: &Expr<'_>, cx: &LateContext<'_>) -> bool {
    let map = cx.tcx.hir();
    let parent_id = map.get_parent_node(expr.hir_id);
    let parent_node = map.get(parent_id);
    matches!(
        parent_node,
        Node::Expr(Expr {
            kind: ExprKind::If(_, _, _),
            ..
        })
    )
}

// Finds the attribute with the given name, if any
pub fn attr_by_name<'a>(attrs: &'a [Attribute], name: &'_ str) -> Option<&'a Attribute> {
    attrs
        .iter()
        .find(|attr| attr.ident().map_or(false, |ident| ident.as_str() == name))
}

// Finds the `#[must_use]` attribute, if any
pub fn must_use_attr(attrs: &[Attribute]) -> Option<&Attribute> {
    attr_by_name(attrs, "must_use")
}

// Returns whether the type has #[must_use] attribute
pub fn is_must_use_ty<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> bool {
    match ty.kind() {
        ty::Adt(ref adt, _) => must_use_attr(&cx.tcx.get_attrs(adt.did)).is_some(),
        ty::Foreign(ref did) => must_use_attr(&cx.tcx.get_attrs(*did)).is_some(),
        ty::Slice(ref ty)
        | ty::Array(ref ty, _)
        | ty::RawPtr(ty::TypeAndMut { ref ty, .. })
        | ty::Ref(_, ref ty, _) => {
            // for the Array case we don't need to care for the len == 0 case
            // because we don't want to lint functions returning empty arrays
            is_must_use_ty(cx, *ty)
        },
        ty::Tuple(ref substs) => substs.types().any(|ty| is_must_use_ty(cx, ty)),
        ty::Opaque(ref def_id, _) => {
            for (predicate, _) in cx.tcx.explicit_item_bounds(*def_id) {
                if let ty::PredicateKind::Trait(trait_predicate, _) = predicate.kind().skip_binder() {
                    if must_use_attr(&cx.tcx.get_attrs(trait_predicate.trait_ref.def_id)).is_some() {
                        return true;
                    }
                }
            }
            false
        },
        ty::Dynamic(binder, _) => {
            for predicate in binder.iter() {
                if let ty::ExistentialPredicate::Trait(ref trait_ref) = predicate.skip_binder() {
                    if must_use_attr(&cx.tcx.get_attrs(trait_ref.def_id)).is_some() {
                        return true;
                    }
                }
            }
            false
        },
        _ => false,
    }
}

// check if expr is calling method or function with #[must_use] attribute
pub fn is_must_use_func_call(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
    let did = match expr.kind {
        ExprKind::Call(ref path, _) => if_chain! {
            if let ExprKind::Path(ref qpath) = path.kind;
            if let def::Res::Def(_, did) = cx.qpath_res(qpath, path.hir_id);
            then {
                Some(did)
            } else {
                None
            }
        },
        ExprKind::MethodCall(_, _, _, _) => cx.typeck_results().type_dependent_def_id(expr.hir_id),
        _ => None,
    };

    did.map_or(false, |did| must_use_attr(&cx.tcx.get_attrs(did)).is_some())
}

pub fn is_no_std_crate(cx: &LateContext<'_>) -> bool {
    cx.tcx.hir().attrs(hir::CRATE_HIR_ID).iter().any(|attr| {
        if let ast::AttrKind::Normal(ref attr, _) = attr.kind {
            attr.path == sym::no_std
        } else {
            false
        }
    })
}

/// Check if parent of a hir node is a trait implementation block.
/// For example, `f` in
/// ```rust,ignore
/// impl Trait for S {
///     fn f() {}
/// }
/// ```
pub fn is_trait_impl_item(cx: &LateContext<'_>, hir_id: HirId) -> bool {
    if let Some(Node::Item(item)) = cx.tcx.hir().find(cx.tcx.hir().get_parent_node(hir_id)) {
        matches!(item.kind, ItemKind::Impl(hir::Impl { of_trait: Some(_), .. }))
    } else {
        false
    }
}

/// Check if it's even possible to satisfy the `where` clause for the item.
///
/// `trivial_bounds` feature allows functions with unsatisfiable bounds, for example:
///
/// ```ignore
/// fn foo() where i32: Iterator {
///     for _ in 2i32 {}
/// }
/// ```
pub fn fn_has_unsatisfiable_preds(cx: &LateContext<'_>, did: DefId) -> bool {
    use rustc_trait_selection::traits;
    let predicates = cx
        .tcx
        .predicates_of(did)
        .predicates
        .iter()
        .filter_map(|(p, _)| if p.is_global() { Some(*p) } else { None });
    traits::impossible_predicates(
        cx.tcx,
        traits::elaborate_predicates(cx.tcx, predicates)
            .map(|o| o.predicate)
            .collect::<Vec<_>>(),
    )
}

/// Returns the `DefId` of the callee if the given expression is a function or method call.
pub fn fn_def_id(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<DefId> {
    match &expr.kind {
        ExprKind::MethodCall(..) => cx.typeck_results().type_dependent_def_id(expr.hir_id),
        ExprKind::Call(
            Expr {
                kind: ExprKind::Path(qpath),
                hir_id: path_hir_id,
                ..
            },
            ..,
        ) => cx.typeck_results().qpath_res(qpath, *path_hir_id).opt_def_id(),
        _ => None,
    }
}

pub fn run_lints(cx: &LateContext<'_>, lints: &[&'static Lint], id: HirId) -> bool {
    lints.iter().any(|lint| {
        matches!(
            cx.tcx.lint_level_at_node(lint, id),
            (Level::Forbid | Level::Deny | Level::Warn, _)
        )
    })
}

/// Returns true iff the given type is a primitive (a bool or char, any integer or floating-point
/// number type, a str, or an array, slice, or tuple of those types).
pub fn is_recursively_primitive_type(ty: Ty<'_>) -> bool {
    match ty.kind() {
        ty::Bool | ty::Char | ty::Int(_) | ty::Uint(_) | ty::Float(_) | ty::Str => true,
        ty::Ref(_, inner, _) if *inner.kind() == ty::Str => true,
        ty::Array(inner_type, _) | ty::Slice(inner_type) => is_recursively_primitive_type(inner_type),
        ty::Tuple(inner_types) => inner_types.types().all(is_recursively_primitive_type),
        _ => false,
    }
}

/// Returns Option<String> where String is a textual representation of the type encapsulated in the
/// slice iff the given expression is a slice of primitives (as defined in the
/// `is_recursively_primitive_type` function) and None otherwise.
pub fn is_slice_of_primitives(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<String> {
    let expr_type = cx.typeck_results().expr_ty_adjusted(expr);
    let expr_kind = expr_type.kind();
    let is_primitive = match expr_kind {
        ty::Slice(element_type) => is_recursively_primitive_type(element_type),
        ty::Ref(_, inner_ty, _) if matches!(inner_ty.kind(), &ty::Slice(_)) => {
            if let ty::Slice(element_type) = inner_ty.kind() {
                is_recursively_primitive_type(element_type)
            } else {
                unreachable!()
            }
        },
        _ => false,
    };

    if is_primitive {
        // if we have wrappers like Array, Slice or Tuple, print these
        // and get the type enclosed in the slice ref
        match expr_type.peel_refs().walk().nth(1).unwrap().expect_ty().kind() {
            ty::Slice(..) => return Some("slice".into()),
            ty::Array(..) => return Some("array".into()),
            ty::Tuple(..) => return Some("tuple".into()),
            _ => {
                // is_recursively_primitive_type() should have taken care
                // of the rest and we can rely on the type that is found
                let refs_peeled = expr_type.peel_refs();
                return Some(refs_peeled.walk().last().unwrap().to_string());
            },
        }
    }
    None
}

/// returns list of all pairs (a, b) from `exprs` such that `eq(a, b)`
/// `hash` must be comformed with `eq`
pub fn search_same<T, Hash, Eq>(exprs: &[T], hash: Hash, eq: Eq) -> Vec<(&T, &T)>
where
    Hash: Fn(&T) -> u64,
    Eq: Fn(&T, &T) -> bool,
{
    if exprs.len() == 2 && eq(&exprs[0], &exprs[1]) {
        return vec![(&exprs[0], &exprs[1])];
    }

    let mut match_expr_list: Vec<(&T, &T)> = Vec::new();

    let mut map: FxHashMap<_, Vec<&_>> =
        FxHashMap::with_capacity_and_hasher(exprs.len(), BuildHasherDefault::default());

    for expr in exprs {
        match map.entry(hash(expr)) {
            Entry::Occupied(mut o) => {
                for o in o.get() {
                    if eq(o, expr) {
                        match_expr_list.push((o, expr));
                    }
                }
                o.get_mut().push(expr);
            },
            Entry::Vacant(v) => {
                v.insert(vec![expr]);
            },
        }
    }

    match_expr_list
}

/// Peels off all references on the pattern. Returns the underlying pattern and the number of
/// references removed.
pub fn peel_hir_pat_refs(pat: &'a Pat<'a>) -> (&'a Pat<'a>, usize) {
    fn peel(pat: &'a Pat<'a>, count: usize) -> (&'a Pat<'a>, usize) {
        if let PatKind::Ref(pat, _) = pat.kind {
            peel(pat, count + 1)
        } else {
            (pat, count)
        }
    }
    peel(pat, 0)
}

/// Peels off up to the given number of references on the expression. Returns the underlying
/// expression and the number of references removed.
pub fn peel_n_hir_expr_refs(expr: &'a Expr<'a>, count: usize) -> (&'a Expr<'a>, usize) {
    fn f(expr: &'a Expr<'a>, count: usize, target: usize) -> (&'a Expr<'a>, usize) {
        match expr.kind {
            ExprKind::AddrOf(_, _, expr) if count != target => f(expr, count + 1, target),
            _ => (expr, count),
        }
    }
    f(expr, 0, count)
}

/// Peels off all references on the expression. Returns the underlying expression and the number of
/// references removed.
pub fn peel_hir_expr_refs(expr: &'a Expr<'a>) -> (&'a Expr<'a>, usize) {
    fn f(expr: &'a Expr<'a>, count: usize) -> (&'a Expr<'a>, usize) {
        match expr.kind {
            ExprKind::AddrOf(BorrowKind::Ref, _, expr) => f(expr, count + 1),
            _ => (expr, count),
        }
    }
    f(expr, 0)
}

/// Peels off all references on the type. Returns the underlying type and the number of references
/// removed.
pub fn peel_mid_ty_refs(ty: Ty<'_>) -> (Ty<'_>, usize) {
    fn peel(ty: Ty<'_>, count: usize) -> (Ty<'_>, usize) {
        if let ty::Ref(_, ty, _) = ty.kind() {
            peel(ty, count + 1)
        } else {
            (ty, count)
        }
    }
    peel(ty, 0)
}

/// Peels off all references on the type.Returns the underlying type, the number of references
/// removed, and whether the pointer is ultimately mutable or not.
pub fn peel_mid_ty_refs_is_mutable(ty: Ty<'_>) -> (Ty<'_>, usize, Mutability) {
    fn f(ty: Ty<'_>, count: usize, mutability: Mutability) -> (Ty<'_>, usize, Mutability) {
        match ty.kind() {
            ty::Ref(_, ty, Mutability::Mut) => f(ty, count + 1, mutability),
            ty::Ref(_, ty, Mutability::Not) => f(ty, count + 1, Mutability::Not),
            _ => (ty, count, mutability),
        }
    }
    f(ty, 0, Mutability::Mut)
}

#[macro_export]
macro_rules! unwrap_cargo_metadata {
    ($cx: ident, $lint: ident, $deps: expr) => {{
        let mut command = cargo_metadata::MetadataCommand::new();
        if !$deps {
            command.no_deps();
        }

        match command.exec() {
            Ok(metadata) => metadata,
            Err(err) => {
                span_lint($cx, $lint, DUMMY_SP, &format!("could not read cargo metadata: {}", err));
                return;
            },
        }
    }};
}

pub fn is_hir_ty_cfg_dependant(cx: &LateContext<'_>, ty: &hir::Ty<'_>) -> bool {
    if_chain! {
        if let TyKind::Path(QPath::Resolved(_, path)) = ty.kind;
        if let Res::Def(_, def_id) = path.res;
        then {
            cx.tcx.has_attr(def_id, sym::cfg) || cx.tcx.has_attr(def_id, sym::cfg_attr)
        } else {
            false
        }
    }
}

/// Check if the resolution of a given path is an `Ok` variant of `Result`.
pub fn is_ok_ctor(cx: &LateContext<'_>, res: Res) -> bool {
    if let Some(ok_id) = cx.tcx.lang_items().result_ok_variant() {
        if let Res::Def(DefKind::Ctor(CtorOf::Variant, CtorKind::Fn), id) = res {
            if let Some(variant_id) = cx.tcx.parent(id) {
                return variant_id == ok_id;
            }
        }
    }
    false
}

/// Check if the resolution of a given path is a `Some` variant of `Option`.
pub fn is_some_ctor(cx: &LateContext<'_>, res: Res) -> bool {
    if let Some(some_id) = cx.tcx.lang_items().option_some_variant() {
        if let Res::Def(DefKind::Ctor(CtorOf::Variant, CtorKind::Fn), id) = res {
            if let Some(variant_id) = cx.tcx.parent(id) {
                return variant_id == some_id;
            }
        }
    }
    false
}

#[cfg(test)]
mod test {
    use super::{reindent_multiline, without_block_comments};

    #[test]
    fn test_reindent_multiline_single_line() {
        assert_eq!("", reindent_multiline("".into(), false, None));
        assert_eq!("...", reindent_multiline("...".into(), false, None));
        assert_eq!("...", reindent_multiline("    ...".into(), false, None));
        assert_eq!("...", reindent_multiline("\t...".into(), false, None));
        assert_eq!("...", reindent_multiline("\t\t...".into(), false, None));
    }

    #[test]
    #[rustfmt::skip]
    fn test_reindent_multiline_block() {
        assert_eq!("\
    if x {
        y
    } else {
        z
    }", reindent_multiline("    if x {
            y
        } else {
            z
        }".into(), false, None));
        assert_eq!("\
    if x {
    \ty
    } else {
    \tz
    }", reindent_multiline("    if x {
        \ty
        } else {
        \tz
        }".into(), false, None));
    }

    #[test]
    #[rustfmt::skip]
    fn test_reindent_multiline_empty_line() {
        assert_eq!("\
    if x {
        y

    } else {
        z
    }", reindent_multiline("    if x {
            y

        } else {
            z
        }".into(), false, None));
    }

    #[test]
    #[rustfmt::skip]
    fn test_reindent_multiline_lines_deeper() {
        assert_eq!("\
        if x {
            y
        } else {
            z
        }", reindent_multiline("\
    if x {
        y
    } else {
        z
    }".into(), true, Some(8)));
    }

    #[test]
    fn test_without_block_comments_lines_without_block_comments() {
        let result = without_block_comments(vec!["/*", "", "*/"]);
        println!("result: {:?}", result);
        assert!(result.is_empty());

        let result = without_block_comments(vec!["", "/*", "", "*/", "#[crate_type = \"lib\"]", "/*", "", "*/", ""]);
        assert_eq!(result, vec!["", "#[crate_type = \"lib\"]", ""]);

        let result = without_block_comments(vec!["/* rust", "", "*/"]);
        assert!(result.is_empty());

        let result = without_block_comments(vec!["/* one-line comment */"]);
        assert!(result.is_empty());

        let result = without_block_comments(vec!["/* nested", "/* multi-line", "comment", "*/", "test", "*/"]);
        assert!(result.is_empty());

        let result = without_block_comments(vec!["/* nested /* inline /* comment */ test */ */"]);
        assert!(result.is_empty());

        let result = without_block_comments(vec!["foo", "bar", "baz"]);
        assert_eq!(result, vec!["foo", "bar", "baz"]);
    }
}
