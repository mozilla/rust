// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/// Run over the whole crate and check for ExprInlineAsm.
/// Inline asm isn't allowed on PNaCl, so we reject it here.

use session::Session;

use syntax::ast;
use syntax::visit::Visitor;
use syntax::visit;

pub fn check_crate(sess: &Session, krate: &ast::Crate) {
    if !sess.targeting_pnacl() { return; }

    visit::walk_crate(&mut CheckNoAsm { sess: sess, }, krate);
}

#[derive(Copy, Clone)]
struct CheckNoAsm<'a> {
    sess: &'a Session,
}

impl<'a, 'v> Visitor<'v> for CheckNoAsm<'a> {
    fn visit_expr(&mut self, e: &ast::Expr) {
        match e.node {
            ast::ExprInlineAsm(_) => self.sess.span_err(e.span,
                                                        "asm! is unsupported on this target"),
            _ => {},
        }
        visit::walk_expr(self, e)
    }
}
