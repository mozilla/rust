use rustc_errors::Applicability;
use rustc_hir::{
    self as hir, GenericArg, GenericBounds, GenericParamKind, HirId, Lifetime, MutTy, Mutability, Node, QPath,
    SyntheticTyParamKind, TyKind,
};
use rustc_lint::LateContext;

use if_chain::if_chain;

use crate::utils::{match_path, paths, snippet, span_lint_and_sugg};

use super::BORROWED_BOX;

pub(super) fn check(cx: &LateContext<'_>, hir_ty: &hir::Ty<'_>, lt: &Lifetime, mut_ty: &MutTy<'_>) -> bool {
    match mut_ty.ty.kind {
        TyKind::Path(ref qpath) => {
            let hir_id = mut_ty.ty.hir_id;
            let def = cx.qpath_res(qpath, hir_id);
            if_chain! {
                if let Some(def_id) = def.opt_def_id();
                if Some(def_id) == cx.tcx.lang_items().owned_box();
                if let QPath::Resolved(None, ref path) = *qpath;
                if let [ref bx] = *path.segments;
                if let Some(ref params) = bx.args;
                if !params.parenthesized;
                if let Some(inner) = params.args.iter().find_map(|arg| match arg {
                    GenericArg::Type(ty) => Some(ty),
                    _ => None,
                });
                then {
                    if is_any_trait(inner) {
                        // Ignore `Box<Any>` types; see issue #1884 for details.
                        return false;
                    }

                    let ltopt = if lt.is_elided() {
                        String::new()
                    } else {
                        format!("{} ", lt.name.ident().as_str())
                    };

                    if mut_ty.mutbl == Mutability::Mut {
                        // Ignore `&mut Box<T>` types; see issue #2907 for
                        // details.
                        return false;
                    }

                    // When trait objects or opaque types have lifetime or auto-trait bounds,
                    // we need to add parentheses to avoid a syntax error due to its ambiguity.
                    // Originally reported as the issue #3128.
                    let inner_snippet = snippet(cx, inner.span, "..");
                    let suggestion = match &inner.kind {
                        TyKind::TraitObject(bounds, lt_bound) if bounds.len() > 1 || !lt_bound.is_elided() => {
                            format!("&{}({})", ltopt, &inner_snippet)
                        },
                        TyKind::Path(qpath)
                            if get_bounds_if_impl_trait(cx, qpath, inner.hir_id)
                                .map_or(false, |bounds| bounds.len() > 1) =>
                        {
                            format!("&{}({})", ltopt, &inner_snippet)
                        },
                        _ => format!("&{}{}", ltopt, &inner_snippet),
                    };
                    span_lint_and_sugg(
                        cx,
                        BORROWED_BOX,
                        hir_ty.span,
                        "you seem to be trying to use `&Box<T>`. Consider using just `&T`",
                        "try",
                        suggestion,
                        // To make this `MachineApplicable`, at least one needs to check if it isn't a trait item
                        // because the trait impls of it will break otherwise;
                        // and there may be other cases that result in invalid code.
                        // For example, type coercion doesn't work nicely.
                        Applicability::Unspecified,
                    );
                    return true;
                }
            };
            false
        },
        _ => false,
    }
}

// Returns true if given type is `Any` trait.
fn is_any_trait(t: &hir::Ty<'_>) -> bool {
    if_chain! {
        if let TyKind::TraitObject(ref traits, _) = t.kind;
        if !traits.is_empty();
        // Only Send/Sync can be used as additional traits, so it is enough to
        // check only the first trait.
        if match_path(&traits[0].trait_ref.path, &paths::ANY_TRAIT);
        then {
            return true;
        }
    }

    false
}

fn get_bounds_if_impl_trait<'tcx>(cx: &LateContext<'tcx>, qpath: &QPath<'_>, id: HirId) -> Option<GenericBounds<'tcx>> {
    if_chain! {
        if let Some(did) = cx.qpath_res(qpath, id).opt_def_id();
        if let Some(Node::GenericParam(generic_param)) = cx.tcx.hir().get_if_local(did);
        if let GenericParamKind::Type { synthetic, .. } = generic_param.kind;
        if synthetic == Some(SyntheticTyParamKind::ImplTrait);
        then {
            Some(generic_param.bounds)
        } else {
            None
        }
    }
}
