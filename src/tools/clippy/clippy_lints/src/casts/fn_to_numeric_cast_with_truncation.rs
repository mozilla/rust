use rustc_errors::Applicability;
use rustc_hir::Expr;
use rustc_lint::LateContext;
use rustc_middle::ty::{self, Ty};

use crate::utils::{snippet_with_applicability, span_lint_and_sugg};

use super::{utils, FN_TO_NUMERIC_CAST_WITH_TRUNCATION};

pub(super) fn check(cx: &LateContext<'_>, expr: &Expr<'_>, cast_expr: &Expr<'_>, cast_from: Ty<'_>, cast_to: Ty<'_>) {
    // We only want to check casts to `ty::Uint` or `ty::Int`
    match cast_to.kind() {
        ty::Uint(_) | ty::Int(..) => { /* continue on */ },
        _ => return,
    }
    match cast_from.kind() {
        ty::FnDef(..) | ty::FnPtr(_) => {
            let mut applicability = Applicability::MaybeIncorrect;
            let from_snippet =
                snippet_with_applicability(cx, cx.tcx.hir().span(cast_expr.hir_id), "x", &mut applicability);

            let to_nbits = utils::int_ty_to_nbits(cast_to, cx.tcx);
            if to_nbits < cx.tcx.data_layout.pointer_size.bits() {
                span_lint_and_sugg(
                    cx,
                    FN_TO_NUMERIC_CAST_WITH_TRUNCATION,
                    cx.tcx.hir().span(expr.hir_id),
                    &format!(
                        "casting function pointer `{}` to `{}`, which truncates the value",
                        from_snippet, cast_to
                    ),
                    "try",
                    format!("{} as usize", from_snippet),
                    applicability,
                );
            }
        },
        _ => {},
    }
}
