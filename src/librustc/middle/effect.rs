// Copyright 2012-2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Enforces the Rust effect system. Currently there is just one effect,
/// `unsafe`.

use middle::ty::{ty_bare_fn, ty_closure, ty_ptr};
use middle::ty;
use middle::typeck::method_map;
use util::ppaux;

use syntax::ast::{deref, expr_call, expr_inline_asm, expr_method_call};
use syntax::ast::{expr_unary, unsafe_fn, expr_path};
use syntax::ast;
use syntax::codemap::span;
use syntax::visit::{fk_item_fn, fk_method, Visitor};
use syntax::visit;

#[deriving(Eq)]
enum UnsafeContext {
    SafeContext,
    UnsafeFn,
    UnsafeBlock(ast::NodeId),
}

struct Context {
    /// The type context.
    type_context: ty::ctxt,
    /// The method map.
    method_map: method_map,
    /// Whether we're in an unsafe context.
    unsafe_context: UnsafeContext,
}

impl Visitor<()> for Context {
    fn visit_fn(&mut self,
                fn_kind: &visit::fn_kind,
                fn_decl: &ast::fn_decl,
                block: &ast::Block,
                span: span,
                node_id: ast::NodeId,
                _: ()) {
        let (is_item_fn, is_unsafe_fn) = match *fn_kind {
            fk_item_fn(_, _, purity, _) => (true, purity == unsafe_fn),
            fk_method(_, _, method) => (true, method.purity == unsafe_fn),
            _ => (false, false),
        };

        let old_unsafe_context = self.unsafe_context;
        if is_unsafe_fn {
            self.unsafe_context = UnsafeFn
        } else if is_item_fn {
            self.unsafe_context = SafeContext
        }

        visit::walk_fn(self, fn_kind, fn_decl, block, span, node_id, ());

        self.unsafe_context = old_unsafe_context
    }

    fn visit_block(&mut self, block: &ast::Block, _: ()) {
        let old_unsafe_context = self.unsafe_context;
        if block.rules == ast::UnsafeBlock &&
                self.unsafe_context == SafeContext {
            self.unsafe_context = UnsafeBlock(block.id)
        }

        visit::walk_block(self, block, ());

        self.unsafe_context = old_unsafe_context
    }

    fn visit_expr(&mut self, expr: @ast::expr, _: ()) {
        match expr.node {
            expr_method_call(callee_id, _, _, _, _, _) => {
                let base_type = ty::node_id_to_type(self.type_context,
                                                    callee_id);
                debug!("effect: method call case, base type is %s",
                       ppaux::ty_to_str(self.type_context, base_type));
                if type_is_unsafe_function(base_type) {
                    self.require_unsafe(expr.span,
                                        "invocation of unsafe method")
                }
            }
            expr_call(base, _, _) => {
                let base_type = ty::node_id_to_type(self.type_context,
                                                    base.id);
                debug!("effect: call case, base type is %s",
                       ppaux::ty_to_str(self.type_context, base_type));
                if type_is_unsafe_function(base_type) {
                    self.require_unsafe(expr.span, "call to unsafe function")
                }
            }
            expr_unary(_, deref, base) => {
                let base_type = ty::node_id_to_type(self.type_context,
                                                    base.id);
                debug!("effect: unary case, base type is %s",
                       ppaux::ty_to_str(self.type_context, base_type));
                match ty::get(base_type).sty {
                    ty_ptr(_) => {
                        self.require_unsafe(expr.span,
                                            "dereference of unsafe pointer")
                    }
                    _ => {}
                }
            }
            expr_inline_asm(*) => {
                self.require_unsafe(expr.span, "use of inline assembly")
            }
            expr_path(*) => {
                match ty::resolve_expr(self.type_context, expr) {
                    ast::def_static(_, true) => {
                        self.require_unsafe(expr.span,
                                            "use of mutable static")
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        visit::walk_expr(self, expr, ());
    }
}

impl Context {
    fn require_unsafe(&mut self, span: span, description: &str) {
        match self.unsafe_context {
            SafeContext => {
                // Report an error.
                self.type_context.sess.span_err(span,
                                  fmt!("%s requires unsafe function or block",
                                       description))
            }
            UnsafeBlock(block_id) => {
                // OK, but record this.
                debug!("effect: recording unsafe block as used: %?", block_id);
                let _ = self.type_context.used_unsafe.insert(block_id);
            }
            UnsafeFn => {}
        }
    }
}

fn type_is_unsafe_function(ty: ty::t) -> bool {
    match ty::get(ty).sty {
        ty_bare_fn(ref f) => f.purity == unsafe_fn,
        ty_closure(ref f) => f.purity == unsafe_fn,
        _ => false,
    }
}

pub fn check_crate(tcx: ty::ctxt,
                   method_map: method_map,
                   crate: &ast::Crate) {
    let mut context = Context {
        type_context: tcx,
        method_map: method_map,
        unsafe_context: SafeContext,
    };
    visit::walk_crate(&mut context, crate, ());
}
