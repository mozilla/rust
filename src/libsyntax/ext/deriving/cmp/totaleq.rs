// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use ast::{MetaItem, item, expr};
use codemap::span;
use ext::base::ExtCtxt;
use ext::build::AstBuilder;
use ext::deriving::generic::*;
use ext::deriving::DerivingOptions;
use ext::deriving::cmp::CmpOptions;

pub fn expand_deriving_totaleq(cx: @ExtCtxt,
                               span: span,
                               options: DerivingOptions,
                               mitem: @MetaItem,
                               in_items: ~[@item]) -> ~[@item] {
    let mut options = match CmpOptions::parse(cx, span, "TotalEq", options, false, false) {
        Some(o) => o,
        None => return in_items
    };

    fn cs_equals(cx: @ExtCtxt, span: span, substr: &Substructure) -> @expr {
        cs_and(|cx, span, _, _| cx.expr_bool(span, false),
               cx, span, substr)
    }

    let trait_def = TraitDef {
        path: Path::new(~["std", "cmp", "TotalEq"]),
        additional_bounds: ~[],
        generics: LifetimeBounds::empty(),
        methods: ~[
            MethodDef {
                name: "equals",
                generics: LifetimeBounds::empty(),
                explicit_self: borrowed_explicit_self(),
                args: ~[borrowed_self()],
                ret_ty: Literal(Path::new(~["bool"])),
                const_nonmatching: true,
                combine_substructure: |cx, span, s| {
                    options.call_substructure(cx, span, s, cs_equals)
                }
            }
        ]
    };
    trait_def.expand(cx, span, mitem, in_items)
}
