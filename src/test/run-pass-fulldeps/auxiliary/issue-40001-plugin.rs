#![feature(box_syntax, plugin, plugin_registrar, rustc_private)]
#![crate_type = "dylib"]

#[macro_use]
extern crate rustc;
extern crate rustc_plugin;
extern crate syntax;

use rustc_plugin::Registry;
use rustc::hir;
use rustc::hir::intravisit;
use hir::Node;
use rustc::lint::{LateContext, LintPass, LintArray, LateLintPass, LintContext};
use syntax::{ast, source_map};

#[plugin_registrar]
pub fn plugin_registrar(reg: &mut Registry) {
    reg.register_late_lint_pass(box MissingWhitelistedAttrPass);
}

declare_lint!(MISSING_WHITELISTED_ATTR, Deny,
              "Checks for missing `whitelisted_attr` attribute");

struct MissingWhitelistedAttrPass;

impl LintPass for MissingWhitelistedAttrPass {
    fn name(&self) -> &'static str {
        "MissingWhitelistedAttrPass"
    }

    fn get_lints(&self) -> LintArray {
        lint_array!(MISSING_WHITELISTED_ATTR)
    }
}

impl<'a, 'tcx> LateLintPass<'a, 'tcx> for MissingWhitelistedAttrPass {
    fn check_fn(&mut self,
                cx: &LateContext<'a, 'tcx>,
                _: intravisit::FnKind<'tcx>,
                _: &'tcx hir::FnDecl,
                _: &'tcx hir::Body,
                span: source_map::Span,
                id: ast::NodeId) {

        let item = match cx.tcx.hir().get(id) {
            Node::Item(item) => item,
            _ => cx.tcx.hir().expect_item(cx.tcx.hir().get_parent(id)),
        };

        if item.attrs.iter().all(|attr| {
            attr.path.segments.last().map(|seg| seg.ident.name.to_string()) !=
            Some("whitelisted_attr".to_owned())
        }) {
            cx.span_lint(MISSING_WHITELISTED_ATTR, span,
                         "Missing 'whitelisted_attr' attribute");
        }
    }
}
