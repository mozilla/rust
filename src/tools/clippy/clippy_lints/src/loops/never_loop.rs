use super::NEVER_LOOP;
use crate::utils::span_lint;
use rustc_hir::{Block, Expr, ExprKind, HirId, InlineAsmOperand, Stmt, StmtKind};
use rustc_lint::LateContext;
use std::iter::{once, Iterator};

pub(super) fn check(cx: &LateContext<'tcx>, expr: &'tcx Expr<'_>) {
    if let ExprKind::Loop(ref block, _, _, _) = expr.kind {
        match never_loop_block(block, expr.hir_id) {
            NeverLoopResult::AlwaysBreak => span_lint(
                cx,
                NEVER_LOOP,
                cx.tcx.hir().span(expr.hir_id),
                "this loop never actually loops",
            ),
            NeverLoopResult::MayContinueMainLoop | NeverLoopResult::Otherwise => (),
        }
    }
}

enum NeverLoopResult {
    // A break/return always get triggered but not necessarily for the main loop.
    AlwaysBreak,
    // A continue may occur for the main loop.
    MayContinueMainLoop,
    Otherwise,
}

#[must_use]
fn absorb_break(arg: &NeverLoopResult) -> NeverLoopResult {
    match *arg {
        NeverLoopResult::AlwaysBreak | NeverLoopResult::Otherwise => NeverLoopResult::Otherwise,
        NeverLoopResult::MayContinueMainLoop => NeverLoopResult::MayContinueMainLoop,
    }
}

// Combine two results for parts that are called in order.
#[must_use]
fn combine_seq(first: NeverLoopResult, second: NeverLoopResult) -> NeverLoopResult {
    match first {
        NeverLoopResult::AlwaysBreak | NeverLoopResult::MayContinueMainLoop => first,
        NeverLoopResult::Otherwise => second,
    }
}

// Combine two results where both parts are called but not necessarily in order.
#[must_use]
fn combine_both(left: NeverLoopResult, right: NeverLoopResult) -> NeverLoopResult {
    match (left, right) {
        (NeverLoopResult::MayContinueMainLoop, _) | (_, NeverLoopResult::MayContinueMainLoop) => {
            NeverLoopResult::MayContinueMainLoop
        },
        (NeverLoopResult::AlwaysBreak, _) | (_, NeverLoopResult::AlwaysBreak) => NeverLoopResult::AlwaysBreak,
        (NeverLoopResult::Otherwise, NeverLoopResult::Otherwise) => NeverLoopResult::Otherwise,
    }
}

// Combine two results where only one of the part may have been executed.
#[must_use]
fn combine_branches(b1: NeverLoopResult, b2: NeverLoopResult) -> NeverLoopResult {
    match (b1, b2) {
        (NeverLoopResult::AlwaysBreak, NeverLoopResult::AlwaysBreak) => NeverLoopResult::AlwaysBreak,
        (NeverLoopResult::MayContinueMainLoop, _) | (_, NeverLoopResult::MayContinueMainLoop) => {
            NeverLoopResult::MayContinueMainLoop
        },
        (NeverLoopResult::Otherwise, _) | (_, NeverLoopResult::Otherwise) => NeverLoopResult::Otherwise,
    }
}

fn never_loop_block(block: &Block<'_>, main_loop_id: HirId) -> NeverLoopResult {
    let stmts = block.stmts.iter().map(stmt_to_expr);
    let expr = once(block.expr.as_deref());
    let mut iter = stmts.chain(expr).flatten();
    never_loop_expr_seq(&mut iter, main_loop_id)
}

fn never_loop_expr_seq<'a, T: Iterator<Item = &'a Expr<'a>>>(es: &mut T, main_loop_id: HirId) -> NeverLoopResult {
    es.map(|e| never_loop_expr(e, main_loop_id))
        .fold(NeverLoopResult::Otherwise, combine_seq)
}

fn stmt_to_expr<'tcx>(stmt: &Stmt<'tcx>) -> Option<&'tcx Expr<'tcx>> {
    match stmt.kind {
        StmtKind::Semi(ref e, ..) | StmtKind::Expr(ref e, ..) => Some(e),
        StmtKind::Local(ref local) => local.init.as_deref(),
        _ => None,
    }
}

fn never_loop_expr(expr: &Expr<'_>, main_loop_id: HirId) -> NeverLoopResult {
    match expr.kind {
        ExprKind::Box(ref e)
        | ExprKind::Unary(_, ref e)
        | ExprKind::Cast(ref e, _)
        | ExprKind::Type(ref e, _)
        | ExprKind::Field(ref e, _)
        | ExprKind::AddrOf(_, _, ref e)
        | ExprKind::Struct(_, _, Some(ref e))
        | ExprKind::Repeat(ref e, _)
        | ExprKind::DropTemps(ref e) => never_loop_expr(e, main_loop_id),
        ExprKind::Array(ref es) | ExprKind::MethodCall(_, _, ref es, _) | ExprKind::Tup(ref es) => {
            never_loop_expr_all(&mut es.iter(), main_loop_id)
        },
        ExprKind::Call(ref e, ref es) => never_loop_expr_all(&mut once(&**e).chain(es.iter()), main_loop_id),
        ExprKind::Binary(_, ref e1, ref e2)
        | ExprKind::Assign(ref e1, ref e2, _)
        | ExprKind::AssignOp(_, ref e1, ref e2)
        | ExprKind::Index(ref e1, ref e2) => never_loop_expr_all(&mut [&**e1, &**e2].iter().cloned(), main_loop_id),
        ExprKind::Loop(ref b, _, _, _) => {
            // Break can come from the inner loop so remove them.
            absorb_break(&never_loop_block(b, main_loop_id))
        },
        ExprKind::If(ref e, ref e2, ref e3) => {
            let e1 = never_loop_expr(e, main_loop_id);
            let e2 = never_loop_expr(e2, main_loop_id);
            let e3 = e3
                .as_ref()
                .map_or(NeverLoopResult::Otherwise, |e| never_loop_expr(e, main_loop_id));
            combine_seq(e1, combine_branches(e2, e3))
        },
        ExprKind::Match(ref e, ref arms, _) => {
            let e = never_loop_expr(e, main_loop_id);
            if arms.is_empty() {
                e
            } else {
                let arms = never_loop_expr_branch(&mut arms.iter().map(|a| &*a.body), main_loop_id);
                combine_seq(e, arms)
            }
        },
        ExprKind::Block(ref b, _) => never_loop_block(b, main_loop_id),
        ExprKind::Continue(d) => {
            let id = d
                .target_id
                .expect("target ID can only be missing in the presence of compilation errors");
            if id == main_loop_id {
                NeverLoopResult::MayContinueMainLoop
            } else {
                NeverLoopResult::AlwaysBreak
            }
        },
        ExprKind::Break(_, ref e) | ExprKind::Ret(ref e) => e.as_ref().map_or(NeverLoopResult::AlwaysBreak, |e| {
            combine_seq(never_loop_expr(e, main_loop_id), NeverLoopResult::AlwaysBreak)
        }),
        ExprKind::InlineAsm(ref asm) => asm
            .operands
            .iter()
            .map(|(o, _)| match o {
                InlineAsmOperand::In { expr, .. }
                | InlineAsmOperand::InOut { expr, .. }
                | InlineAsmOperand::Const { expr }
                | InlineAsmOperand::Sym { expr } => never_loop_expr(expr, main_loop_id),
                InlineAsmOperand::Out { expr, .. } => never_loop_expr_all(&mut expr.iter(), main_loop_id),
                InlineAsmOperand::SplitInOut { in_expr, out_expr, .. } => {
                    never_loop_expr_all(&mut once(in_expr).chain(out_expr.iter()), main_loop_id)
                },
            })
            .fold(NeverLoopResult::Otherwise, combine_both),
        ExprKind::Struct(_, _, None)
        | ExprKind::Yield(_, _)
        | ExprKind::Closure(_, _, _, _, _)
        | ExprKind::LlvmInlineAsm(_)
        | ExprKind::Path(_)
        | ExprKind::ConstBlock(_)
        | ExprKind::Lit(_)
        | ExprKind::Err => NeverLoopResult::Otherwise,
    }
}

fn never_loop_expr_all<'a, T: Iterator<Item = &'a Expr<'a>>>(es: &mut T, main_loop_id: HirId) -> NeverLoopResult {
    es.map(|e| never_loop_expr(e, main_loop_id))
        .fold(NeverLoopResult::Otherwise, combine_both)
}

fn never_loop_expr_branch<'a, T: Iterator<Item = &'a Expr<'a>>>(e: &mut T, main_loop_id: HirId) -> NeverLoopResult {
    e.map(|e| never_loop_expr(e, main_loop_id))
        .fold(NeverLoopResult::AlwaysBreak, combine_branches)
}
