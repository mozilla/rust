//! Clippy wrappers around rustc's diagnostic functions.

use rustc_errors::{Applicability, CodeSuggestion, DiagnosticBuilder, Substitution, SubstitutionPart, SuggestionStyle};
use rustc_hir::HirId;
use rustc_lint::{LateContext, Lint, LintContext};
use rustc_span::source_map::{MultiSpan, Span};
use std::env;

fn docs_link(db: &mut DiagnosticBuilder<'_>, lint: &'static Lint) {
    if env::var("CLIPPY_DISABLE_DOCS_LINKS").is_err() {
        db.help(&format!(
            "for further information visit https://rust-lang.github.io/rust-clippy/{}/index.html#{}",
            &option_env!("RUST_RELEASE_NUM").map_or("master".to_string(), |n| {
                // extract just major + minor version and ignore patch versions
                format!("rust-{}", n.rsplitn(2, '.').nth(1).unwrap())
            }),
            lint.name_lower().replacen("clippy::", "", 1)
        ));
    }
}

/// Emit a basic lint message with a `msg` and a `span`.
///
/// This is the most primitive of our lint emission methods and can
/// be a good way to get a new lint started.
///
/// Usually it's nicer to provide more context for lint messages.
/// Be sure the output is understandable when you use this method.
///
/// # Example
///
/// ```ignore
/// error: usage of mem::forget on Drop type
///   --> $DIR/mem_forget.rs:17:5
///    |
/// 17 |     std::mem::forget(seven);
///    |     ^^^^^^^^^^^^^^^^^^^^^^^
/// ```
pub fn span_lint<T: LintContext>(cx: &T, lint: &'static Lint, sp: impl Into<MultiSpan>, msg: &str) {
    cx.struct_span_lint(lint, sp, |ldb| {
        let mut db = ldb.build(msg);
        docs_link(&mut db, lint);
        db.emit();
    });
}

/// Same as `span_lint` but with an extra `help` message.
///
/// Use this if you want to provide some general help but
/// can't provide a specific machine applicable suggestion.
///
/// The `help` message is not attached to any `Span`.
///
/// # Example
///
/// ```ignore
/// error: constant division of 0.0 with 0.0 will always result in NaN
///   --> $DIR/zero_div_zero.rs:6:25
///    |
/// 6  |     let other_f64_nan = 0.0f64 / 0.0;
///    |                         ^^^^^^^^^^^^
///    |
///    = help: Consider using `std::f64::NAN` if you would like a constant representing NaN
/// ```
pub fn span_lint_and_help<'a, T: LintContext>(cx: &'a T, lint: &'static Lint, span: Span, msg: &str, help: &str) {
    cx.struct_span_lint(lint, span, |ldb| {
        let mut db = ldb.build(msg);
        db.help(help);
        docs_link(&mut db, lint);
        db.emit();
    });
}

/// Like `span_lint` but with a `note` section instead of a `help` message.
///
/// The `note` message is presented separately from the main lint message
/// and is attached to a specific span:
///
/// # Example
///
/// ```ignore
/// error: calls to `std::mem::forget` with a reference instead of an owned value. Forgetting a reference does nothing.
///   --> $DIR/drop_forget_ref.rs:10:5
///    |
/// 10 |     forget(&SomeStruct);
///    |     ^^^^^^^^^^^^^^^^^^^
///    |
///    = note: `-D clippy::forget-ref` implied by `-D warnings`
/// note: argument has type &SomeStruct
///   --> $DIR/drop_forget_ref.rs:10:12
///    |
/// 10 |     forget(&SomeStruct);
///    |            ^^^^^^^^^^^
/// ```
pub fn span_lint_and_note<'a, T: LintContext>(
    cx: &'a T,
    lint: &'static Lint,
    span: Span,
    msg: &str,
    note_span: Span,
    note: &str,
) {
    cx.struct_span_lint(lint, span, |ldb| {
        let mut db = ldb.build(msg);
        if note_span == span {
            db.note(note);
        } else {
            db.span_note(note_span, note);
        }
        docs_link(&mut db, lint);
        db.emit();
    });
}

pub fn span_lint_and_then<'a, T: LintContext, F>(cx: &'a T, lint: &'static Lint, sp: Span, msg: &str, f: F)
where
    F: for<'b> FnOnce(&mut DiagnosticBuilder<'b>),
{
    cx.struct_span_lint(lint, sp, |ldb| {
        let mut db = ldb.build(msg);
        f(&mut db);
        docs_link(&mut db, lint);
        db.emit();
    });
}

pub fn span_lint_hir(cx: &LateContext<'_, '_>, lint: &'static Lint, hir_id: HirId, sp: Span, msg: &str) {
    cx.tcx.struct_span_lint_hir(lint, hir_id, sp, |ldb| {
        let mut db = ldb.build(msg);
        docs_link(&mut db, lint);
        db.emit();
    });
}

pub fn span_lint_hir_and_then(
    cx: &LateContext<'_, '_>,
    lint: &'static Lint,
    hir_id: HirId,
    sp: Span,
    msg: &str,
    f: impl FnOnce(&mut DiagnosticBuilder<'_>),
) {
    cx.tcx.struct_span_lint_hir(lint, hir_id, sp, |ldb| {
        let mut db = ldb.build(msg);
        f(&mut db);
        docs_link(&mut db, lint);
        db.emit();
    });
}

/// Add a span lint with a suggestion on how to fix it.
///
/// These suggestions can be parsed by rustfix to allow it to automatically fix your code.
/// In the example below, `help` is `"try"` and `sugg` is the suggested replacement `".any(|x| x >
/// 2)"`.
///
/// ```ignore
/// error: This `.fold` can be more succinctly expressed as `.any`
/// --> $DIR/methods.rs:390:13
///     |
/// 390 |     let _ = (0..3).fold(false, |acc, x| acc || x > 2);
///     |                   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ help: try: `.any(|x| x > 2)`
///     |
///     = note: `-D fold-any` implied by `-D warnings`
/// ```
pub fn span_lint_and_sugg<'a, T: LintContext>(
    cx: &'a T,
    lint: &'static Lint,
    sp: Span,
    msg: &str,
    help: &str,
    sugg: String,
    applicability: Applicability,
) {
    span_lint_and_then(cx, lint, sp, msg, |db| {
        db.span_suggestion(sp, help, sugg, applicability);
    });
}

/// Create a suggestion made from several `span → replacement`.
///
/// Note: in the JSON format (used by `compiletest_rs`), the help message will
/// appear once per
/// replacement. In human-readable format though, it only appears once before
/// the whole suggestion.
pub fn multispan_sugg<I>(db: &mut DiagnosticBuilder<'_>, help_msg: String, sugg: I)
where
    I: IntoIterator<Item = (Span, String)>,
{
    let sugg = CodeSuggestion {
        substitutions: vec![Substitution {
            parts: sugg
                .into_iter()
                .map(|(span, snippet)| SubstitutionPart { snippet, span })
                .collect(),
        }],
        msg: help_msg,
        style: SuggestionStyle::ShowCode,
        applicability: Applicability::Unspecified,
    };
    db.suggestions.push(sugg);
}
