use super::utils::can_be_expressed_as_pointer_cast;
use super::TRANSMUTES_EXPRESSIBLE_AS_PTR_CASTS;
use crate::utils::{span_lint_and_then, sugg};
use rustc_errors::Applicability;
use rustc_hir::Expr;
use rustc_lint::LateContext;
use rustc_middle::ty::Ty;

/// Checks for `transmutes_expressible_as_ptr_casts` lint.
/// Returns `true` if it's triggered, otherwise returns `false`.
pub(super) fn check<'tcx>(
    cx: &LateContext<'tcx>,
    e: &'tcx Expr<'_>,
    from_ty: Ty<'tcx>,
    to_ty: Ty<'tcx>,
    args: &'tcx [Expr<'_>],
) -> bool {
    if can_be_expressed_as_pointer_cast(cx, e, from_ty, to_ty) {
        span_lint_and_then(
            cx,
            TRANSMUTES_EXPRESSIBLE_AS_PTR_CASTS,
            cx.tcx.hir().span(e.hir_id),
            &format!(
                "transmute from `{}` to `{}` which could be expressed as a pointer cast instead",
                from_ty, to_ty
            ),
            |diag| {
                if let Some(arg) = sugg::Sugg::hir_opt(cx, &args[0]) {
                    let sugg = arg.as_ty(&to_ty.to_string()).to_string();
                    diag.span_suggestion(
                        cx.tcx.hir().span(e.hir_id),
                        "try",
                        sugg,
                        Applicability::MachineApplicable,
                    );
                }
            },
        );
        true
    } else {
        false
    }
}
