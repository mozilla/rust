// Copyright 2012-2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use ast::{self, NestedMetaItem};
use ext::base::{ExtCtxt, SyntaxExtension};
use ext::build::AstBuilder;
use ast::Name;
use symbol::Symbol;
use syntax_pos::Span;
use codemap;

pub fn derive_attr_trait<'a>(cx: &mut ExtCtxt, attr: &'a ast::Attribute)
                             -> Option<&'a NestedMetaItem> {
    if attr.name() != "derive" {
        return None;
    }
    if attr.value_str().is_some() {
        cx.span_err(attr.span, "unexpected value in `derive`");
        return None;
    }

    let traits = attr.meta_item_list().unwrap_or(&[]);

    if traits.is_empty() {
        cx.span_warn(attr.span, "empty trait list in `derive`");
        return None;
    }

    return traits.get(0);
}

pub fn verify_derive_attrs(cx: &mut ExtCtxt, attrs: &[ast::Attribute]) {
    for attr in attrs {
        if attr.name() != "derive" {
            continue;
        }

        if attr.value_str().is_some() {
            cx.span_err(attr.span, "unexpected value in `derive`");
        }

        let traits = attr.meta_item_list().unwrap_or(&[]).to_owned();

        if traits.is_empty() {
            cx.span_warn(attr.span, "empty trait list in `derive`");
            continue;
        }
        for titem in traits {
            if titem.word().is_none() {
                cx.span_err(titem.span, "malformed `derive` entry");
            }
        }
    }
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum DeriveType {
    Legacy,
    ProcMacro,
    Builtin
}

impl DeriveType {
    // Classify a derive trait name by resolving the macro.
    pub fn classify(cx: &mut ExtCtxt, tname: Name) -> Option<DeriveType> {
        let legacy_derive_name = Symbol::intern(&format!("derive_{}", tname));

        if let Ok(_) = cx.resolver.resolve_builtin_macro(legacy_derive_name) {
            return Some(DeriveType::Legacy);
        }

        match cx.resolver.resolve_builtin_macro(tname) {
            Ok(ext) => match *ext {
                SyntaxExtension::BuiltinDerive(..) => Some(DeriveType::Builtin),
                SyntaxExtension::ProcMacroDerive(..) => Some(DeriveType::ProcMacro),
                _ => None,
            },
            Err(_) => Some(DeriveType::ProcMacro),
        }
    }
}

pub fn get_derive_attr(cx: &mut ExtCtxt, attrs: &mut Vec<ast::Attribute>,
                       derive_type: DeriveType) -> Option<ast::Attribute> {
    for i in 0..attrs.len() {
        if attrs[i].name() != "derive" {
            continue;
        }

        if attrs[i].value_str().is_some() {
            continue;
        }

        let mut traits = attrs[i].meta_item_list().unwrap_or(&[]).to_owned();

        // First, weed out malformed #[derive]
        traits.retain(|titem| titem.word().is_some());

        let mut titem = None;

        // See if we can find a matching trait.
        for j in 0..traits.len() {
            let tname = if let Some(tname) = traits[j].name() {
                tname
            } else {
                continue;
            };

            if DeriveType::classify(cx, tname) == Some(derive_type) {
                titem = Some(traits.remove(j));
                break;
            }
        }

        // If we find a trait, remove the trait from the attribute.
        if let Some(titem) = titem {
            if traits.len() == 0 {
                attrs.remove(i);
            } else {
                let derive = Symbol::intern("derive");
                let mitem = cx.meta_list(titem.span, derive, traits);
                attrs[i] = cx.attribute(titem.span, mitem);
            }
            let derive = Symbol::intern("derive");
            let mitem = cx.meta_list(titem.span, derive, vec![titem]);
            return Some(cx.attribute(mitem.span, mitem));
        }
    }
    return None;
}

fn allow_unstable(cx: &mut ExtCtxt, span: Span, attr_name: &str) -> Span {
    Span {
        expn_id: cx.codemap().record_expansion(codemap::ExpnInfo {
            call_site: span,
            callee: codemap::NameAndSpan {
                format: codemap::MacroAttribute(Symbol::intern(attr_name)),
                span: Some(span),
                allow_internal_unstable: true,
            },
        }),
        ..span
    }
}

pub fn add_derived_markers(cx: &mut ExtCtxt, attrs: &mut Vec<ast::Attribute>) {
    if attrs.is_empty() {
        return;
    }

    let titems = attrs.iter().filter(|a| {
        a.name() == "derive"
    }).flat_map(|a| {
        a.meta_item_list().unwrap_or(&[]).iter()
    }).filter_map(|titem| {
        titem.name()
    }).collect::<Vec<_>>();

    let span = attrs[0].span;

    if !attrs.iter().any(|a| a.name() == "structural_match") &&
        titems.iter().any(|t| *t == "PartialEq") && titems.iter().any(|t| *t == "Eq") {
        let structural_match = Symbol::intern("structural_match");
        let span = allow_unstable(cx, span, "derive(PartialEq, Eq)");
        let meta = cx.meta_word(span, structural_match);
        attrs.push(cx.attribute(span, meta));
    }

    if !attrs.iter().any(|a| a.name() == "rustc_copy_clone_marker") &&
        titems.iter().any(|t| *t == "Copy") && titems.iter().any(|t| *t == "Clone") {
        let structural_match = Symbol::intern("structural_match");
        let span = allow_unstable(cx, span, "derive(Copy, Clone)");
        let meta = cx.meta_word(span, structural_match);
        attrs.push(cx.attribute(span, meta));
    }
}
