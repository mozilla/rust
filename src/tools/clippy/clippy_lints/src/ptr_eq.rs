use crate::utils;
use if_chain::if_chain;
use rustc_errors::Applicability;
use rustc_hir::{BinOpKind, Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_session::{declare_lint_pass, declare_tool_lint};

declare_clippy_lint! {
    /// **What it does:** Use `std::ptr::eq` when applicable
    ///
    /// **Why is this bad?** `ptr::eq` can be used to compare `&T` references
    /// (which coerce to `*const T` implicitly) by their address rather than
    /// comparing the values they point to.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    ///
    /// ```rust
    /// let a = &[1, 2, 3];
    /// let b = &[1, 2, 3];
    ///
    /// assert!(a as *const _ as usize == b as *const _ as usize);
    /// ```
    /// Use instead:
    /// ```rust
    /// let a = &[1, 2, 3];
    /// let b = &[1, 2, 3];
    ///
    /// assert!(std::ptr::eq(a, b));
    /// ```
    pub PTR_EQ,
    style,
    "use `std::ptr::eq` when comparing raw pointers"
}

declare_lint_pass!(PtrEq => [PTR_EQ]);

static LINT_MSG: &str = "use `std::ptr::eq` when comparing raw pointers";

impl LateLintPass<'_> for PtrEq {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'_>) {
        if utils::in_macro(cx.tcx.hir().span(expr.hir_id)) {
            return;
        }

        if let ExprKind::Binary(ref op, ref left, ref right) = expr.kind {
            if BinOpKind::Eq == op.node {
                let (left, right) = match (expr_as_cast_to_usize(cx, left), expr_as_cast_to_usize(cx, right)) {
                    (Some(lhs), Some(rhs)) => (lhs, rhs),
                    _ => (&**left, &**right),
                };

                if_chain! {
                    if let Some(left_var) = expr_as_cast_to_raw_pointer(cx, left);
                    if let Some(right_var) = expr_as_cast_to_raw_pointer(cx, right);
                    if let Some(left_snip) = utils::snippet_opt(cx, cx.tcx.hir().span(left_var.hir_id));
                    if let Some(right_snip) = utils::snippet_opt(cx, cx.tcx.hir().span(right_var.hir_id));
                    then {
                        utils::span_lint_and_sugg(
                            cx,
                            PTR_EQ,
                            cx.tcx.hir().span(expr.hir_id),
                            LINT_MSG,
                            "try",
                            format!("std::ptr::eq({}, {})", left_snip, right_snip),
                            Applicability::MachineApplicable,
                            );
                    }
                }
            }
        }
    }
}

// If the given expression is a cast to an usize, return the lhs of the cast
// E.g., `foo as *const _ as usize` returns `foo as *const _`.
fn expr_as_cast_to_usize<'tcx>(cx: &LateContext<'tcx>, cast_expr: &'tcx Expr<'_>) -> Option<&'tcx Expr<'tcx>> {
    if cx.typeck_results().expr_ty(cast_expr) == cx.tcx.types.usize {
        if let ExprKind::Cast(ref expr, _) = cast_expr.kind {
            return Some(expr);
        }
    }
    None
}

// If the given expression is a cast to a `*const` pointer, return the lhs of the cast
// E.g., `foo as *const _` returns `foo`.
fn expr_as_cast_to_raw_pointer<'tcx>(cx: &LateContext<'tcx>, cast_expr: &'tcx Expr<'_>) -> Option<&'tcx Expr<'tcx>> {
    if cx.typeck_results().expr_ty(cast_expr).is_unsafe_ptr() {
        if let ExprKind::Cast(ref expr, _) = cast_expr.kind {
            return Some(expr);
        }
    }
    None
}
