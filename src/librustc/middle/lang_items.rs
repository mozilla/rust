// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Detecting language items.
//
// Language items are items that represent concepts intrinsic to the language
// itself. Examples are:
//
// * Traits that specify "kinds"; e.g. "Freeze", "Send".
//
// * Traits that represent operators; e.g. "Add", "Sub", "Index".
//
// * Functions called by the compiler itself.


use driver::session::Session;
use metadata::csearch::each_lang_item;
use middle::ty::{BuiltinBound, BoundFreeze, BoundPod, BoundSend, BoundSized};
use syntax::ast;
use syntax::ast_util::local_def;
use syntax::attr::AttrMetaMethods;
use syntax::visit;
use syntax::visit::Visitor;

use std::hashmap::HashMap;
use std::iter::Enumerate;
use std::vec;

// The actual lang items defined come at the end of this file in one handy table.
// So you probably just want to nip down to the end.
macro_rules! lets_do_this {
    (
        $( $variant:ident, $name:expr, $method:ident; )*
    ) => {

#[deriving(FromPrimitive)]
pub enum LangItem {
    $($variant),*
}

pub struct LanguageItems {
    items: ~[Option<ast::DefId>],
}

impl LanguageItems {
    pub fn new() -> LanguageItems {
        fn foo(_: LangItem) -> Option<ast::DefId> { None }

        LanguageItems {
            items: ~[$(foo($variant)),*]
        }
    }

    pub fn items<'a>(&'a self) -> Enumerate<vec::Items<'a, Option<ast::DefId>>> {
        self.items.iter().enumerate()
    }

    pub fn item_name(index: uint) -> &'static str {
        let item: Option<LangItem> = FromPrimitive::from_uint(index);
        match item {
            $( Some($variant) => $name, )*
            None => "???"
        }
    }

    pub fn require(&self, it: LangItem) -> Result<ast::DefId, ~str> {
        match self.items[it as uint] {
            Some(id) => Ok(id),
            None => Err(format!("requires `{}` lang_item",
                             LanguageItems::item_name(it as uint)))
        }
    }

    pub fn to_builtin_kind(&self, id: ast::DefId) -> Option<BuiltinBound> {
        if Some(id) == self.freeze_trait() {
            Some(BoundFreeze)
        } else if Some(id) == self.send_trait() {
            Some(BoundSend)
        } else if Some(id) == self.sized_trait() {
            Some(BoundSized)
        } else if Some(id) == self.pod_trait() {
            Some(BoundPod)
        } else {
            None
        }
    }

    $(
        pub fn $method(&self) -> Option<ast::DefId> {
            self.items[$variant as uint]
        }
    )*
}

struct LanguageItemCollector {
    items: LanguageItems,

    session: Session,

    item_refs: HashMap<&'static str, uint>,
}

struct LanguageItemVisitor<'a> {
    this: &'a mut LanguageItemCollector,
}

impl<'a> Visitor<()> for LanguageItemVisitor<'a> {
    fn visit_item(&mut self, item: &ast::Item, _: ()) {
        match extract(item.attrs) {
            Some(value) => {
                let item_index = self.this.item_refs.find_equiv(&value).map(|x| *x);

                match item_index {
                    Some(item_index) => {
                        self.this.collect_item(item_index, local_def(item.id))
                    }
                    None => {}
                }
            }
            None => {}
        }

        visit::walk_item(self, item, ());
    }
}

impl LanguageItemCollector {
    pub fn new(session: Session) -> LanguageItemCollector {
        let mut item_refs = HashMap::new();

        $( item_refs.insert($name, $variant as uint); )*

        LanguageItemCollector {
            session: session,
            items: LanguageItems::new(),
            item_refs: item_refs
        }
    }

    pub fn collect_item(&mut self, item_index: uint, item_def_id: ast::DefId) {
        // Check for duplicates.
        match self.items.items[item_index] {
            Some(original_def_id) if original_def_id != item_def_id => {
                self.session.err(format!("duplicate entry for `{}`",
                                      LanguageItems::item_name(item_index)));
            }
            Some(_) | None => {
                // OK.
            }
        }

        // Matched.
        self.items.items[item_index] = Some(item_def_id);
    }

    pub fn collect_local_language_items(&mut self, crate: &ast::Crate) {
        let mut v = LanguageItemVisitor { this: self };
        visit::walk_crate(&mut v, crate, ());
    }

    pub fn collect_external_language_items(&mut self) {
        let crate_store = self.session.cstore;
        crate_store.iter_crate_data(|crate_number, _crate_metadata| {
            each_lang_item(crate_store, crate_number, |node_id, item_index| {
                let def_id = ast::DefId { crate: crate_number, node: node_id };
                self.collect_item(item_index, def_id);
                true
            });
        })
    }

    pub fn collect(&mut self, crate: &ast::Crate) {
        self.collect_local_language_items(crate);
        self.collect_external_language_items();
    }
}

pub fn extract(attrs: &[ast::Attribute]) -> Option<@str> {
    for attribute in attrs.iter() {
        match attribute.name_str_pair() {
            Some((key, value)) if "lang" == key => {
                return Some(value);
            }
            Some(..) | None => {}
        }
    }

    return None;
}

pub fn collect_language_items(crate: &ast::Crate,
                              session: Session) -> @LanguageItems {
    let mut collector = LanguageItemCollector::new(session);
    collector.collect(crate);
    let LanguageItemCollector { items, .. } = collector;
    session.abort_if_errors();
    @items
}

// End of the macro
    }
}

lets_do_this! {
//  Variant name,                    Name,                      Method name;
    FreezeTraitLangItem,             "freeze",                  freeze_trait;
    SendTraitLangItem,               "send",                    send_trait;
    SizedTraitLangItem,              "sized",                   sized_trait;
    PodTraitLangItem,                "pod",                     pod_trait;

    DropTraitLangItem,               "drop",                    drop_trait;

    AddTraitLangItem,                "add",                     add_trait;
    SubTraitLangItem,                "sub",                     sub_trait;
    MulTraitLangItem,                "mul",                     mul_trait;
    DivTraitLangItem,                "div",                     div_trait;
    RemTraitLangItem,                "rem",                     rem_trait;
    NegTraitLangItem,                "neg",                     neg_trait;
    NotTraitLangItem,                "not",                     not_trait;
    BitXorTraitLangItem,             "bitxor",                  bitxor_trait;
    BitAndTraitLangItem,             "bitand",                  bitand_trait;
    BitOrTraitLangItem,              "bitor",                   bitor_trait;
    ShlTraitLangItem,                "shl",                     shl_trait;
    ShrTraitLangItem,                "shr",                     shr_trait;
    IndexTraitLangItem,              "index",                   index_trait;

    EqTraitLangItem,                 "eq",                      eq_trait;
    OrdTraitLangItem,                "ord",                     ord_trait;

    StrEqFnLangItem,                 "str_eq",                  str_eq_fn;
    UniqStrEqFnLangItem,             "uniq_str_eq",             uniq_str_eq_fn;
    FailFnLangItem,                  "fail_",                   fail_fn;
    FailBoundsCheckFnLangItem,       "fail_bounds_check",       fail_bounds_check_fn;
    ExchangeMallocFnLangItem,        "exchange_malloc",         exchange_malloc_fn;
    ClosureExchangeMallocFnLangItem, "closure_exchange_malloc", closure_exchange_malloc_fn;
    ExchangeFreeFnLangItem,          "exchange_free",           exchange_free_fn;
    MallocFnLangItem,                "malloc",                  malloc_fn;
    FreeFnLangItem,                  "free",                    free_fn;
    StrDupUniqFnLangItem,            "strdup_uniq",             strdup_uniq_fn;

    StartFnLangItem,                 "start",                   start_fn;

    TyDescStructLangItem,            "ty_desc",                 ty_desc;
    TyVisitorTraitLangItem,          "ty_visitor",              ty_visitor;
    OpaqueStructLangItem,            "opaque",                  opaque;

    EventLoopFactoryLangItem,        "event_loop_factory",      event_loop_factory;

    TypeIdLangItem,                  "type_id",                 type_id;

    ManagedHeapLangItem,             "managed_heap",            managed_heap;
    ExchangeHeapLangItem,            "exchange_heap",           exchange_heap;
    GcLangItem,                      "gc",                      gc;

    EhPersonalityLangItem,           "eh_personality",          eh_personality_fn;

    CharImplLangItem,                "char_impl",               char_impl;
    IntImplLangItem,                 "int_impl",                int_impl;
    I8ImplLangItem,                  "i8_impl",                 i8_impl;
    I16ImplLangItem,                 "i16_impl",                i16_impl;
    I32ImplLangItem,                 "i32_impl",                i32_impl;
    I64ImplLangItem,                 "i64_impl",                i64_impl;
    UintImplLangItem,                "uint_impl",               uint_impl;
    U8ImplLangItem,                  "u8_impl",                 u8_impl;
    U16ImplLangItem,                 "u16_impl",                u16_impl;
    U32ImplLangItem,                 "u32_impl",                u32_impl;
    U64ImplLangItem,                 "u64_impl",                u64_impl;
    BoolImplLangItem,                "bool_impl",               bool_impl;
    NilImplLangItem,                 "nil_impl",                nil_impl;
    F32ImplLangItem,                 "f32_impl",                f32_impl;
    F64ImplLangItem,                 "f64_impl",                f64_impl;
}
