use rustc_ast::LitKind;
use rustc_errors::Applicability;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::LateContext;
use rustc_middle::ty::{self, UintTy};

use if_chain::if_chain;

use crate::utils::{snippet_with_applicability, span_lint_and_then};

use super::CHAR_LIT_AS_U8;

pub(super) fn check(cx: &LateContext<'tcx>, expr: &'tcx Expr<'_>) {
    if_chain! {
        if let ExprKind::Cast(e, _) = &expr.kind;
        if let ExprKind::Lit(l) = &e.kind;
        if let LitKind::Char(c) = l.node;
        if ty::Uint(UintTy::U8) == *cx.typeck_results().expr_ty(expr).kind();
        then {
            let mut applicability = Applicability::MachineApplicable;
            let snippet = snippet_with_applicability(cx, cx.tcx.hir().span(e.hir_id), "'x'", &mut applicability);

            span_lint_and_then(
                cx,
                CHAR_LIT_AS_U8,
                cx.tcx.hir().span(expr.hir_id),
                "casting a character literal to `u8` truncates",
                |diag| {
                    diag.note("`char` is four bytes wide, but `u8` is a single byte");

                    if c.is_ascii() {
                        diag.span_suggestion(
                            cx.tcx.hir().span(expr.hir_id),
                            "use a byte literal instead",
                            format!("b{}", snippet),
                            applicability,
                        );
                    }
            });
        }
    }
}
