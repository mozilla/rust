//! Checks for needless address of operations (`&`)
//!
//! This lint is **warn** by default

use crate::utils::{is_automatically_derived, snippet_opt, span_lint_and_then};
use if_chain::if_chain;
use rustc_errors::Applicability;
use rustc_hir::{BindingAnnotation, BorrowKind, Expr, ExprKind, Item, Mutability, Pat, PatKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty;
use rustc_middle::ty::adjustment::{Adjust, Adjustment};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::def_id::LocalDefId;

declare_clippy_lint! {
    /// **What it does:** Checks for address of operations (`&`) that are going to
    /// be dereferenced immediately by the compiler.
    ///
    /// **Why is this bad?** Suggests that the receiver of the expression borrows
    /// the expression.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    /// ```rust
    /// // Bad
    /// let x: &i32 = &&&&&&5;
    ///
    /// // Good
    /// let x: &i32 = &5;
    /// ```
    pub NEEDLESS_BORROW,
    nursery,
    "taking a reference that is going to be automatically dereferenced"
}

#[derive(Default)]
pub struct NeedlessBorrow {
    derived_item: Option<LocalDefId>,
}

impl_lint_pass!(NeedlessBorrow => [NEEDLESS_BORROW]);

impl<'tcx> LateLintPass<'tcx> for NeedlessBorrow {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, e: &'tcx Expr<'_>) {
        let e_span = cx.tcx.hir().span(e.hir_id);
        if e_span.from_expansion() || self.derived_item.is_some() {
            return;
        }
        if let ExprKind::AddrOf(BorrowKind::Ref, Mutability::Not, ref inner) = e.kind {
            if let ty::Ref(_, ty, _) = cx.typeck_results().expr_ty(inner).kind() {
                for adj3 in cx.typeck_results().expr_adjustments(e).windows(3) {
                    if let [Adjustment {
                        kind: Adjust::Deref(_), ..
                    }, Adjustment {
                        kind: Adjust::Deref(_), ..
                    }, Adjustment {
                        kind: Adjust::Borrow(_),
                        ..
                    }] = *adj3
                    {
                        span_lint_and_then(
                            cx,
                            NEEDLESS_BORROW,
                            e_span,
                            &format!(
                                "this expression borrows a reference (`&{}`) that is immediately dereferenced \
                             by the compiler",
                                ty
                            ),
                            |diag| {
                                if let Some(snippet) = snippet_opt(cx, cx.tcx.hir().span(inner.hir_id)) {
                                    diag.span_suggestion(
                                        e_span,
                                        "change this to",
                                        snippet,
                                        Applicability::MachineApplicable,
                                    );
                                }
                            },
                        );
                    }
                }
            }
        }
    }
    fn check_pat(&mut self, cx: &LateContext<'tcx>, pat: &'tcx Pat<'_>) {
        let pat_span = cx.tcx.hir().span(pat.hir_id);
        if pat_span.from_expansion() || self.derived_item.is_some() {
            return;
        }
        if_chain! {
            if let PatKind::Binding(BindingAnnotation::Ref, .., name, _) = pat.kind;
            if let ty::Ref(_, tam, mutbl) = *cx.typeck_results().pat_ty(pat).kind();
            if mutbl == Mutability::Not;
            if let ty::Ref(_, _, mutbl) = *tam.kind();
            // only lint immutable refs, because borrowed `&mut T` cannot be moved out
            if mutbl == Mutability::Not;
            then {
                span_lint_and_then(
                    cx,
                    NEEDLESS_BORROW,
                    pat_span,
                    "this pattern creates a reference to a reference",
                    |diag| {
                        if let Some(snippet) = snippet_opt(cx, name.span) {
                            diag.span_suggestion(
                                pat_span,
                                "change this to",
                                snippet,
                                Applicability::MachineApplicable,
                            );
                        }
                    }
                )
            }
        }
    }

    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'_>) {
        let attrs = cx.tcx.hir().attrs(item.hir_id());
        if is_automatically_derived(attrs) {
            debug_assert!(self.derived_item.is_none());
            self.derived_item = Some(item.def_id);
        }
    }

    fn check_item_post(&mut self, _: &LateContext<'tcx>, item: &'tcx Item<'_>) {
        if let Some(id) = self.derived_item {
            if item.def_id == id {
                self.derived_item = None;
            }
        }
    }
}
