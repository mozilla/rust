// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// force-host
// no-prefer-dynamic

#![feature(proc_macro)]
#![crate_type = "proc-macro"]

extern crate proc_macro;

use proc_macro::{TokenStream, TokenTree, Delimiter, Literal, Spacing, Group};

#[proc_macro_attribute]
pub fn foo(attr: TokenStream, input: TokenStream) -> TokenStream {
    assert!(attr.is_empty());
    let input = input.into_iter().collect::<Vec<_>>();
    {
        let mut cursor = &input[..];
        assert_inline(&mut cursor);
        assert_doc(&mut cursor);
        assert_inline(&mut cursor);
        assert_doc(&mut cursor);
        assert_foo(&mut cursor);
        assert!(cursor.is_empty());
    }
    fold_stream(input.into_iter().collect())
}

#[proc_macro_attribute]
pub fn bar(attr: TokenStream, input: TokenStream) -> TokenStream {
    assert!(attr.is_empty());
    let input = input.into_iter().collect::<Vec<_>>();
    {
        let mut cursor = &input[..];
        assert_inline(&mut cursor);
        assert_doc(&mut cursor);
        assert_invoc(&mut cursor);
        assert_inline(&mut cursor);
        assert_doc(&mut cursor);
        assert_foo(&mut cursor);
        assert!(cursor.is_empty());
    }
    input.into_iter().collect()
}

fn assert_inline(slice: &mut &[TokenTree]) {
    match &slice[0] {
        TokenTree::Op(tt) => assert_eq!(tt.op(), '#'),
        _ => panic!("expected '#' char"),
    }
    match &slice[1] {
        TokenTree::Group(tt) => assert_eq!(tt.delimiter(), Delimiter::Bracket),
        _ => panic!("expected brackets"),
    }
    *slice = &slice[2..];
}

fn assert_doc(slice: &mut &[TokenTree]) {
    match &slice[0] {
        TokenTree::Op(tt) => {
            assert_eq!(tt.op(), '#');
            assert_eq!(tt.spacing(), Spacing::Alone);
        }
        _ => panic!("expected #"),
    }
    let inner = match &slice[1] {
        TokenTree::Group(tt) => {
            assert_eq!(tt.delimiter(), Delimiter::Bracket);
            tt.stream()
        }
        _ => panic!("expected brackets"),
    };
    let tokens = inner.into_iter().collect::<Vec<_>>();
    let tokens = &tokens[..];

    if tokens.len() != 3 {
        panic!("expected three tokens in doc")
    }

    match &tokens[0] {
        TokenTree::Term(tt) => assert_eq!("doc", tt.as_str()),
        _ => panic!("expected `doc`"),
    }
    match &tokens[1] {
        TokenTree::Op(tt) => {
            assert_eq!(tt.op(), '=');
            assert_eq!(tt.spacing(), Spacing::Alone);
        }
        _ => panic!("expected equals"),
    }
    match tokens[2] {
        TokenTree::Literal(_) => {}
        _ => panic!("expected literal"),
    }

    *slice = &slice[2..];
}

fn assert_invoc(slice: &mut &[TokenTree]) {
    match &slice[0] {
        TokenTree::Op(tt) => assert_eq!(tt.op(), '#'),
        _ => panic!("expected '#' char"),
    }
    match &slice[1] {
        TokenTree::Group(tt) => assert_eq!(tt.delimiter(), Delimiter::Bracket),
        _ => panic!("expected brackets"),
    }
    *slice = &slice[2..];
}

fn assert_foo(slice: &mut &[TokenTree]) {
    match &slice[0] {
        TokenTree::Term(tt) => assert_eq!(tt.as_str(), "fn"),
        _ => panic!("expected fn"),
    }
    match &slice[1] {
        TokenTree::Term(tt) => assert_eq!(tt.as_str(), "foo"),
        _ => panic!("expected foo"),
    }
    match &slice[2] {
        TokenTree::Group(tt) => {
            assert_eq!(tt.delimiter(), Delimiter::Parenthesis);
            assert!(tt.stream().is_empty());
        }
        _ => panic!("expected parens"),
    }
    match &slice[3] {
        TokenTree::Group(tt) => assert_eq!(tt.delimiter(), Delimiter::Brace),
        _ => panic!("expected braces"),
    }
    *slice = &slice[4..];
}

fn fold_stream(input: TokenStream) -> TokenStream {
    input.into_iter().map(fold_tree).collect()
}

fn fold_tree(input: TokenTree) -> TokenTree {
    match input {
        TokenTree::Group(b) => {
            TokenTree::Group(Group::new(b.delimiter(), fold_stream(b.stream())))
        }
        TokenTree::Op(b) => TokenTree::Op(b),
        TokenTree::Term(a) => TokenTree::Term(a),
        TokenTree::Literal(a) => {
            if a.to_string() != "\"foo\"" {
                TokenTree::Literal(a)
            } else {
                TokenTree::Literal(Literal::i32_unsuffixed(3))
            }
        }
    }
}
