use super::FOR_LOOPS_OVER_FALLIBLES;
use crate::utils::{is_type_diagnostic_item, snippet, span_lint_and_help};
use rustc_hir::{Expr, Pat};
use rustc_lint::LateContext;
use rustc_span::symbol::sym;

/// Checks for `for` loops over `Option`s and `Result`s.
pub(super) fn check(cx: &LateContext<'_>, pat: &Pat<'_>, arg: &Expr<'_>) {
    let ty = cx.typeck_results().expr_ty(arg);
    let arg_span = cx.tcx.hir().span(arg.hir_id);
    if is_type_diagnostic_item(cx, ty, sym::option_type) {
        span_lint_and_help(
            cx,
            FOR_LOOPS_OVER_FALLIBLES,
            arg_span,
            &format!(
                "for loop over `{0}`, which is an `Option`. This is more readably written as an \
                `if let` statement",
                snippet(cx, arg_span, "_")
            ),
            None,
            &format!(
                "consider replacing `for {0} in {1}` with `if let Some({0}) = {1}`",
                snippet(cx, cx.tcx.hir().span(pat.hir_id), "_"),
                snippet(cx, arg_span, "_")
            ),
        );
    } else if is_type_diagnostic_item(cx, ty, sym::result_type) {
        span_lint_and_help(
            cx,
            FOR_LOOPS_OVER_FALLIBLES,
            arg_span,
            &format!(
                "for loop over `{0}`, which is a `Result`. This is more readably written as an \
                `if let` statement",
                snippet(cx, arg_span, "_")
            ),
            None,
            &format!(
                "consider replacing `for {0} in {1}` with `if let Ok({0}) = {1}`",
                snippet(cx, cx.tcx.hir().span(pat.hir_id), "_"),
                snippet(cx, arg_span, "_")
            ),
        );
    }
}
