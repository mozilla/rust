use rustc_data_structures::fx::FxHashMap;
use rustc_lint::LintStore;
use rustc_lint_defs::{declare_tool_lint, Lint, LintId};
use rustc_session::{lint, Session};

use std::lazy::SyncLazy as Lazy;

/// This function is used to setup the lint initialization. By default, in rustdoc, everything
/// is "allowed". Depending if we run in test mode or not, we want some of them to be at their
/// default level. For example, the "INVALID_CODEBLOCK_ATTRIBUTES" lint is activated in both
/// modes.
///
/// A little detail easy to forget is that there is a way to set the lint level for all lints
/// through the "WARNINGS" lint. To prevent this to happen, we set it back to its "normal" level
/// inside this function.
///
/// It returns a tuple containing:
///  * Vector of tuples of lints' name and their associated "max" level
///  * HashMap of lint id with their associated "max" level
pub(crate) fn init_lints<F>(
    mut allowed_lints: Vec<String>,
    lint_opts: Vec<(String, lint::Level)>,
    filter_call: F,
) -> (Vec<(String, lint::Level)>, FxHashMap<lint::LintId, lint::Level>)
where
    F: Fn(&lint::Lint) -> Option<(String, lint::Level)>,
{
    let warnings_lint_name = lint::builtin::WARNINGS.name;

    allowed_lints.push(warnings_lint_name.to_owned());
    allowed_lints.extend(lint_opts.iter().map(|(lint, _)| lint).cloned());

    let lints = || {
        lint::builtin::HardwiredLints::get_lints()
            .into_iter()
            .chain(rustc_lint::SoftLints::get_lints().into_iter())
    };

    let lint_opts = lints()
        .filter_map(|lint| {
            // Permit feature-gated lints to avoid feature errors when trying to
            // allow all lints.
            if lint.feature_gate.is_some() || allowed_lints.iter().any(|l| lint.name == l) {
                None
            } else {
                filter_call(lint)
            }
        })
        .chain(lint_opts.into_iter())
        .collect::<Vec<_>>();

    let lint_caps = lints()
        .filter_map(|lint| {
            // We don't want to allow *all* lints so let's ignore
            // those ones.
            if allowed_lints.iter().any(|l| lint.name == l) {
                None
            } else {
                Some((lint::LintId::of(lint), lint::Allow))
            }
        })
        .collect();
    (lint_opts, lint_caps)
}

declare_tool_lint! {
    /// The `broken_intra_doc_links` lint detects failures in resolving
    /// intra-doc link targets. This is a `rustdoc` only lint, see the
    /// documentation in the [rustdoc book].
    ///
    /// [rustdoc book]: ../../../rustdoc/lints.html#broken_intra_doc_links
    pub rustdoc::BROKEN_INTRA_DOC_LINKS,
    Warn,
    "failures in resolving intra-doc link targets"
}

declare_tool_lint! {
    /// This is a subset of `broken_intra_doc_links` that warns when linking from
    /// a public item to a private one. This is a `rustdoc` only lint, see the
    /// documentation in the [rustdoc book].
    ///
    /// [rustdoc book]: ../../../rustdoc/lints.html#private_intra_doc_links
    pub rustdoc::PRIVATE_INTRA_DOC_LINKS,
    Warn,
    "linking from a public item to a private one"
}

declare_tool_lint! {
    /// The `invalid_codeblock_attributes` lint detects code block attributes
    /// in documentation examples that have potentially mis-typed values. This
    /// is a `rustdoc` only lint, see the documentation in the [rustdoc book].
    ///
    /// [rustdoc book]: ../../../rustdoc/lints.html#invalid_codeblock_attributes
    pub rustdoc::INVALID_CODEBLOCK_ATTRIBUTES,
    Warn,
    "codeblock attribute looks a lot like a known one"
}

declare_tool_lint! {
    /// The `missing_doc_code_examples` lint detects publicly-exported items
    /// without code samples in their documentation. This is a `rustdoc` only
    /// lint, see the documentation in the [rustdoc book].
    ///
    /// [rustdoc book]: ../../../rustdoc/lints.html#missing_doc_code_examples
    pub rustdoc::MISSING_DOC_CODE_EXAMPLES,
    Allow,
    "detects publicly-exported items without code samples in their documentation"
}

declare_tool_lint! {
    /// The `private_doc_tests` lint detects code samples in docs of private
    /// items not documented by `rustdoc`. This is a `rustdoc` only lint, see
    /// the documentation in the [rustdoc book].
    ///
    /// [rustdoc book]: ../../../rustdoc/lints.html#private_doc_tests
    pub rustdoc::PRIVATE_DOC_TESTS,
    Allow,
    "detects code samples in docs of private items not documented by rustdoc"
}

declare_tool_lint! {
    /// The `invalid_html_tags` lint detects invalid HTML tags. This is a
    /// `rustdoc` only lint, see the documentation in the [rustdoc book].
    ///
    /// [rustdoc book]: ../../../rustdoc/lints.html#invalid_html_tags
    pub rustdoc::INVALID_HTML_TAGS,
    Allow,
    "detects invalid HTML tags in doc comments"
}

declare_tool_lint! {
    /// The `non_autolinks` lint detects when a URL could be written using
    /// only angle brackets. This is a `rustdoc` only lint, see the
    /// documentation in the [rustdoc book].
    ///
    /// [rustdoc book]: ../../../rustdoc/lints.html#non_autolinks
    pub rustdoc::NON_AUTOLINKS,
    Warn,
    "detects URLs that could be written using only angle brackets"
}

crate static RUSTDOC_LINTS: Lazy<Vec<&'static Lint>> = Lazy::new(|| {
    vec![
        BROKEN_INTRA_DOC_LINKS,
        PRIVATE_INTRA_DOC_LINKS,
        MISSING_DOC_CODE_EXAMPLES,
        PRIVATE_DOC_TESTS,
        INVALID_CODEBLOCK_ATTRIBUTES,
        INVALID_HTML_TAGS,
        NON_AUTOLINKS,
    ]
});

crate fn register_lints(_sess: &Session, lint_store: &mut LintStore) {
    lint_store.register_lints(&**RUSTDOC_LINTS);
    lint_store.register_group(
        true,
        "rustdoc",
        None,
        RUSTDOC_LINTS.iter().map(|&lint| LintId::of(lint)).collect(),
    );
    lint_store
        .register_renamed("intra_doc_link_resolution_failure", "rustdoc::broken_intra_doc_links");
}
