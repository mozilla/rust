use rustc_hir::{Expr, ExprKind, MutTy, Mutability, TyKind, UnOp};
use rustc_lint::LateContext;
use rustc_middle::ty;

use if_chain::if_chain;

use crate::utils::span_lint;

use super::CAST_REF_TO_MUT;

pub(super) fn check(cx: &LateContext<'tcx>, expr: &'tcx Expr<'_>) {
    if_chain! {
        if let ExprKind::Unary(UnOp::Deref, e) = &expr.kind;
        if let ExprKind::Cast(e, t) = &e.kind;
        if let TyKind::Ptr(MutTy { mutbl: Mutability::Mut, .. }) = t.kind;
        if let ExprKind::Cast(e, t) = &e.kind;
        if let TyKind::Ptr(MutTy { mutbl: Mutability::Not, .. }) = t.kind;
        if let ty::Ref(..) = cx.typeck_results().node_type(e.hir_id).kind();
        then {
            span_lint(
                cx,
                CAST_REF_TO_MUT,
                cx.tcx.hir().span(expr.hir_id),
                "casting `&T` to `&mut T` may cause undefined behavior, consider instead using an `UnsafeCell`",
            );
        }
    }
}
