// Copyright 2012-2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use core::prelude::*;

use ast;
use ast::{TraitTyParamBound, Ty, and, bind_by_ref, binop, deref, enum_def};
use ast::{enum_variant_kind, expr, expr_match, ident, impure_fn, item, item_};
use ast::{item_enum, item_impl, item_struct, Generics};
use ast::{m_imm, meta_item, method};
use ast::{named_field, or, pat, pat_ident, pat_wild, public, pure_fn};
use ast::{stmt, struct_def, struct_variant_kind};
use ast::{sty_by_ref, sty_region, tuple_variant_kind, ty_nil, TyParam};
use ast::{TyParamBound, ty_path, ty_rptr, unnamed_field, variant};
use ext::base::ext_ctxt;
use ext::build;
use codemap::{span, spanned};
use ast_util;

use core::uint;

pub fn expand_deriving_iter_bytes(cx: @ext_ctxt,
                                  span: span,
                                  _mitem: @meta_item,
                                  in_items: ~[@item])
                               -> ~[@item] {
    super::expand_deriving(cx,
                    span,
                    in_items,
                    expand_deriving_iter_bytes_struct_def,
                    expand_deriving_iter_bytes_enum_def)
}

fn create_derived_iter_bytes_impl(cx: @ext_ctxt,
                                  span: span,
                                  type_ident: ident,
                                  generics: &Generics,
                                  method: @method)
                               -> @item {
    let methods = [ method ];
    let trait_path = [
        cx.ident_of(~"core"),
        cx.ident_of(~"to_bytes"),
        cx.ident_of(~"IterBytes")
    ];
    super::create_derived_impl(cx, span, type_ident, generics, methods, trait_path)
}

// Creates a method from the given set of statements conforming to the
// signature of the `iter_bytes` method.
fn create_iter_bytes_method(cx: @ext_ctxt,
                            span: span,
                            +statements: ~[@stmt])
                         -> @method {
    // Create the `lsb0` parameter.
    let bool_ident = cx.ident_of(~"bool");
    let lsb0_arg_type = build::mk_simple_ty_path(cx, span, bool_ident);
    let lsb0_ident = cx.ident_of(~"__lsb0");
    let lsb0_arg = build::mk_arg(cx, span, lsb0_ident, lsb0_arg_type);

    // Create the `f` parameter.
    let core_ident = cx.ident_of(~"core");
    let to_bytes_ident = cx.ident_of(~"to_bytes");
    let cb_ident = cx.ident_of(~"Cb");
    let core_to_bytes_cb_ident = ~[ core_ident, to_bytes_ident, cb_ident ];
    let f_arg_type = build::mk_ty_path(cx, span, core_to_bytes_cb_ident);
    let f_ident = cx.ident_of(~"__f");
    let f_arg = build::mk_arg(cx, span, f_ident, f_arg_type);

    // Create the type of the return value.
    let output_type = @ast::Ty { id: cx.next_id(), node: ty_nil, span: span };

    // Create the function declaration.
    let inputs = ~[ lsb0_arg, f_arg ];
    let fn_decl = build::mk_fn_decl(inputs, output_type);

    // Create the body block.
    let body_block = build::mk_block_(cx, span, statements);

    // Create the method.
    let self_ty = spanned { node: sty_region(m_imm), span: span };
    let method_ident = cx.ident_of(~"iter_bytes");
    @ast::method {
        ident: method_ident,
        attrs: ~[],
        generics: ast_util::empty_generics(),
        self_ty: self_ty,
        purity: pure_fn,
        decl: fn_decl,
        body: body_block,
        id: cx.next_id(),
        span: span,
        self_id: cx.next_id(),
        vis: public
    }
}

fn call_substructure_iter_bytes_method(cx: @ext_ctxt,
                                       span: span,
                                       self_field: @expr)
                                    -> @stmt {
    // Gather up the parameters we want to chain along.
    let lsb0_ident = cx.ident_of(~"__lsb0");
    let f_ident = cx.ident_of(~"__f");
    let lsb0_expr = build::mk_path(cx, span, ~[ lsb0_ident ]);
    let f_expr = build::mk_path(cx, span, ~[ f_ident ]);

    // Call the substructure method.
    let iter_bytes_ident = cx.ident_of(~"iter_bytes");
    let self_method = build::mk_access_(cx,
                                        span,
                                        self_field,
                                        iter_bytes_ident);
    let self_call = build::mk_call_(cx,
                                    span,
                                    self_method,
                                    ~[ lsb0_expr, f_expr ]);

    // Create a statement out of this expression.
    build::mk_stmt(cx, span, self_call)
}

fn expand_deriving_iter_bytes_struct_def(cx: @ext_ctxt,
                                         span: span,
                                         struct_def: &struct_def,
                                         type_ident: ident,
                                         generics: &Generics)
                                      -> @item {
    // Create the method.
    let method = expand_deriving_iter_bytes_struct_method(cx,
                                                          span,
                                                          struct_def);

    // Create the implementation.
    return create_derived_iter_bytes_impl(cx,
                                          span,
                                          type_ident,
                                          generics,
                                          method);
}

fn expand_deriving_iter_bytes_enum_def(cx: @ext_ctxt,
                                       span: span,
                                       enum_definition: &enum_def,
                                       type_ident: ident,
                                       generics: &Generics)
                                    -> @item {
    // Create the method.
    let method = expand_deriving_iter_bytes_enum_method(cx,
                                                        span,
                                                        enum_definition);

    // Create the implementation.
    return create_derived_iter_bytes_impl(cx,
                                          span,
                                          type_ident,
                                          generics,
                                          method);
}

fn expand_deriving_iter_bytes_struct_method(cx: @ext_ctxt,
                                            span: span,
                                            struct_def: &struct_def)
                                         -> @method {
    let self_ident = cx.ident_of(~"self");

    // Create the body of the method.
    let mut statements = ~[];
    for struct_def.fields.each |struct_field| {
        match struct_field.node.kind {
            named_field(ident, _, _) => {
                // Create the accessor for this field.
                let self_field = build::mk_access(cx,
                                                  span,
                                                  ~[ self_ident ],
                                                  ident);

                // Call the substructure method.
                let stmt = call_substructure_iter_bytes_method(cx,
                                                               span,
                                                               self_field);
                statements.push(stmt);
            }
            unnamed_field => {
                cx.span_unimpl(span,
                               ~"unnamed fields with `deriving_iter_bytes`");
            }
        }
    }

    // Create the method itself.
    return create_iter_bytes_method(cx, span, statements);
}

fn expand_deriving_iter_bytes_enum_method(cx: @ext_ctxt,
                                          span: span,
                                          enum_definition: &enum_def)
                                       -> @method {
    // Create the arms of the match in the method body.
    let arms = do enum_definition.variants.mapi |i, variant| {
        // Create the matching pattern.
        let pat = super::create_enum_variant_pattern(cx, span, variant, ~"__self");

        // Determine the discriminant. We will feed this value to the byte
        // iteration function.
        let discriminant;
        match variant.node.disr_expr {
            Some(copy disr_expr) => discriminant = disr_expr,
            None => discriminant = build::mk_uint(cx, span, i),
        }

        // Feed the discriminant to the byte iteration function.
        let mut stmts = ~[];
        let discrim_stmt = call_substructure_iter_bytes_method(cx,
                                                               span,
                                                               discriminant);
        stmts.push(discrim_stmt);

        // Feed each argument in this variant to the byte iteration function
        // as well.
        for uint::range(0, super::variant_arg_count(cx, span, variant)) |j| {
            // Create the expression for this field.
            let field_ident = cx.ident_of(~"__self" + j.to_str());
            let field = build::mk_path(cx, span, ~[ field_ident ]);

            // Call the substructure method.
            let stmt = call_substructure_iter_bytes_method(cx, span, field);
            stmts.push(stmt);
        }

        // Create the pattern body.
        let match_body_block = build::mk_block_(cx, span, stmts);

        // Create the arm.
        ast::arm {
            pats: ~[ pat ],
            guard: None,
            body: match_body_block,
        }
    };

    // Create the method body.
    let self_match_expr = super::expand_enum_or_struct_match(cx, span, arms);
    let self_match_stmt = build::mk_stmt(cx, span, self_match_expr);

    // Create the method.
    create_iter_bytes_method(cx, span, ~[ self_match_stmt ])
}
