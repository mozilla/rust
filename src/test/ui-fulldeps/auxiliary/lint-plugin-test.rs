// force-host

#![feature(plugin_registrar)]
#![feature(box_syntax, rustc_private)]

extern crate rustc_ast;

// Load rustc as a plugin to get macros
extern crate rustc_driver;
#[macro_use]
extern crate rustc_lint;
#[macro_use]
extern crate rustc_session;

use rustc_driver::plugin::Registry;
use rustc_lint::{EarlyContext, EarlyLintPass, LintArray, LintContext, LintPass};
use rustc_ast as ast;
declare_lint!(TEST_LINT, Warn, "Warn about items named 'lintme'");

declare_lint_pass!(Pass => [TEST_LINT]);

impl EarlyLintPass for Pass {
    fn check_item(&mut self, cx: &EarlyContext, it: &ast::Item) {
        if it.ident.name.as_str() == "lintme" {
            if let Some(lint) = cx.lookup_lint(TEST_LINT) {
                lint.build("item is named 'lintme'").set_span(it.span).emit()
            }
        }
    }
}

#[plugin_registrar]
pub fn plugin_registrar(reg: &mut Registry) {
    reg.lint_store.register_lints(&[&TEST_LINT]);
    reg.lint_store.register_early_pass(|| box Pass);
}
