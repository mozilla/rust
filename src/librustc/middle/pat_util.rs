// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use middle::def::*;
use middle::resolve;
use middle::ty;

use std::collections::HashMap;
use syntax::ast;
use syntax::ast_util::{walk_pat};
use syntax::codemap::{Span, DUMMY_SP};
use syntax::owned_slice::OwnedSlice;

pub type PatIdMap = HashMap<ast::Ident, ast::NodeId>;

// This is used because same-named variables in alternative patterns need to
// use the NodeId of their namesake in the first pattern.
pub fn pat_id_map(dm: &resolve::DefMap, pat: &ast::Pat) -> PatIdMap {
    let mut map = HashMap::new();
    pat_bindings(dm, pat, |_bm, p_id, _s, path1| {
      map.insert(path1.node, p_id);
    });
    map
}

pub fn pat_is_variant_or_struct(dm: &resolve::DefMap, pat: &ast::Pat) -> bool {
    match pat.node {
        ast::PatEnum(_, _) |
        ast::PatIdent(_, _, None) |
        ast::PatStruct(..) => {
            match dm.borrow().find(&pat.id) {
                Some(&DefVariant(..)) | Some(&DefStruct(..)) => true,
                _ => false
            }
        }
        _ => false
    }
}

pub fn pat_is_const(dm: &resolve::DefMap, pat: &ast::Pat) -> bool {
    match pat.node {
        ast::PatIdent(_, _, None) | ast::PatEnum(..) => {
            match dm.borrow().find(&pat.id) {
                Some(&DefStatic(_, false)) => true,
                _ => false
            }
        }
        _ => false
    }
}

pub fn pat_is_binding(dm: &resolve::DefMap, pat: &ast::Pat) -> bool {
    match pat.node {
        ast::PatIdent(..) => {
            !pat_is_variant_or_struct(dm, pat) &&
            !pat_is_const(dm, pat)
        }
        _ => false
    }
}

pub fn pat_is_binding_or_wild(dm: &resolve::DefMap, pat: &ast::Pat) -> bool {
    match pat.node {
        ast::PatIdent(..) => pat_is_binding(dm, pat),
        ast::PatWild(_) => true,
        _ => false
    }
}

/// Call `it` on every "binding" in a pattern, e.g., on `a` in
/// `match foo() { Some(a) => (), None => () }`
pub fn pat_bindings(dm: &resolve::DefMap,
                    pat: &ast::Pat,
                    it: |ast::BindingMode, ast::NodeId, Span, &ast::SpannedIdent|) {
    walk_pat(pat, |p| {
        match p.node {
          ast::PatIdent(binding_mode, ref pth, _) if pat_is_binding(dm, p) => {
            it(binding_mode, p.id, p.span, pth);
          }
          _ => {}
        }
        true
    });
}

/// Checks if the pattern contains any patterns that bind something to
/// an ident, e.g. `foo`, or `Foo(foo)` or `foo @ Bar(..)`.
pub fn pat_contains_bindings(dm: &resolve::DefMap, pat: &ast::Pat) -> bool {
    let mut contains_bindings = false;
    walk_pat(pat, |p| {
        if pat_is_binding(dm, p) {
            contains_bindings = true;
            false // there's at least one binding, can short circuit now.
        } else {
            true
        }
    });
    contains_bindings
}

pub fn simple_identifier<'a>(pat: &'a ast::Pat) -> Option<&'a ast::Ident> {
    match pat.node {
        ast::PatIdent(ast::BindByValue(_), ref path1, None) => {
            Some(&path1.node)
        }
        _ => {
            None
        }
    }
}

pub fn def_to_path(tcx: &ty::ctxt, id: ast::DefId) -> ast::Path {
    ty::with_path(tcx, id, |mut path| ast::Path {
        global: false,
        segments: path.last().map(|elem| ast::PathSegment {
            identifier: ast::Ident::new(elem.name()),
            lifetimes: vec!(),
            types: OwnedSlice::empty()
        }).into_iter().collect(),
        span: DUMMY_SP,
    })
}
