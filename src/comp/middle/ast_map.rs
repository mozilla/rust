import std::map;
import syntax::ast::*;
import syntax::ast_util;
import syntax::{visit, codemap};

enum path_elt { path_mod(str), path_name(str) }
type path = [path_elt];

fn path_to_str_with_sep(p: path, sep: str) -> str {
    let strs = vec::map(p) {|e|
        alt e {
          path_mod(s) { s }
          path_name(s) { s }
        }
    };
    str::connect(strs, sep)
}

fn path_to_str(p: path) -> str {
    path_to_str_with_sep(p, "::")
}

enum ast_node {
    node_item(@item, @path),
    node_native_item(@native_item, @path),
    node_method(@method, node_id, @path),
    node_variant(variant, def_id, @path),
    node_expr(@expr),
    // Locals are numbered, because the alias analysis needs to know in which
    // order they are introduced.
    node_arg(arg, uint),
    node_local(uint),
    node_res_ctor(@item),
}

type map = std::map::map<node_id, ast_node>;
type ctx = {map: map, mutable path: path, mutable local_id: uint};
type vt = visit::vt<ctx>;

fn mk_ast_map_visitor() -> vt {
    ret visit::mk_vt(@{
        visit_item: map_item,
        visit_native_item: map_native_item,
        visit_expr: map_expr,
        visit_fn: map_fn,
        visit_local: map_local,
        visit_arm: map_arm
        with *visit::default_visitor()
    });
}

fn map_crate(c: crate) -> map {
    let cx = {map: std::map::new_int_hash(),
              mutable path: [],
              mutable local_id: 0u};
    visit::visit_crate(c, cx, mk_ast_map_visitor());
    ret cx.map;
}

// Used for items loaded from external crate that are being inlined into this
// crate:
fn map_decoded_item(map: map, path: path, i: @item) {
    // I believe it is ok for the local IDs of inlined items from other crates
    // to overlap with the local ids from this crate, so just generate the ids
    // starting from 0.  (In particular, I think these ids are only used in
    // alias analysis, which we will not be running on the inlined items, and
    // even if we did I think it only needs an ordering between local
    // variables that are simultaneously in scope).
    let cx = {map: map,
              mutable path: path,
              mutable local_id: 0u};
    let v = mk_ast_map_visitor();
    v.visit_item(i, cx, v);
}

fn map_fn(fk: visit::fn_kind, decl: fn_decl, body: blk,
          sp: codemap::span, id: node_id, cx: ctx, v: vt) {
    for a in decl.inputs {
        cx.map.insert(a.id, node_arg(a, cx.local_id));
        cx.local_id += 1u;
    }
    visit::visit_fn(fk, decl, body, sp, id, cx, v);
}

fn number_pat(cx: ctx, pat: @pat) {
    pat_util::walk_pat(pat) {|p|
        alt p.node {
          pat_ident(_, _) {
            cx.map.insert(p.id, node_local(cx.local_id));
            cx.local_id += 1u;
          }
          _ {}
        }
    };
}

fn map_local(loc: @local, cx: ctx, v: vt) {
    number_pat(cx, loc.node.pat);
    visit::visit_local(loc, cx, v);
}

fn map_arm(arm: arm, cx: ctx, v: vt) {
    number_pat(cx, arm.pats[0]);
    visit::visit_arm(arm, cx, v);
}

fn map_item(i: @item, cx: ctx, v: vt) {
    cx.map.insert(i.id, node_item(i, @cx.path));
    alt i.node {
      item_impl(_, _, _, ms) {
        for m in ms { cx.map.insert(m.id, node_method(m, i.id, @cx.path)); }
      }
      item_res(_, _, _, dtor_id, ctor_id) {
        cx.map.insert(ctor_id, node_res_ctor(i));
        cx.map.insert(dtor_id, node_item(i, @cx.path));
      }
      item_enum(vs, _) {
        for v in vs {
            cx.map.insert(v.node.id, node_variant(
                v, ast_util::local_def(i.id),
                @(cx.path + [path_name(i.ident)])));
        }
      }
      _ { }
    }
    alt i.node {
      item_mod(_) | item_native_mod(_) { cx.path += [path_mod(i.ident)]; }
      _ { cx.path += [path_name(i.ident)]; }
    }
    visit::visit_item(i, cx, v);
    vec::pop(cx.path);
}

fn map_native_item(i: @native_item, cx: ctx, v: vt) {
    cx.map.insert(i.id, node_native_item(i, @cx.path));
    visit::visit_native_item(i, cx, v);
}

fn map_expr(ex: @expr, cx: ctx, v: vt) {
    cx.map.insert(ex.id, node_expr(ex));
    visit::visit_expr(ex, cx, v);
}

// Local Variables:
// mode: rust
// fill-column: 78;
// indent-tabs-mode: nil
// c-basic-offset: 4
// buffer-file-coding-system: utf-8-unix
// End:
