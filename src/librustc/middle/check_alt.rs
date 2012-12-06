// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use syntax::ast::*;
use syntax::ast_util::{variant_def_ids, dummy_sp, unguarded_pat};
use const_eval::{eval_const_expr, const_val, const_int, const_bool,
                 compare_const_vals, lookup_const_by_id};
use syntax::codemap::span;
use syntax::print::pprust::pat_to_str;
use util::ppaux::ty_to_str;
use pat_util::*;
use syntax::visit;
use middle::ty;
use middle::ty::*;
use middle::typeck::method_map;
use std::map::HashMap;

struct AltCheckCtxt {
    tcx: ty::ctxt,
    method_map: method_map,
}

fn check_crate(tcx: ty::ctxt, method_map: method_map, crate: @crate) {
    let cx = @AltCheckCtxt { tcx: tcx, method_map: method_map };
    visit::visit_crate(*crate, (), visit::mk_vt(@{
        visit_expr: |a,b,c| check_expr(cx, a, b, c),
        visit_local: |a,b,c| check_local(cx, a, b, c),
        visit_fn: |kind, decl, body, sp, id, e, v|
            check_fn(cx, kind, decl, body, sp, id, e, v),
        .. *visit::default_visitor::<()>()
    }));
    tcx.sess.abort_if_errors();
}

fn check_expr(cx: @AltCheckCtxt, ex: @expr, &&s: (), v: visit::vt<()>) {
    visit::visit_expr(ex, s, v);
    match ex.node {
      expr_match(scrut, ref arms) => {
        check_arms(cx, (*arms));
        /* Check for exhaustiveness */
         // Check for empty enum, because is_useful only works on inhabited
         // types.
       let pat_ty = node_id_to_type(cx.tcx, scrut.id);
       if (*arms).is_empty() {
           if !type_is_empty(cx.tcx, pat_ty) {
               // We know the type is inhabited, so this must be wrong
               cx.tcx.sess.span_err(ex.span, fmt!("non-exhaustive patterns: \
                            type %s is non-empty",
                            ty_to_str(cx.tcx, pat_ty)));
           }
           // If the type *is* empty, it's vacuously exhaustive
           return;
       }
       match ty::get(pat_ty).sty {
          ty_enum(did, _) => {
              if (*enum_variants(cx.tcx, did)).is_empty() &&
                    (*arms).is_empty() {

               return;
            }
          }
          _ => { /* We assume only enum types can be uninhabited */ }
       }
       let arms = vec::concat(vec::filter_map((*arms), unguarded_pat));
       check_exhaustive(cx, ex.span, arms);
     }
     _ => ()
    }
}

// Check for unreachable patterns
fn check_arms(cx: @AltCheckCtxt, arms: ~[arm]) {
    let mut seen = ~[];
    for arms.each |arm| {
        for arm.pats.each |pat| {
            let v = ~[*pat];
            match is_useful(cx, seen, v) {
              not_useful => {
                cx.tcx.sess.span_err(pat.span, ~"unreachable pattern");
              }
              _ => ()
            }
            if arm.guard.is_none() { seen.push(v); }
        }
    }
}

fn raw_pat(p: @pat) -> @pat {
    match p.node {
      pat_ident(_, _, Some(s)) => { raw_pat(s) }
      _ => { p }
    }
}

fn check_exhaustive(cx: @AltCheckCtxt, sp: span, pats: ~[@pat]) {
    assert(pats.is_not_empty());
    let ext = match is_useful(cx, vec::map(pats, |p| ~[*p]), ~[wild()]) {
      not_useful => return, // This is good, wildcard pattern isn't reachable
      useful_ => None,
      useful(ty, ref ctor) => {
        match ty::get(ty).sty {
          ty::ty_bool => {
            match (*ctor) {
              val(const_bool(true)) => Some(~"true"),
              val(const_bool(false)) => Some(~"false"),
              _ => None
            }
          }
          ty::ty_enum(id, _) => {
              let vid = match (*ctor) { variant(id) => id,
              _ => fail ~"check_exhaustive: non-variant ctor" };
            match vec::find(*ty::enum_variants(cx.tcx, id),
                                |v| v.id == vid) {
                Some(v) => Some(cx.tcx.sess.str_of(v.name)),
              None => fail ~"check_exhaustive: bad variant in ctor"
            }
          }
          _ => None
        }
      }
    };
    let msg = ~"non-exhaustive patterns" + match ext {
      Some(ref s) => ~": " + (*s) + ~" not covered",
      None => ~""
    };
    cx.tcx.sess.span_err(sp, msg);
}

type matrix = ~[~[@pat]];

enum useful { useful(ty::t, ctor), useful_, not_useful }

enum ctor {
    single,
    variant(def_id),
    val(const_val),
    range(const_val, const_val),
}

impl ctor : cmp::Eq {
    pure fn eq(&self, other: &ctor) -> bool {
        match ((*self), (*other)) {
            (single, single) => true,
            (variant(did_self), variant(did_other)) => did_self == did_other,
            (val(ref cv_self), val(ref cv_other)) =>
                (*cv_self) == (*cv_other),
            (range(ref cv0_self, ref cv1_self),
             range(ref cv0_other, ref cv1_other)) => {
                (*cv0_self) == (*cv0_other) && (*cv1_self) == (*cv1_other)
            }
            (single, _) | (variant(_), _) | (val(_), _) | (range(*), _) => {
                false
            }
        }
    }
    pure fn ne(&self, other: &ctor) -> bool { !(*self).eq(other) }
}

// Algorithm from http://moscova.inria.fr/~maranget/papers/warn/index.html
//
// Whether a vector `v` of patterns is 'useful' in relation to a set of such
// vectors `m` is defined as there being a set of inputs that will match `v`
// but not any of the sets in `m`.
//
// This is used both for reachability checking (if a pattern isn't useful in
// relation to preceding patterns, it is not reachable) and exhaustiveness
// checking (if a wildcard pattern is useful in relation to a matrix, the
// matrix isn't exhaustive).

// Note: is_useful doesn't work on empty types, as the paper notes.
// So it assumes that v is non-empty.
fn is_useful(cx: @AltCheckCtxt, m: matrix, v: ~[@pat]) -> useful {
    if m.len() == 0u { return useful_; }
    if m[0].len() == 0u { return not_useful; }
    let real_pat = match vec::find(m, |r| r[0].id != 0) {
      Some(r) => r[0], None => v[0]
    };
    let left_ty = if real_pat.id == 0 { ty::mk_nil(cx.tcx) }
                  else { ty::node_id_to_type(cx.tcx, real_pat.id) };

    match pat_ctor_id(cx, v[0]) {
      None => {
        match missing_ctor(cx, m, left_ty) {
          None => {
            match ty::get(left_ty).sty {
              ty::ty_bool => {
                match is_useful_specialized(cx, m, v,
                                            val(const_bool(true)),
                                            0u, left_ty){
                  not_useful => {
                    is_useful_specialized(cx, m, v,
                                          val(const_bool(false)),
                                          0u, left_ty)
                  }
                  ref u => (*u)
                }
              }
              ty::ty_enum(eid, _) => {
                for (*ty::enum_variants(cx.tcx, eid)).each |va| {
                    match is_useful_specialized(cx, m, v, variant(va.id),
                                                va.args.len(), left_ty) {
                      not_useful => (),
                      ref u => return (*u)
                    }
                }
                not_useful
              }
              _ => {
                let arity = ctor_arity(cx, single, left_ty);
                is_useful_specialized(cx, m, v, single, arity, left_ty)
              }
            }
          }
          Some(ref ctor) => {
            match is_useful(cx, vec::filter_map(m, |r| default(cx, *r)),
                            vec::tail(v)) {
              useful_ => useful(left_ty, (*ctor)),
              ref u => (*u)
            }
          }
        }
      }
      Some(ref v0_ctor) => {
        let arity = ctor_arity(cx, (*v0_ctor), left_ty);
        is_useful_specialized(cx, m, v, (*v0_ctor), arity, left_ty)
      }
    }
}

fn is_useful_specialized(cx: @AltCheckCtxt, m: matrix, v: ~[@pat], ctor: ctor,
                          arity: uint, lty: ty::t) -> useful {
    let ms = vec::filter_map(m, |r| specialize(cx, *r, ctor, arity, lty));
    let could_be_useful = is_useful(
        cx, ms, specialize(cx, v, ctor, arity, lty).get());
    match could_be_useful {
      useful_ => useful(lty, ctor),
      ref u => (*u)
    }
}

fn pat_ctor_id(cx: @AltCheckCtxt, p: @pat) -> Option<ctor> {
    let pat = raw_pat(p);
    match pat.node {
      pat_wild => { None }
      pat_ident(_, _, _) | pat_enum(_, _) => {
        match cx.tcx.def_map.find(pat.id) {
          Some(def_variant(_, id)) => Some(variant(id)),
          Some(def_const(did)) => {
            let const_expr = lookup_const_by_id(cx.tcx, did).get();
            Some(val(eval_const_expr(cx.tcx, const_expr)))
          }
          _ => None
        }
      }
      pat_lit(expr) => { Some(val(eval_const_expr(cx.tcx, expr))) }
      pat_range(lo, hi) => {
        Some(range(eval_const_expr(cx.tcx, lo), eval_const_expr(cx.tcx, hi)))
      }
      pat_struct(*) => {
        match cx.tcx.def_map.find(pat.id) {
          Some(def_variant(_, id)) => Some(variant(id)),
          _ => Some(single)
        }
      }
      pat_box(_) | pat_uniq(_) | pat_rec(_, _) | pat_tup(_) |
      pat_region(*) => {
        Some(single)
      }
    }
}

fn is_wild(cx: @AltCheckCtxt, p: @pat) -> bool {
    let pat = raw_pat(p);
    match pat.node {
      pat_wild => { true }
      pat_ident(_, _, _) => {
        match cx.tcx.def_map.find(pat.id) {
          Some(def_variant(_, _)) | Some(def_const(*)) => { false }
          _ => { true }
        }
      }
      _ => { false }
    }
}

fn missing_ctor(cx: @AltCheckCtxt,
                m: matrix,
                left_ty: ty::t)
             -> Option<ctor> {
    match ty::get(left_ty).sty {
      ty::ty_box(_) | ty::ty_uniq(_) | ty::ty_rptr(*) | ty::ty_tup(_) |
      ty::ty_rec(_) | ty::ty_class(*) => {
        for m.each |r| {
            if !is_wild(cx, r[0]) { return None; }
        }
        return Some(single);
      }
      ty::ty_enum(eid, _) => {
        let mut found = ~[];
        for m.each |r| {
            do option::iter(&pat_ctor_id(cx, r[0])) |id| {
                if !vec::contains(found, id) {
                    found.push(*id);
                }
            }
        }
        let variants = ty::enum_variants(cx.tcx, eid);
        if found.len() != (*variants).len() {
            for vec::each(*variants) |v| {
                if !found.contains(&(variant(v.id))) {
                    return Some(variant(v.id));
                }
            }
            fail;
        } else { None }
      }
      ty::ty_nil => None,
      ty::ty_bool => {
        let mut true_found = false, false_found = false;
        for m.each |r| {
            match pat_ctor_id(cx, r[0]) {
              None => (),
              Some(val(const_bool(true))) => true_found = true,
              Some(val(const_bool(false))) => false_found = true,
              _ => fail ~"impossible case"
            }
        }
        if true_found && false_found { None }
        else if true_found { Some(val(const_bool(false))) }
        else { Some(val(const_bool(true))) }
      }
      _ => Some(single)
    }
}

fn ctor_arity(cx: @AltCheckCtxt, ctor: ctor, ty: ty::t) -> uint {
    match ty::get(ty).sty {
      ty::ty_tup(fs) => fs.len(),
      ty::ty_rec(fs) => fs.len(),
      ty::ty_box(_) | ty::ty_uniq(_) | ty::ty_rptr(*) => 1u,
      ty::ty_enum(eid, _) => {
          let id = match ctor { variant(id) => id,
          _ => fail ~"impossible case" };
        match vec::find(*ty::enum_variants(cx.tcx, eid), |v| v.id == id ) {
            Some(v) => v.args.len(),
            None => fail ~"impossible case"
        }
      }
      ty::ty_class(cid, _) => ty::lookup_class_fields(cx.tcx, cid).len(),
      _ => 0u
    }
}

fn wild() -> @pat {
    @{id: 0, node: pat_wild, span: syntax::ast_util::dummy_sp()}
}

fn specialize(cx: @AltCheckCtxt, r: ~[@pat], ctor_id: ctor, arity: uint,
              left_ty: ty::t) -> Option<~[@pat]> {
    let r0 = raw_pat(r[0]);
    match r0.node {
      pat_wild => Some(vec::append(vec::from_elem(arity, wild()),
                                   vec::tail(r))),
      pat_ident(_, _, _) => {
        match cx.tcx.def_map.find(r0.id) {
          Some(def_variant(_, id)) => {
            if variant(id) == ctor_id { Some(vec::tail(r)) }
            else { None }
          }
          Some(def_const(did)) => {
            let const_expr = lookup_const_by_id(cx.tcx, did).get();
            let e_v = eval_const_expr(cx.tcx, const_expr);
            let match_ = match ctor_id {
                val(ref v) => compare_const_vals(e_v, (*v)) == 0,
                range(ref c_lo, ref c_hi) => {
                    compare_const_vals((*c_lo), e_v) >= 0 &&
                        compare_const_vals((*c_hi), e_v) <= 0
                }
                single => true,
                _ => fail ~"type error"
            };
            if match_ { Some(vec::tail(r)) } else { None }
          }
          _ => Some(vec::append(vec::from_elem(arity, wild()), vec::tail(r)))
        }
      }
      pat_enum(_, args) => {
        match cx.tcx.def_map.get(r0.id) {
          def_variant(_, id) if variant(id) == ctor_id => {
            let args = match args {
              Some(args) => args,
              None => vec::from_elem(arity, wild())
            };
            Some(vec::append(args, vec::tail(r)))
          }
          def_variant(_, _) => None,
          def_class(*) => {
            // XXX: Is this right? --pcw
            let new_args;
            match args {
              Some(args) => new_args = args,
              None => new_args = vec::from_elem(arity, wild())
            }
            Some(vec::append(new_args, vec::tail(r)))
          }
          _ => None
        }
      }
      pat_rec(flds, _) => {
        let ty_flds = match ty::get(left_ty).sty {
            ty::ty_rec(flds) => flds,
            _ => fail ~"bad type for pat_rec"
        };
        let args = vec::map(ty_flds, |ty_fld| {
            match vec::find(flds, |f| f.ident == ty_fld.ident ) {
              Some(f) => f.pat,
              _ => wild()
            }
        });
        Some(vec::append(args, vec::tail(r)))
      }
      pat_struct(_, flds, _) => {
        // Is this a struct or an enum variant?
        match cx.tcx.def_map.get(r0.id) {
            def_variant(_, variant_id) => {
                if variant(variant_id) == ctor_id {
                    // XXX: Is this right? --pcw
                    let args = flds.map(|ty_field| {
                        match vec::find(flds, |f| f.ident == ty_field.ident) {
                            Some(f) => f.pat,
                            _ => wild()
                        }
                    });
                    Some(vec::append(args, vec::tail(r)))
                } else {
                    None
                }
            }
            _ => {
                // Grab the class data that we care about.
                let class_fields, class_id;
                match ty::get(left_ty).sty {
                    ty::ty_class(cid, _) => {
                        class_id = cid;
                        class_fields = ty::lookup_class_fields(cx.tcx,
                                                               class_id);
                    }
                    _ => {
                        cx.tcx.sess.span_bug(r0.span, ~"struct pattern \
                                                        didn't resolve to a \
                                                        struct");
                    }
                }
                let args = vec::map(class_fields, |class_field| {
                    match vec::find(flds, |f| f.ident == class_field.ident ) {
                      Some(f) => f.pat,
                      _ => wild()
                    }
                });
                Some(vec::append(args, vec::tail(r)))
            }
        }
      }
      pat_tup(args) => Some(vec::append(args, vec::tail(r))),
      pat_box(a) | pat_uniq(a) | pat_region(a) =>
          Some(vec::append(~[a], vec::tail(r))),
      pat_lit(expr) => {
        let e_v = eval_const_expr(cx.tcx, expr);
        let match_ = match ctor_id {
          val(ref v) => compare_const_vals(e_v, (*v)) == 0,
          range(ref c_lo, ref c_hi) => {
            compare_const_vals((*c_lo), e_v) >= 0 &&
                compare_const_vals((*c_hi), e_v) <= 0
          }
          single => true,
          _ => fail ~"type error"
        };
        if match_ { Some(vec::tail(r)) } else { None }
      }
      pat_range(lo, hi) => {
        let (c_lo, c_hi) = match ctor_id {
          val(ref v) => ((*v), (*v)),
          range(ref lo, ref hi) => ((*lo), (*hi)),
          single => return Some(vec::tail(r)),
          _ => fail ~"type error"
        };
        let v_lo = eval_const_expr(cx.tcx, lo),
            v_hi = eval_const_expr(cx.tcx, hi);
        let match_ = compare_const_vals(c_lo, v_lo) >= 0 &&
                    compare_const_vals(c_hi, v_hi) <= 0;
        if match_ { Some(vec::tail(r)) } else { None }
      }
    }
}

fn default(cx: @AltCheckCtxt, r: ~[@pat]) -> Option<~[@pat]> {
    if is_wild(cx, r[0]) { Some(vec::tail(r)) }
    else { None }
}

fn check_local(cx: @AltCheckCtxt, loc: @local, &&s: (), v: visit::vt<()>) {
    visit::visit_local(loc, s, v);
    if is_refutable(cx, loc.node.pat) {
        cx.tcx.sess.span_err(loc.node.pat.span,
                          ~"refutable pattern in local binding");
    }
}

fn check_fn(cx: @AltCheckCtxt,
            kind: visit::fn_kind,
            decl: fn_decl,
            body: blk,
            sp: span,
            id: node_id,
            &&s: (),
            v: visit::vt<()>) {
    visit::visit_fn(kind, decl, body, sp, id, s, v);
    for decl.inputs.each |input| {
        if is_refutable(cx, input.pat) {
            cx.tcx.sess.span_err(input.pat.span,
                              ~"refutable pattern in function argument");
        }
    }
}

fn is_refutable(cx: @AltCheckCtxt, pat: &pat) -> bool {
    match cx.tcx.def_map.find(pat.id) {
      Some(def_variant(enum_id, _)) => {
        if vec::len(*ty::enum_variants(cx.tcx, enum_id)) != 1u {
            return true;
        }
      }
      Some(def_const(*)) => return true,
      _ => ()
    }

    match pat.node {
      pat_box(sub) | pat_uniq(sub) | pat_region(sub) |
      pat_ident(_, _, Some(sub)) => {
        is_refutable(cx, sub)
      }
      pat_wild | pat_ident(_, _, None) => { false }
      pat_lit(@{node: expr_lit(@{node: lit_nil, _}), _}) => { false } // "()"
      pat_lit(_) | pat_range(_, _) => { true }
      pat_rec(fields, _) => {
        fields.any(|f| is_refutable(cx, f.pat))
      }
      pat_struct(_, fields, _) => {
        fields.any(|f| is_refutable(cx, f.pat))
      }
      pat_tup(elts) => {
        elts.any(|elt| is_refutable(cx, *elt))
      }
      pat_enum(_, Some(args)) => {
        args.any(|a| is_refutable(cx, *a))
      }
      pat_enum(_,_) => { false }
    }
}

// Local Variables:
// mode: rust
// fill-column: 78;
// indent-tabs-mode: nil
// c-basic-offset: 4
// buffer-file-coding-system: utf-8-unix
// End:
