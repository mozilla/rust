use rustc_hir::{self as hir, def_id::DefId, QPath};
use rustc_lint::LateContext;
use rustc_span::symbol::sym;

use crate::utils::{is_ty_param_diagnostic_item, span_lint};

use super::OPTION_OPTION;

pub(super) fn check(cx: &LateContext<'_>, hir_ty: &hir::Ty<'_>, qpath: &QPath<'_>, def_id: DefId) -> bool {
    if cx.tcx.is_diagnostic_item(sym::option_type, def_id)
        && is_ty_param_diagnostic_item(cx, qpath, sym::option_type).is_some()
    {
        span_lint(
            cx,
            OPTION_OPTION,
            cx.tcx.hir().span(hir_ty.hir_id),
            "consider using `Option<T>` instead of `Option<Option<T>>` or a custom \
                                 enum if you need to distinguish all 3 cases",
        );
        true
    } else {
        false
    }
}
