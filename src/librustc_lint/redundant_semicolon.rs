use crate::{EarlyContext, EarlyLintPass, LintContext};
use rustc_ast::ast::{Block, StmtKind};
use rustc_errors::Applicability;
use rustc_span::Span;

declare_lint! {
    pub REDUNDANT_SEMICOLONS,
    Warn,
    "detects unnecessary trailing semicolons"
}

declare_lint_pass!(RedundantSemicolons => [REDUNDANT_SEMICOLONS]);

impl EarlyLintPass for RedundantSemicolons {
    fn check_block(&mut self, cx: &EarlyContext<'_>, block: &Block) {
        let mut seq = None;
        for stmt in block.stmts.iter() {
            match (&stmt.kind, &mut seq) {
                (StmtKind::Empty, None) => seq = Some((stmt.span, false)),
                (StmtKind::Empty, Some(seq)) => *seq = (seq.0.to(stmt.span), true),
                (_, seq) => maybe_lint_redundant_semis(cx, seq),
            }
        }
        maybe_lint_redundant_semis(cx, &mut seq);
    }
}

fn maybe_lint_redundant_semis(cx: &EarlyContext<'_>, seq: &mut Option<(Span, bool)>) {
    if let Some((span, multiple)) = seq.take() {
        cx.struct_span_lint(REDUNDANT_SEMICOLONS, span, |lint| {
            let (msg, rem) = if multiple {
                ("unnecessary trailing semicolons", "remove these semicolons")
            } else {
                ("unnecessary trailing semicolon", "remove this semicolon")
            };
            lint.build(msg)
                .span_suggestion(span, rem, String::new(), Applicability::MaybeIncorrect)
                .emit();
        });
    }
}
