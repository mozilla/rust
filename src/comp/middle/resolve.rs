import syntax::{ast, ast_util, codemap};
import syntax::ast::*;
import ast::{ident, fn_ident, def, def_id, node_id};
import syntax::ast_util::{local_def, def_id_of_def, class_item_ident};
import pat_util::*;

import front::attr;
import metadata::{csearch, cstore};
import driver::session::session;
import util::common::*;
import std::map::{new_int_hash, new_str_hash};
import syntax::codemap::span;
import syntax::visit;
import visit::vt;
import std::{list, deque};
import std::map::hashmap;
import std::list::{list, nil, cons};
import option::{is_none, is_some};
import syntax::print::pprust::*;

export resolve_crate, resolve_crate_reexports;
export def_map, ext_map, exp_map, impl_map;
export _impl, iscopes, method_info;

// Resolving happens in two passes. The first pass collects defids of all
// (internal) imports and modules, so that they can be looked up when needed,
// and then uses this information to resolve the imports. The second pass
// locates all names (in expressions, types, and alt patterns) and resolves
// them, storing the resulting def in the AST nodes.

enum scope {
    scope_toplevel,
    scope_crate,
    scope_item(@ast::item),
    scope_bare_fn(ast::fn_decl, node_id, [ast::ty_param]),
    scope_fn_expr(ast::fn_decl, node_id, [ast::ty_param]),
    scope_native_item(@ast::native_item),
    scope_loop(@ast::local), // there's only 1 decl per loop.
    scope_block(ast::blk, @mutable uint, @mutable uint),
    scope_arm(ast::arm),
    scope_method(ast::node_id, [ast::ty_param]),
}

type scopes = list<scope>;

fn top_scope() -> scopes {
    cons(scope_crate, @cons(scope_toplevel, @nil))
}

enum import_state {
    todo(ast::node_id, ast::ident, @[ast::ident], span, scopes),
    is_glob(@[ast::ident], scopes, span),
    resolving(span),
    resolved(option<def>, /* value */
             option<def>, /* type */
             option<def>, /* module */
             @[@_impl], /* impls */
             /* used for reporting unused import warning */
             ast::ident, span),
}

enum glob_import_state {
    glob_resolving(span),
    glob_resolved(option<def>,  /* value */
                  option<def>,  /* type */
                  option<def>), /* module */
}

type ext_hash = hashmap<{did: def_id, ident: str, ns: namespace}, def>;

fn new_ext_hash() -> ext_hash {
    type key = {did: def_id, ident: str, ns: namespace};
    fn hash(v: key) -> uint {
        ret str::hash(v.ident) + util::common::hash_def(v.did) +
                alt v.ns {
                  ns_val(_) { 1u }
                  ns_type { 2u }
                  ns_module { 3u }
                };
    }
    fn eq(v1: key, v2: key) -> bool {
        ret util::common::def_eq(v1.did, v2.did) &&
                str::eq(v1.ident, v2.ident) && v1.ns == v2.ns;
    }
    ret std::map::mk_hashmap::<key, def>(hash, eq);
}

enum mod_index_entry {
    mie_view_item(ident, node_id, span),
    mie_import_ident(node_id, span),
    mie_item(@ast::item),
    mie_class_item(node_id, /* parent class name */
                   @ast::class_item), /* class member */
    mie_native_item(@ast::native_item),
    mie_enum_variant(/* variant index */uint,
                     /*parts of enum item*/ [variant],
                    node_id, span),
}

type mod_index = hashmap<ident, list<mod_index_entry>>;

// A tuple of an imported def and the view_path from its originating import
type glob_imp_def = {def: def, path: @ast::view_path};

type indexed_mod = {
    m: option<ast::_mod>,
    index: mod_index,
    mutable glob_imports: [glob_imp_def],
    mutable globbed_exports: [ident],
    glob_imported_names: hashmap<str, glob_import_state>,
    path: str
};

/* native modules can't contain enums, and we don't store their ASTs because
   we only need to look at them to determine exports, which they can't
   control.*/

type def_map = hashmap<node_id, def>;
type ext_map = hashmap<def_id, [ident]>;
type exp_map = hashmap<str, @mutable [def]>;
type impl_map = hashmap<node_id, iscopes>;
type impl_cache = hashmap<def_id, option<@[@_impl]>>;

type env =
    {cstore: cstore::cstore,
     def_map: def_map,
     ast_map: ast_map::map,
     imports: hashmap<ast::node_id, import_state>,
     exp_map: exp_map,
     mod_map: hashmap<ast::node_id, @indexed_mod>,
     block_map: hashmap<ast::node_id, [glob_imp_def]>,
     ext_map: ext_map,
     impl_map: impl_map,
     impl_cache: impl_cache,
     ext_cache: ext_hash,
     used_imports: {mutable track: bool,
                    mutable data: [ast::node_id]},
     mutable reported: [{ident: str, sc: scope}],
     mutable ignored_imports: [node_id],
     mutable current_tp: option<uint>,
     mutable resolve_unexported: bool,
     sess: session};


// Used to distinguish between lookups from outside and from inside modules,
// since export restrictions should only be applied for the former.
enum dir { inside, outside, }

// There are two types of ns_value enum: "definitely a enum"; and "enum or
// other value". This is so that lookup can behave differently when looking up
// a variable name that's not yet in scope to check if it's already bound to a
// enum.
enum namespace { ns_val(enumness), ns_type, ns_module, }
enum enumness {
    definite_enum,
    value_or_enum
}

fn resolve_crate(sess: session, amap: ast_map::map, crate: @ast::crate) ->
   {def_map: def_map, exp_map: exp_map, impl_map: impl_map} {
    let e = create_env(sess, amap);
    map_crate(e, crate);
    resolve_imports(*e);
    check_exports(e);
    resolve_names(e, crate);
    resolve_impls(e, crate);
    // check_for_collisions must happen after resolve_names so we
    // don't complain if a pattern uses the same nullary enum twice
    check_for_collisions(e, *crate);
    if sess.opts.warn_unused_imports {
        check_unused_imports(e);
    }
    ret {def_map: e.def_map, exp_map: e.exp_map, impl_map: e.impl_map};
}

// Used by rustdoc
fn resolve_crate_reexports(sess: session, amap: ast_map::map,
                           crate: @ast::crate) -> exp_map {
    let e = create_env(sess, amap);
    map_crate(e, crate);
    resolve_imports(*e);
    check_exports(e);
    ret e.exp_map;
}

fn create_env(sess: session, amap: ast_map::map) -> @env {
    @{cstore: sess.cstore,
      def_map: new_int_hash(),
      ast_map: amap,
      imports: new_int_hash(),
      exp_map: new_str_hash(),
      mod_map: new_int_hash(),
      block_map: new_int_hash(),
      ext_map: new_def_hash(),
      impl_map: new_int_hash(),
      impl_cache: new_def_hash(),
      ext_cache: new_ext_hash(),
      used_imports: {mutable track: false, mutable data:  []},
      mutable reported: [],
      mutable ignored_imports: [],
      mutable current_tp: none,
      mutable resolve_unexported: false,
      sess: sess}
}

fn iter_export_paths(vi: ast::view_item, f: fn(vp: @ast::view_path)) {
    alt vi.node {
      ast::view_item_export(vps) {
        for vp in vps {
            f(vp);
        }
      }
      _ {}
    }
}

fn iter_import_paths(vi: ast::view_item, f: fn(vp: @ast::view_path)) {
    alt vi.node {
      ast::view_item_import(vps) {
        for vp in vps {
            f(vp);
        }
      }
      _ {}
    }
}

fn iter_effective_import_paths(vi: ast::view_item,
                               f: fn(vp: @ast::view_path)) {
    iter_import_paths(vi, f);
    iter_export_paths(vi) {|vp|
        alt vp.node {
          ast::view_path_simple(_, _, _) { }
          // FIXME: support uniform ident-list exports eventually;
          // at the moment they have half a meaning as reaching into
          // tags.
          ast::view_path_list(_, _, _) {}
          ast::view_path_glob(_,_) {
            f(vp);
          }
        }
    }
}

// Locate all modules and imports and index them, so that the next passes can
// resolve through them.
fn map_crate(e: @env, c: @ast::crate) {

    fn index_vi(e: @env, i: @ast::view_item, sc: scopes, _v: vt<scopes>) {
        iter_effective_import_paths(*i) { |vp|
            alt vp.node {
              ast::view_path_simple(name, path, id) {
                e.imports.insert(id, todo(id, name, path, vp.span, sc));
              }
              ast::view_path_glob(path, id) {
                e.imports.insert(id, is_glob(path, sc, vp.span));
              }
              ast::view_path_list(mod_path, idents, _) {
                for ident in idents {
                    let t = todo(ident.node.id, ident.node.name,
                                 @(*mod_path + [ident.node.name]),
                                 ident.span, sc);
                    e.imports.insert(ident.node.id, t);
                }
              }
            }
        }
    }

    fn path_from_scope(sc: scopes, n: str) -> str {
        let path = n + "::";
        list::iter(sc) {|s|
            alt s {
              scope_item(i) { path = i.ident + "::" + path; }
              _ {}
            }
        }
        path
    }

    fn index_i(e: @env, i: @ast::item, sc: scopes, v: vt<scopes>) {
        visit_item_with_scope(e, i, sc, v);
        alt i.node {
          ast::item_mod(md) {
            e.mod_map.insert(i.id,
                             @{m: some(md),
                               index: index_mod(md),
                               mutable glob_imports: [],
                               mutable globbed_exports: [],
                               glob_imported_names: new_str_hash(),
                               path: path_from_scope(sc, i.ident)});
          }
          ast::item_native_mod(nmd) {
            e.mod_map.insert(i.id,
                             @{m: none::<ast::_mod>,
                               index: index_nmod(nmd),
                               mutable glob_imports: [],
                               mutable globbed_exports: [],
                               glob_imported_names: new_str_hash(),
                               path: path_from_scope(sc, i.ident)});
          }
          _ { }
        }
    }

    // Note: a glob export works as an implict import, along with a
    // re-export of anything that was exported at the glob-target location.
    // So we wind up reusing the glob-import machinery when looking at
    // glob exports. They just do re-exporting in a later step.
    fn link_glob(e: @env, vi: @ast::view_item, sc: scopes, _v: vt<scopes>) {
        iter_effective_import_paths(*vi) { |vp|
            alt vp.node {
              ast::view_path_glob(path, _) {
                alt follow_import(*e, sc, *path, vp.span) {
                  some(imp) {
                    let glob = {def: imp, path: vp};
                    alt list::head(sc) {
                      scope_item(i) {
                        e.mod_map.get(i.id).glob_imports += [glob];
                      }
                      scope_block(b, _, _) {
                        let globs = alt e.block_map.find(b.node.id) {
                          some(globs) { globs + [glob] }
                          none { [glob] }
                        };
                        e.block_map.insert(b.node.id, globs);
                      }
                      scope_crate {
                        e.mod_map.get(ast::crate_node_id).glob_imports
                            += [glob];
                      }
                      _ { e.sess.span_bug(vi.span, "Unexpected scope in a \
                                                    glob import"); }
                    }
                  }
                  _ { }
                }
              }
              _ { }
            }
        }
    }

    // First, find all the modules, and index the names that they contain
    let v_map_mod =
        @{visit_view_item: bind index_vi(e, _, _, _),
          visit_item: bind index_i(e, _, _, _),
          visit_block: visit_block_with_scope
          with *visit::default_visitor::<scopes>()};
    visit::visit_crate(*c, top_scope(), visit::mk_vt(v_map_mod));

    // Register the top-level mod
    e.mod_map.insert(ast::crate_node_id,
                     @{m: some(c.node.module),
                       index: index_mod(c.node.module),
                       mutable glob_imports: [],
                       mutable globbed_exports: [],
                       glob_imported_names: new_str_hash(),
                       path: ""});

    // Next, assemble the links for globbed imports and exports.
    let v_link_glob =
        @{visit_view_item: bind link_glob(e, _, _, _),
          visit_block: visit_block_with_scope,
          visit_item: bind visit_item_with_scope(e, _, _, _)
          with *visit::default_visitor::<scopes>()};
    visit::visit_crate(*c, top_scope(), visit::mk_vt(v_link_glob));

}

fn resolve_imports(e: env) {
    e.used_imports.track = true;
    e.imports.values {|v|
        alt v {
          todo(node_id, name, path, span, scopes) {
            resolve_import(e, local_def(node_id), name, *path, span, scopes);
          }
          resolved(_, _, _, _, _, _) | is_glob(_, _, _) { }
          _ { e.sess.bug("Shouldn't see a resolving in resolve_imports"); }
        }
    };
    e.used_imports.track = false;
    e.sess.abort_if_errors();
}

fn check_unused_imports(e: @env) {
    e.imports.items {|k, v|
        alt v {
            resolved(_, _, _, _, name, sp) {
              if !vec::contains(e.used_imports.data, k) {
                e.sess.span_warn(sp, "unused import " + name);
              }
            }
            _ { }
        }
    };
}

fn resolve_capture_item(e: @env, sc: scopes, &&cap_item: @ast::capture_item) {
    let dcur = lookup_in_scope_strict(
        *e, sc, cap_item.span, cap_item.name, ns_val(value_or_enum));
    maybe_insert(e, cap_item.id, dcur);
}

fn maybe_insert(e: @env, id: node_id, def: option<def>) {
    alt def {
       some(df) { e.def_map.insert(id, df); }
       _ {}
    }
}

fn resolve_names(e: @env, c: @ast::crate) {
    e.used_imports.track = true;
    let v =
        @{visit_native_item: visit_native_item_with_scope,
          visit_item: bind visit_item_with_scope(e, _, _, _),
          visit_block: visit_block_with_scope,
          visit_decl: visit_decl_with_scope,
          visit_arm: visit_arm_with_scope,
          visit_local: bind visit_local_with_scope(e, _, _, _),
          visit_pat: bind walk_pat(e, _, _, _),
          visit_expr: bind walk_expr(e, _, _, _),
          visit_ty: bind walk_ty(e, _, _, _),
          visit_ty_params: bind walk_tps(e, _, _, _),
          visit_constr: bind walk_constr(e, _, _, _, _, _),
          visit_fn: bind visit_fn_with_scope(e, _, _, _, _, _, _, _)
          with *visit::default_visitor()};
    visit::visit_crate(*c, top_scope(), visit::mk_vt(v));
    e.used_imports.track = false;
    e.sess.abort_if_errors();

    fn walk_expr(e: @env, exp: @ast::expr, sc: scopes, v: vt<scopes>) {
        visit_expr_with_scope(exp, sc, v);
        alt exp.node {
          ast::expr_path(p) {
            maybe_insert(e, exp.id,
                         lookup_path_strict(*e, sc, exp.span, p.node,
                                            ns_val(value_or_enum)));
          }
          ast::expr_fn(_, _, _, cap_clause) {
            let rci = bind resolve_capture_item(e, sc, _);
            vec::iter(cap_clause.copies, rci);
            vec::iter(cap_clause.moves, rci);
          }
          _ { }
        }
    }
    fn walk_ty(e: @env, t: @ast::ty, sc: scopes, v: vt<scopes>) {
        visit::visit_ty(t, sc, v);
        alt t.node {
          ast::ty_path(p, id) {
            maybe_insert(e, id,
                         lookup_path_strict(*e, sc, t.span, p.node, ns_type));
          }
          _ { }
        }
    }
    fn walk_tps(e: @env, tps: [ast::ty_param], sc: scopes, v: vt<scopes>) {
        let outer_current_tp = e.current_tp, current = 0u;
        for tp in tps {
            e.current_tp = some(current);
            for bound in *tp.bounds {
                alt bound {
                  bound_iface(t) { v.visit_ty(t, sc, v); }
                  _ {}
                }
            }
            current += 1u;
        }
        e.current_tp = outer_current_tp;
    }
    fn walk_constr(e: @env, p: @ast::path, sp: span, id: node_id, sc: scopes,
                   _v: vt<scopes>) {
        maybe_insert(e, id, lookup_path_strict(*e, sc,
                         sp, p.node, ns_val(value_or_enum)));
    }
    fn walk_pat(e: @env, pat: @ast::pat, sc: scopes, v: vt<scopes>) {
        visit::visit_pat(pat, sc, v);
        alt pat.node {
          ast::pat_enum(p, _) {
            alt lookup_path_strict(*e, sc, p.span, p.node,
                                           ns_val(value_or_enum)) {
              some(fnd@ast::def_variant(_,_)) {
                e.def_map.insert(pat.id, fnd);
              }
              _ {
                e.sess.span_err(p.span,
                                "not a enum variant: " +
                                    ast_util::path_name(p));
              }
            }
          }
          /* Here we determine whether a given pat_ident binds a new
           variable a refers to a nullary enum. */
          ast::pat_ident(p, none) {
              alt lookup_in_scope(*e, sc, p.span, path_to_ident(p),
                                    ns_val(definite_enum)) {
                some(fnd@ast::def_variant(_,_)) {
                    e.def_map.insert(pat.id, fnd);
                }
                _ {
                    // Binds a var -- nothing needs to be done
                }
              }
          }
          _ { }
        }
    }
}


// Visit helper functions
fn visit_item_with_scope(e: @env, i: @ast::item, sc: scopes, v: vt<scopes>) {
    // Some magic here. Items with the !resolve_unexported attribute
    // cause us to consider every name to be exported when resolving their
    // contents. This is used to allow the test runner to run unexported
    // tests.
    let old_resolve_unexported = e.resolve_unexported;
    e.resolve_unexported |=
        attr::contains_name(attr::attr_metas(i.attrs),
                            "!resolve_unexported");

    let sc = cons(scope_item(i), @sc);
    alt i.node {
      ast::item_impl(tps, ifce, sty, methods) {
        visit::visit_ty_params(tps, sc, v);
        alt ifce { some(ty) { v.visit_ty(ty, sc, v); } _ {} }
        v.visit_ty(sty, sc, v);
        for m in methods {
            v.visit_ty_params(m.tps, sc, v);
            let msc = cons(scope_method(i.id, tps + m.tps), @sc);
            v.visit_fn(visit::fk_method(m.ident, []),
                       m.decl, m.body, m.span, m.id, msc, v);
        }
      }
      ast::item_iface(tps, methods) {
        visit::visit_ty_params(tps, sc, v);
        for m in methods {
            let msc = cons(scope_method(i.id, tps + m.tps), @sc);
            for a in m.decl.inputs { v.visit_ty(a.ty, msc, v); }
            v.visit_ty(m.decl.output, msc, v);
        }
      }
      ast::item_class(tps, members, ctor_id, ctor_decl, ctor_block) {
        visit::visit_ty_params(tps, sc, v);
        let class_scope = cons(scope_item(i), @sc);
        /* visit the constructor... */
        visit_fn_with_scope(e, visit::fk_item_fn(i.ident, tps), ctor_decl,
                            ctor_block, ctor_block.span, ctor_id,
                            class_scope, v);
        /* visit the items */
        for cm in members {
            alt cm.node.decl {
              class_method(i) { visit_item_with_scope(e, i, class_scope, v); }
              instance_var(_,t,_,_) { v.visit_ty(t, class_scope, v); }
            }
        }
      }
      _ { visit::visit_item(i, sc, v); }
    }

    e.resolve_unexported = old_resolve_unexported;
}

fn visit_native_item_with_scope(ni: @ast::native_item, sc: scopes,
                                v: vt<scopes>) {
    visit::visit_native_item(ni, cons(scope_native_item(ni), @sc), v);
}

fn visit_fn_with_scope(e: @env, fk: visit::fn_kind, decl: ast::fn_decl,
                       body: ast::blk, sp: span,
                       id: node_id, sc: scopes, v: vt<scopes>) {
    // is this a main fn declaration?
    alt fk {
      visit::fk_item_fn(nm, _) {
        if is_main_name([ast_map::path_name(nm)]) &&
           !e.sess.building_library {
            // This is a main function -- set it in the session
            // as the main ID
            e.sess.main_fn = some((id, sp));
        }
      }
      _ { /* fallthrough */ }
    }

    // here's where we need to set up the mapping
    // for f's constrs in the table.
    for c: @ast::constr in decl.constraints { resolve_constr(e, c, sc, v); }
    let scope = alt fk {
      visit::fk_item_fn(_, tps) | visit::fk_res(_, tps) |
      visit::fk_method(_, tps) { scope_bare_fn(decl, id, tps) }
      visit::fk_anon(ast::proto_bare) { scope_bare_fn(decl, id, []) }
      visit::fk_anon(_) | visit::fk_fn_block { scope_fn_expr(decl, id, []) }
    };

    visit::visit_fn(fk, decl, body, sp, id, cons(scope, @sc), v);
}

fn visit_block_with_scope(b: ast::blk, sc: scopes, v: vt<scopes>) {
    let pos = @mutable 0u, loc = @mutable 0u;
    let block_sc = cons(scope_block(b, pos, loc), @sc);
    for vi in b.node.view_items { v.visit_view_item(vi, block_sc, v); }
    for stmt in b.node.stmts {
        v.visit_stmt(stmt, block_sc, v);;
        *pos += 1u;;
        *loc = 0u;
    }
    visit::visit_expr_opt(b.node.expr, block_sc, v);
}

fn visit_decl_with_scope(d: @decl, sc: scopes, v: vt<scopes>) {
    let loc_pos = alt list::head(sc) {
      scope_block(_, _, pos) { pos }
      _ { @mutable 0u }
    };
    alt d.node {
      decl_local(locs) {
        for loc in locs { v.visit_local(loc, sc, v);; *loc_pos += 1u; }
      }
      decl_item(it) { v.visit_item(it, sc, v); }
    }
}

fn visit_arm_with_scope(a: ast::arm, sc: scopes, v: vt<scopes>) {
    for p: @pat in a.pats { v.visit_pat(p, sc, v); }
    let sc_inner = cons(scope_arm(a), @sc);
    visit::visit_expr_opt(a.guard, sc_inner, v);
    v.visit_block(a.body, sc_inner, v);
}

fn visit_expr_with_scope(x: @ast::expr, sc: scopes, v: vt<scopes>) {
    alt x.node {
      ast::expr_for(decl, coll, blk) {
        let new_sc = cons(scope_loop(decl), @sc);
        v.visit_expr(coll, sc, v);
        v.visit_local(decl, new_sc, v);
        v.visit_block(blk, new_sc, v);
      }
      _ { visit::visit_expr(x, sc, v); }
    }
}

// This is only for irrefutable patterns (e.g. ones that appear in a let)
// So if x occurs, and x is already known to be a enum, that's always an error
fn visit_local_with_scope(e: @env, loc: @local, sc:scopes, v:vt<scopes>) {
    // Check whether the given local has the same name as a enum that's
    // in scope
    // We disallow this, in order to make alt patterns consisting of
    // a single identifier unambiguous (does the pattern "foo" refer
    // to enum foo, or is it binding a new name foo?)
    alt loc.node.pat.node {
      pat_ident(an_ident,_) {
          // Be sure to pass definite_enum to lookup_in_scope so that
          // if this is a name that's being shadowed, we don't die
          alt lookup_in_scope(*e, sc, loc.span,
                 path_to_ident(an_ident), ns_val(definite_enum)) {
              some(ast::def_variant(enum_id,variant_id)) {
                  // Declaration shadows a enum that's in scope.
                  // That's an error.
                  e.sess.span_err(loc.span,
                    #fmt("Declaration of %s shadows a enum that's in scope",
                         path_to_ident(an_ident)));
                  }
              _ {}
          }
      }
      _ {}
    }
    visit::visit_local(loc, sc, v);
}


fn follow_import(e: env, sc: scopes, path: [ident], sp: span) ->
   option<def> {
    let path_len = vec::len(path);
    let dcur = lookup_in_scope_strict(e, sc, sp, path[0], ns_module);
    let i = 1u;
    while true {
       alt dcur {
          some(dcur_def) {
            if i == path_len { break; }
            dcur =
                lookup_in_mod_strict(e, dcur_def, sp, path[i],
                                 ns_module, outside);
            i += 1u;
          }
          _ { break; }
       }
    }
    if i == path_len {
       alt dcur {
          some(ast::def_mod(_)) | some(ast::def_native_mod(_)) { ret dcur; }
          _ {
            e.sess.span_err(sp, str::connect(path, "::") +
                            " does not name a module.");
            ret none;
          }
        }
    } else { ret none; }
}

fn resolve_constr(e: @env, c: @ast::constr, sc: scopes, _v: vt<scopes>) {
    alt lookup_path_strict(*e, sc, c.span, c.node.path.node,
                           ns_val(value_or_enum)) {
       some(d@ast::def_fn(_,ast::pure_fn)) {
         e.def_map.insert(c.node.id, d);
       }
       _ {
           let s = path_to_str(c.node.path);
           e.sess.span_err(c.span, #fmt("%s is not declared pure. Try \
             `pure fn %s` instead of `fn %s`.", s, s, s));
       }
    }
}

// Import resolution
fn resolve_import(e: env, defid: ast::def_id, name: ast::ident,
                  ids: [ast::ident], sp: codemap::span, sc: scopes) {
    fn register(e: env, id: node_id, cx: ctxt, sp: codemap::span,
                name: ast::ident, lookup: fn(namespace) -> option<def>,
                impls: [@_impl]) {
        let val = lookup(ns_val(value_or_enum)), typ = lookup(ns_type),
            md = lookup(ns_module);
        if is_none(val) && is_none(typ) && is_none(md) &&
           vec::len(impls) == 0u {
            unresolved_err(e, cx, sp, name, "import");
        } else {
            e.imports.insert(id, resolved(val, typ, md, @impls, name, sp));
        }
    }
    // Temporarily disable this import and the imports coming after during
    // resolution of this import.
    fn find_imports_after(e: env, id: node_id, sc: scopes) -> [node_id] {
        fn lst(my_id: node_id, vis: [@view_item]) -> [node_id] {
            let imports = [], found = false;
            for vi in vis {
                iter_effective_import_paths(*vi) {|vp|
                    alt vp.node {
                      view_path_simple(_, _, id)
                      | view_path_glob(_, id) {
                        if id == my_id { found = true; }
                        if found { imports += [id]; }
                      }
                      view_path_list(_, ids, _) {
                        for id in ids {
                            if id.node.id == my_id { found = true; }
                            if found { imports += [id.node.id]; }
                        }
                      }
                    }
                }
            }
            imports
        }
        alt sc {
          cons(scope_item(@{node: item_mod(m), _}), _) {
            lst(id, m.view_items)
          }
          cons(scope_item(@{node: item_native_mod(m), _}), _) {
            lst(id, m.view_items)
          }
          cons(scope_block(b, _, _), _) {
            lst(id, b.node.view_items)
          }
          cons(scope_crate, _) {
            lst(id,
                option::get(e.mod_map.get(ast::crate_node_id).m).view_items)
          }
          _ {
              e.sess.bug("find_imports_after: nil or unexpected scope");
          }
        }
    }
    // This function has cleanup code at the end. Do not return without going
    // through that.
    e.imports.insert(defid.node, resolving(sp));
    let ignored = find_imports_after(e, defid.node, sc);
    e.ignored_imports <-> ignored;
    let n_idents = vec::len(ids);
    let end_id = ids[n_idents - 1u];
    if n_idents == 1u {
        register(e, defid.node, in_scope(sc), sp, name,
                 {|ns| lookup_in_scope(e, sc, sp, end_id, ns) }, []);
    } else {
        alt lookup_in_scope(e, sc, sp, ids[0], ns_module) {
          none {
            unresolved_err(e, in_scope(sc), sp, ids[0], ns_name(ns_module));
          }
          some(dcur_) {
            let dcur = dcur_, i = 1u;
            while true {
                if i == n_idents - 1u {
                    let impls = [];
                    find_impls_in_mod(e, dcur, impls, some(end_id));
                    register(e, defid.node, in_mod(dcur), sp, name, {|ns|
                        lookup_in_mod(e, dcur, sp, end_id, ns, outside)
                    }, impls);
                    break;
                } else {
                    dcur = alt lookup_in_mod(e, dcur, sp, ids[i], ns_module,
                                             outside) {
                      some(dcur) { dcur }
                      none {
                        unresolved_err(e, in_mod(dcur), sp, ids[i],
                                       ns_name(ns_module));
                        break;
                      }
                    };
                    i += 1u;
                }
            }
          }
        }
    }
    e.ignored_imports <-> ignored;
    // If we couldn't resolve the import, don't leave it in a partially
    // resolved state, to avoid having it reported later as a cyclic
    // import
    alt e.imports.find(defid.node) {
      some(resolving(sp)) {
        e.imports.insert(defid.node, resolved(none, none, none, @[], "", sp));
      }
      _ { }
    }
}


// Utilities
fn ns_name(ns: namespace) -> str {
    alt ns {
      ns_type { "typename" }
      ns_val(v) {
          alt (v) {
              value_or_enum { "name" }
              definite_enum    { "enum" }
          }
      }
      ns_module { "modulename" }
    }
}

enum ctxt { in_mod(def), in_scope(scopes), }

fn unresolved_err(e: env, cx: ctxt, sp: span, name: ident, kind: str) {
    fn find_fn_or_mod_scope(sc: scopes) -> option<scope> {
        let sc = sc;
        while true {
            alt sc {
              cons(cur, rest) {
                alt cur {
                  scope_crate | scope_bare_fn(_, _, _) |
                  scope_fn_expr(_, _, _) |
                  scope_item(@{node: ast::item_mod(_), _}) {
                    ret some(cur);
                  }
                  _ { sc = *rest; }
                }
              }
              _ { ret none; }
            }
        }
        fail;
    }
    let path = name;
    alt cx {
      in_scope(sc) {
        alt find_fn_or_mod_scope(sc) {
          some(err_scope) {
            for rs: {ident: str, sc: scope} in e.reported {
                if str::eq(rs.ident, name) && err_scope == rs.sc { ret; }
            }
            e.reported += [{ident: name, sc: err_scope}];
          }
          _ {}
        }
      }
      in_mod(def) {
        let did = def_id_of_def(def);
        if did.crate == ast::local_crate {
            path = e.mod_map.get(did.node).path + path;
        } else if did.node != ast::crate_node_id {
            let paths = e.ext_map.get(did);
            if vec::len(paths) > 0u {
                path = str::connect(paths, "::") + "::" + path;
            }
        }
      }
    }
    e.sess.span_err(sp, mk_unresolved_msg(path, kind));
}

fn unresolved_fatal(e: env, sp: span, id: ident, kind: str) -> ! {
    e.sess.span_fatal(sp, mk_unresolved_msg(id, kind));
}

fn mk_unresolved_msg(id: ident, kind: str) -> str {
    ret #fmt["unresolved %s: %s", kind, id];
}

// Lookup helpers
fn lookup_path_strict(e: env, sc: scopes, sp: span, pth: ast::path_,
                      ns: namespace) -> option<def> {
    let n_idents = vec::len(pth.idents);
    let headns = if n_idents == 1u { ns } else { ns_module };

    let first_scope = if pth.global { top_scope() } else { sc };

    let dcur_ =
        lookup_in_scope_strict(e, first_scope, sp, pth.idents[0], headns);

    alt dcur_ {
      none { ret none; }
      some(dcur__) {
         let i = 1u;
         let dcur = dcur__;
         while i < n_idents {
            let curns = if n_idents == i + 1u { ns } else { ns_module };
            alt lookup_in_mod_strict(e, dcur, sp, pth.idents[i],
                                 curns, outside) {
               none { break; }
               some(thing) { dcur = thing; }
            }
            i += 1u;
         }
         ret some(dcur);
      }
    }
}

fn lookup_in_scope_strict(e: env, sc: scopes, sp: span, name: ident,
                          ns: namespace) -> option<def> {
    alt lookup_in_scope(e, sc, sp, name, ns) {
      none {
        unresolved_err(e, in_scope(sc), sp, name, ns_name(ns));
        ret none;
      }
      some(d) { ret some(d); }
    }
}

fn scope_is_fn(sc: scope) -> bool {
    ret alt sc {
      scope_bare_fn(_, _, _) | scope_native_item(_) { true }
      _ { false }
    };
}

// Returns:
//   none - does not close
//   some(node_id) - closes via the expr w/ node_id
fn scope_closes(sc: scope) -> option<node_id> {
    alt sc {
      scope_fn_expr(_, node_id, _) { some(node_id) }
      _ { none }
    }
}

fn def_is_local(d: def) -> bool {
    alt d {
      ast::def_arg(_, _) | ast::def_local(_) | ast::def_binding(_) |
      ast::def_upvar(_, _, _) { true }
      _ { false }
    }
}

fn def_is_self(d: def) -> bool {
    alt d {
      ast::def_self(_) { true }
      _ { false }
    }
}

fn def_is_ty_arg(d: def) -> bool {
    ret alt d { ast::def_ty_param(_, _) { true } _ { false } };
}

fn lookup_in_scope(e: env, sc: scopes, sp: span, name: ident, ns: namespace)
   -> option<def> {

    fn in_scope(e: env, sp: span, name: ident, s: scope, ns: namespace) ->
       option<def> {
        alt s {
          scope_toplevel {
            if ns == ns_type {
                ret some(ast::def_prim_ty(alt name {
                  "bool" { ast::ty_bool }
                  "int" { ast::ty_int(ast::ty_i) }
                  "uint" { ast::ty_uint(ast::ty_u) }
                  "float" { ast::ty_float(ast::ty_f) }
                  "str" { ast::ty_str }
                  "char" { ast::ty_int(ast::ty_char) }
                  "i8" { ast::ty_int(ast::ty_i8) }
                  "i16" { ast::ty_int(ast::ty_i16) }
                  "i32" { ast::ty_int(ast::ty_i32) }
                  "i64" { ast::ty_int(ast::ty_i64) }
                  "u8" { ast::ty_uint(ast::ty_u8) }
                  "u16" { ast::ty_uint(ast::ty_u16) }
                  "u32" { ast::ty_uint(ast::ty_u32) }
                  "u64" { ast::ty_uint(ast::ty_u64) }
                  "f32" { ast::ty_float(ast::ty_f32) }
                  "f64" { ast::ty_float(ast::ty_f64) }
                  _ { ret none; }
                }));
            }
          }
          scope_crate {
            ret lookup_in_local_mod(e, ast::crate_node_id, sp,
                                    name, ns, inside);
          }
          scope_item(it) {
            alt it.node {
              ast::item_impl(tps, _, _, _) {
                if ns == ns_type { ret lookup_in_ty_params(e, name, tps); }
              }
              ast::item_enum(_, tps) | ast::item_ty(_, tps) {
                if ns == ns_type { ret lookup_in_ty_params(e, name, tps); }
              }
              ast::item_iface(tps, _) {
                if ns == ns_type {
                    if name == "self" {
                        ret some(def_self(it.id));
                    }
                    ret lookup_in_ty_params(e, name, tps);
                }
              }
              ast::item_mod(_) {
                ret lookup_in_local_mod(e, it.id, sp, name, ns, inside);
              }
              ast::item_native_mod(m) {
                ret lookup_in_local_native_mod(e, it.id, sp, name, ns);
              }
              ast::item_class(tps, members, ctor_id, _, _) {
                  if ns == ns_type {
                    ret lookup_in_ty_params(e, name, tps);
                  }
                  if ns == ns_val(value_or_enum) && name == it.ident {
                      ret some(ast::def_fn(local_def(ctor_id),
                                           ast::impure_fn));
                  }
                  if ns == ns_val(value_or_enum) {
                          ret lookup_in_class(local_def(it.id),
                                              members, name);
                  }
                  // FIXME: AST allows other items to appear in a class,
                  // but that might not be wise
              }
              _ { }
            }
          }
          scope_method(id, tps) {
            if (name == "self" && ns == ns_val(value_or_enum)) {
                ret some(ast::def_self(id));
            } else if ns == ns_type {
                ret lookup_in_ty_params(e, name, tps);
            }
          }
          scope_native_item(it) {
            alt it.node {
              ast::native_item_fn(decl, ty_params) {
                ret lookup_in_fn(e, name, decl, ty_params, ns);
              }
              _ {
                  e.sess.span_bug(it.span, "lookup_in_scope: \
                    scope_native_item doesn't refer to a native item");
              }
            }
          }
          scope_bare_fn(decl, _, ty_params) |
          scope_fn_expr(decl, _, ty_params) {
            ret lookup_in_fn(e, name, decl, ty_params, ns);
          }
          scope_loop(local) {
            if ns == ns_val(value_or_enum) {
                alt lookup_in_pat(e, name, local.node.pat) {
                  some(nid) { ret some(ast::def_binding(nid)); }
                  _ { }
                }
            }
          }
          scope_block(b, pos, loc) {
            ret lookup_in_block(e, name, sp, b.node, *pos, *loc, ns);
          }
          scope_arm(a) {
            if ns == ns_val(value_or_enum) {
                alt lookup_in_pat(e, name, a.pats[0]) {
                  some(nid) { ret some(ast::def_binding(nid)); }
                  _ { ret none; }
                }
            }
          }
        }
        ret none;
    }
    let left_fn = false;
    let closing = [];
    // Used to determine whether self is in scope
    let left_fn_level2 = false;
    let sc = sc;
    while true {
        alt copy sc {
          nil { ret none; }
          cons(hd, tl) {
              alt in_scope(e, sp, name, hd, ns) {
               some(df_) {
                 let df = df_;
                 let local = def_is_local(df), self_scope = def_is_self(df);
                 if left_fn && local || left_fn_level2 && self_scope
                   || scope_is_fn(hd) && left_fn && def_is_ty_arg(df) {
                    let msg = alt ns {
                      ns_type {
                        "attempt to use a type argument out of scope"
                      }
                      ns_val(v) {
                          alt(v) {
                            /* If we were looking for a enum, at this point
                               we know it's bound to a non-enum value, and
                               we can return none instead of failing */
                            definite_enum { ret none; }
                            _ { "attempted dynamic environment-capture" }
                          }
                      }
                      _ { "attempted dynamic environment-capture" }
                    };
                    e.sess.span_fatal(sp, msg);
                } else if local || self_scope {
                    let i = vec::len(closing);
                    while i > 0u {
                        i -= 1u;
                        #debug["name=%s df=%?", name, df];
                        assert def_is_local(df) || def_is_self(df);
                        let df_id = def_id_of_def(df).node;
                        df = ast::def_upvar(df_id, @df, closing[i]);
                    }
                }
                ret some(df);
            }
                      _ {}
                  }
             if left_fn {
                left_fn_level2 = true;
            } else if ns != ns_module {
                left_fn = scope_is_fn(hd);
                alt scope_closes(hd) {
                  some(node_id) { closing += [node_id]; }
                  _ { }
                }
            }
            sc = *tl;
          }
        }
    }
    e.sess.bug("reached unreachable code in lookup_in_scope"); // sigh
}

fn lookup_in_ty_params(e: env, name: ident, ty_params: [ast::ty_param])
    -> option<def> {
    let n = 0u;
    for tp: ast::ty_param in ty_params {
        if str::eq(tp.ident, name) && alt e.current_tp {
            some(cur) { n < cur } none { true }
        } { ret some(ast::def_ty_param(local_def(tp.id), n)); }
        n += 1u;
    }
    ret none;
}

fn lookup_in_pat(e: env, name: ident, pat: @ast::pat) -> option<node_id> {
    let found = none;

    pat_util::pat_bindings(e.def_map, pat) {|p_id, _sp, n|
        if str::eq(path_to_ident(n), name)
                    { found = some(p_id); }
    };
    ret found;
}

fn lookup_in_fn(e: env, name: ident, decl: ast::fn_decl,
                ty_params: [ast::ty_param],
                ns: namespace) -> option<def> {
    alt ns {
      ns_val(value_or_enum) {
        for a: ast::arg in decl.inputs {
            if str::eq(a.ident, name) {
                ret some(ast::def_arg(a.id, a.mode));
            }
        }
        ret none;
      }
      ns_type { ret lookup_in_ty_params(e, name, ty_params); }
      _ { ret none; }
    }
}

/*
   FIXME: not sure about this code. maybe this should be handled
   using the mod_index stuff
 */
fn lookup_in_class(parent_id: def_id,
                   members: [@class_item], name: ident)
   -> option<def> {
    for m in members {
      alt m.node.decl {
        instance_var(v_name,_,_,id) {
            if v_name == name {
              ret some(def_class_field(parent_id, local_def(id)));
            }
        }
        class_method(i) {
            if i.ident == name {
              ret some(def_class_method(parent_id, local_def(i.id)));
            }
        }
      }
    }
    ret none;
}

fn lookup_in_block(e: env, name: ident, sp: span, b: ast::blk_, pos: uint,
                   loc_pos: uint, ns: namespace) -> option<def> {

    let i = vec::len(b.stmts);
    while i > 0u {
        i -= 1u;
        let st = b.stmts[i];
        alt st.node {
          ast::stmt_decl(d, _) {
            alt d.node {
              ast::decl_local(locs) {
                if i <= pos {
                    let j = vec::len(locs);
                    while j > 0u {
                        j -= 1u;
                        let loc = locs[j];
                        if ns == ns_val(value_or_enum)
                                     && (i < pos || j < loc_pos) {
                            alt lookup_in_pat(e, name, loc.node.pat) {
                              some(nid) {
                                ret some(ast::def_local(nid));
                              }
                              _ { }
                            }
                        }
                    }
                }
              }
              ast::decl_item(it) {
                alt it.node {
                  ast::item_enum(variants, _) {
                    if ns == ns_type {
                        if str::eq(it.ident, name) {
                            ret some(ast::def_ty(local_def(it.id)));
                        }
                    } else {
                        alt ns {
                           ns_val(_) {
                               for v: ast::variant in variants {
                                  if str::eq(v.node.name, name) {
                                     let i = v.node.id;
                                     ret some(ast::def_variant
                                        (local_def(it.id), local_def(i)));
                                  }
                               }
                          }
                           _ {}
                        }
                    }
                  }
                  _ {
                    if str::eq(it.ident, name) {
                        let found = found_def_item(it, ns);
                        if !is_none(found) {
                            ret found;
                        }
                    }
                  }
                }
              }
            }
          }
          _ { }
        }
    }
    for vi in b.view_items {

        let is_import = false;
        alt vi.node {
          ast::view_item_import(_) { is_import = true; }
          _ {}
        }

        alt vi.node {

          ast::view_item_import(vps) | ast::view_item_export(vps) {
            for vp in vps {
                alt vp.node {
                  ast::view_path_simple(ident, _, id) {
                    if is_import && name == ident {
                        ret lookup_import(e, local_def(id), ns);
                    }
                  }

                  ast::view_path_list(path, idents, _) {
                    for ident in idents {
                        if name == ident.node.name {
                            let def = local_def(ident.node.id);
                            ret lookup_import(e, def, ns);
                        }
                    }
                  }

                  ast::view_path_glob(_, _) {
                    alt e.block_map.find(b.id) {
                      some(globs) {
                        let found = lookup_in_globs(e, globs, sp, name,
                                                    ns, inside);
                        if found != none {
                            ret found;
                        }
                      }
                      _ {}
                    }
                  }
                }
            }
          }
          _ { e.sess.span_bug(vi.span, "Unexpected view_item in block"); }
        }
    }
    ret none;
}

fn found_def_item(i: @ast::item, ns: namespace) -> option<def> {
    alt i.node {
      ast::item_const(_, _) {
        if ns == ns_val(value_or_enum) {
            ret some(ast::def_const(local_def(i.id))); }
      }
      ast::item_fn(decl, _, _) {
        if ns == ns_val(value_or_enum) {
            ret some(ast::def_fn(local_def(i.id), decl.purity));
        }
      }
      ast::item_mod(_) {
        if ns == ns_module { ret some(ast::def_mod(local_def(i.id))); }
      }
      ast::item_native_mod(_) {
        if ns == ns_module { ret some(ast::def_native_mod(local_def(i.id))); }
      }
      ast::item_ty(_, _) | item_iface(_, _) | item_enum(_, _) {
        if ns == ns_type { ret some(ast::def_ty(local_def(i.id))); }
      }
      ast::item_res(_, _, _, _, ctor_id) {
        alt ns {
          ns_val(value_or_enum) {
            ret some(ast::def_fn(local_def(ctor_id), ast::impure_fn));
          }
          ns_type { ret some(ast::def_ty(local_def(i.id))); }
          _ { }
        }
      }
      ast::item_class(_, _, _, _, _) {
          if ns == ns_type {
            ret some(ast::def_class(local_def(i.id)));
          }
      }
      ast::item_impl(_,_,_,_) { /* ??? */ }
    }
    ret none;
}

fn lookup_in_mod_strict(e: env, m: def, sp: span, name: ident,
                        ns: namespace, dr: dir) -> option<def> {
    alt lookup_in_mod(e, m, sp, name, ns, dr) {
      none {
        unresolved_err(e, in_mod(m), sp, name, ns_name(ns));
        ret none;
      }
      some(d) { ret some(d); }
    }
}

fn lookup_in_mod(e: env, m: def, sp: span, name: ident, ns: namespace,
                 dr: dir) -> option<def> {
    let defid = def_id_of_def(m);
    if defid.crate != ast::local_crate {
        // examining a module in an external crate
        let cached = e.ext_cache.find({did: defid, ident: name, ns: ns});
        if !is_none(cached) { ret cached; }
        let path = [name];
        if defid.node != ast::crate_node_id {
            path = cstore::get_path(e.cstore, defid) + path;
        }
        alt lookup_external(e, defid.crate, path, ns) {
           some(df) {
               e.ext_cache.insert({did: defid, ident: name, ns: ns}, df);
               ret some(df);
           }
           _ { ret none; }
        }
    }
    alt m {
      ast::def_mod(defid) {
        ret lookup_in_local_mod(e, defid.node, sp, name, ns, dr);
      }
      ast::def_native_mod(defid) {
        ret lookup_in_local_native_mod(e, defid.node, sp, name, ns);
      }
      _ {
          // Precondition
          e.sess.span_bug(sp, "lookup_in_mod was passed a non-mod def");
      }
    }
}

fn found_view_item(e: env, id: node_id) -> option<def> {
    alt cstore::find_use_stmt_cnum(e.cstore, id) {
      some(cnum) {
        some(ast::def_mod({crate: cnum, node: ast::crate_node_id}))
      }
      none {
        // This can happen if we didn't load external crate info.
        // Rustdoc depends on this.
        none
      }
    }
}

fn lookup_import(e: env, defid: def_id, ns: namespace) -> option<def> {
    // Imports are simply ignored when resolving themselves.
    if vec::contains(e.ignored_imports, defid.node) { ret none; }
    alt e.imports.get(defid.node) {
      todo(node_id, name, path, span, scopes) {
        resolve_import(e, local_def(node_id), name, *path, span, scopes);
        ret lookup_import(e, defid, ns);
      }
      resolving(sp) {
        e.sess.span_err(sp, "cyclic import");
        ret none;
      }
      resolved(val, typ, md, _, _, _) {
        if e.used_imports.track {
            e.used_imports.data += [defid.node];
        }
        ret alt ns { ns_val(_) { val } ns_type { typ }
            ns_module { md } };
      }
      is_glob(_,_,_) {
          e.sess.bug("lookup_import: can't handle is_glob");
      }
    }
}

fn lookup_in_local_native_mod(e: env, node_id: node_id, sp: span, id: ident,
                              ns: namespace) -> option<def> {
    ret lookup_in_local_mod(e, node_id, sp, id, ns, inside);
}

fn is_exported(e: env, i: ident, m: @indexed_mod) -> bool {

    alt m.m {
      some(_m) {
        if ast_util::is_exported(i, _m) { ret true; }
      }
      _ {}
    }

    ret vec::contains(m.globbed_exports, i)
        || e.resolve_unexported;
}

fn lookup_in_local_mod(e: env, node_id: node_id, sp: span, id: ident,
                       ns: namespace, dr: dir) -> option<def> {
    let info = e.mod_map.get(node_id);
    if dr == outside && !is_exported(e, id, info) {
        // if we're in a native mod, then dr==inside, so info.m is some _mod
        ret none; // name is not visible
    }
    alt info.index.find(id) {
      none { }
      some(lst) {
        let found = list::find(lst, bind lookup_in_mie(e, _, ns));
        if !is_none(found) {
            ret found;
        }
      }
    }
    // not local or explicitly imported; try globs:
    ret lookup_glob_in_mod(e, info, sp, id, ns, outside);
}

fn lookup_in_globs(e: env, globs: [glob_imp_def], sp: span, id: ident,
                   ns: namespace, dr: dir) -> option<def> {
    fn lookup_in_mod_(e: env, def: glob_imp_def, sp: span, name: ident,
                      ns: namespace, dr: dir) -> option<glob_imp_def> {
        alt def.path.node {

          ast::view_path_glob(_, id) {
            if vec::contains(e.ignored_imports, id) { ret none; }
          }

          _ {
            e.sess.span_bug(sp, "lookup_in_globs: not a glob");
          }
        }
        alt lookup_in_mod(e, def.def, sp, name, ns, dr) {
          some(d) { option::some({def: d, path: def.path}) }
          none { none }
        }
    }
    let matches = vec::filter_map(copy globs,
                                  bind lookup_in_mod_(e, _, sp, id, ns, dr));
    if vec::len(matches) == 0u {
        ret none;
        }
    else if vec::len(matches) == 1u || ns == ns_val(definite_enum) {
        ret some(matches[0].def);
    } else {
        for match: glob_imp_def in matches {
            let sp = match.path.span;
            e.sess.span_note(sp, #fmt["'%s' is imported here", id]);
        }
        e.sess.span_fatal(sp, "'" + id + "' is glob-imported from" +
                          " multiple different modules.");
    }
}

fn lookup_glob_in_mod(e: env, info: @indexed_mod, sp: span, id: ident,
                      wanted_ns: namespace, dr: dir) -> option<def> {
    // since we don't know what names we have in advance,
    // absence takes the place of todo()
    if !info.glob_imported_names.contains_key(id) {
        info.glob_imported_names.insert(id, glob_resolving(sp));
        // kludge
        let val_ns = if wanted_ns == ns_val(definite_enum) {
            ns_val(definite_enum)
        } else {
            ns_val(value_or_enum)
        };
        let globs = info.glob_imports;
        let val = lookup_in_globs(e, globs, sp, id, val_ns, dr);
        let typ = lookup_in_globs(e, globs, sp, id, ns_type, dr);
        let md = lookup_in_globs(e, globs, sp, id, ns_module, dr);
        info.glob_imported_names.insert(id, glob_resolved(val, typ, md));
    }
    alt info.glob_imported_names.get(id) {
      glob_resolving(sp) {
          ret none;
      }
      glob_resolved(val, typ, md) {
        ret alt wanted_ns {
                ns_val(_) { val }
                ns_type { typ }
                ns_module { md }
        };
      }
    }
}

fn lookup_in_mie(e: env, mie: mod_index_entry, ns: namespace) ->
   option<def> {
    alt mie {
      mie_view_item(_, id, _) {
         if ns == ns_module { ret found_view_item(e, id); }
      }
      mie_import_ident(id, _) { ret lookup_import(e, local_def(id), ns); }
      mie_item(item) { ret found_def_item(item, ns); }
      mie_enum_variant(variant_idx, variants, parent_id, parent_span) {
         alt ns {
            ns_val(_) {
               let vid = variants[variant_idx].node.id;
               ret some(ast::def_variant(local_def(parent_id),
                                        local_def(vid)));
            }
            _ { ret none; }
         }
      }
      mie_native_item(native_item) {
        alt native_item.node {
          ast::native_item_fn(decl, _) {
            if ns == ns_val(value_or_enum) {
                ret some(ast::def_fn(local_def(native_item.id),
                                     decl.purity));
            }
          }
        }
      }
      mie_class_item(parent_id, class_item) {
          alt class_item.node.decl {
              instance_var(_,_,_,id) {
                  ret some(ast::def_class_field(local_def(parent_id),
                                                local_def(id)));
              }
              class_method(it) {
                  ret some(ast::def_class_method(local_def(parent_id),
                                                 local_def(it.id)));
              }
          }
      }
    }
    ret none;
}


// Module indexing
fn add_to_index(index: hashmap<ident, list<mod_index_entry>>, id: ident,
                ent: mod_index_entry) {
    alt index.find(id) {
      none { index.insert(id, cons(ent, @nil::<mod_index_entry>)); }
      some(prev) { index.insert(id, cons(ent, @prev)); }
    }
}

fn index_view_items(view_items: [@ast::view_item],
                    index: hashmap<ident, list<mod_index_entry>>) {
    for vi in view_items {
        alt vi.node {
          ast::view_item_use(ident, _, id) {
           add_to_index(index, ident, mie_view_item(ident, id, vi.span));
          }
          _ {}
        }

        iter_effective_import_paths(*vi) {|vp|
            alt vp.node {
              ast::view_path_simple(ident, _, id) {
                add_to_index(index, ident, mie_import_ident(id, vp.span));
              }
              ast::view_path_list(_, idents, _) {
                for ident in idents {
                    add_to_index(index, ident.node.name,
                                 mie_import_ident(ident.node.id,
                                                  ident.span));
                }
              }

              // globbed imports have to be resolved lazily.
              ast::view_path_glob(_, _) {}
            }
        }
    }
}

fn index_mod(md: ast::_mod) -> mod_index {
    let index = new_str_hash::<list<mod_index_entry>>();

    index_view_items(md.view_items, index);

    for it: @ast::item in md.items {
        alt it.node {
          ast::item_const(_, _) | ast::item_fn(_, _, _) | ast::item_mod(_) |
          ast::item_native_mod(_) | ast::item_ty(_, _) |
          ast::item_res(_, _, _, _, _) |
          ast::item_impl(_, _, _, _) | ast::item_iface(_, _) {
            add_to_index(index, it.ident, mie_item(it));
          }
          ast::item_enum(variants, _) {
            add_to_index(index, it.ident, mie_item(it));
            let variant_idx: uint = 0u;
            for v: ast::variant in variants {
                add_to_index(index, v.node.name,
                             mie_enum_variant(variant_idx, variants,
                                             it.id, it.span));
                variant_idx += 1u;
            }
          }
          ast::item_class(tps, items, ctor_id, ctor_decl, ctor_body) {
              // add the class name itself
              add_to_index(index, it.ident, mie_item(it));
              // add the constructor decl
              add_to_index(index, it.ident,
                           mie_item(@{ident: it.ident, attrs: [],
                                       id: ctor_id,
                                       node:
                                         item_fn(ctor_decl, tps, ctor_body),
                                       span: ctor_body.span}));
              // add the members
              for ci in items {
                 add_to_index(index, class_item_ident(ci),
                              mie_class_item(it.id, ci));
              }
          }
        }
    }
    ret index;
}


fn index_nmod(md: ast::native_mod) -> mod_index {
    let index = new_str_hash::<list<mod_index_entry>>();

    index_view_items(md.view_items, index);

    for it: @ast::native_item in md.items {
        add_to_index(index, it.ident, mie_native_item(it));
    }
    ret index;
}


// External lookups
fn ns_for_def(d: def) -> namespace {
    alt d {
      ast::def_variant(_, _) { ns_val(definite_enum) }
      ast::def_fn(_, _) | ast::def_self(_) |
      ast::def_const(_) | ast::def_arg(_, _) | ast::def_local(_) |
      ast::def_upvar(_, _, _) |  ast::def_self(_) |
      ast::def_class_field(_,_) | ast::def_class_method(_,_)
          { ns_val(value_or_enum) }
      ast::def_mod(_) | ast::def_native_mod(_) { ns_module }
      ast::def_ty(_) | ast::def_binding(_) | ast::def_use(_) |
      ast::def_ty_param(_, _) | ast::def_prim_ty(_) | ast::def_class(_)
      { ns_type }
    }
}

// if we're searching for a value, it's ok if we found
// a enum
fn ns_ok(wanted:namespace, actual:namespace) -> bool {
    alt actual {
      ns_val(definite_enum) {
        alt wanted {
          ns_val(_) { true }
          _ { false }
        }
      }
      _ { wanted == actual}
    }
}

fn lookup_external(e: env, cnum: int, ids: [ident], ns: namespace) ->
   option<def> {
    for d: def in csearch::lookup_defs(e.sess.cstore, cnum, ids) {
        e.ext_map.insert(def_id_of_def(d), ids);
        if ns_ok(ns, ns_for_def(d)) { ret some(d); }
    }
    ret none;
}


// Collision detection
fn check_for_collisions(e: @env, c: ast::crate) {
    // Module indices make checking those relatively simple -- just check each
    // name for multiple entities in the same namespace.
    e.mod_map.values {|val|
        val.index.items {|k, v| check_mod_name(*e, k, v); };
    };
    // Other scopes have to be checked the hard way.
    let v =
        @{visit_item: bind check_item(e, _, _, _),
          visit_block: bind check_block(e, _, _, _),
          visit_arm: bind check_arm(e, _, _, _),
          visit_expr: bind check_expr(e, _, _, _),
          visit_ty: bind check_ty(e, _, _, _) with *visit::default_visitor()};
    visit::visit_crate(c, (), visit::mk_vt(v));
}

fn check_mod_name(e: env, name: ident, entries: list<mod_index_entry>) {
    let saw_mod = false;
    let saw_type = false;
    let saw_value = false;
    let entries = entries;
    fn dup(e: env, sp: span, word: str, name: ident) {
        e.sess.span_fatal(sp, "duplicate definition of " + word + name);
    }
    while true {
        alt entries {
          cons(entry, rest) {
            if !is_none(lookup_in_mie(e, entry, ns_val(value_or_enum))) {
                if saw_value {
                    dup(e, mie_span(entry), "", name);
                } else { saw_value = true; }
            }
            if !is_none(lookup_in_mie(e, entry, ns_type)) {
                if saw_type {
                    dup(e, mie_span(entry), "type ", name);
                } else { saw_type = true; }
            }
            if !is_none(lookup_in_mie(e, entry, ns_module)) {
                if saw_mod {
                    dup(e, mie_span(entry), "module ", name);
                } else { saw_mod = true; }
            }
            entries = *rest;
          }
          nil { break; }
        }
    }
}

fn mie_span(mie: mod_index_entry) -> span {
    ret alt mie {
          mie_view_item(_, _, span) { span }
          mie_import_ident(_, span) { span }
          mie_item(item) { item.span }
          mie_enum_variant(_, _, _, span) { span }
          mie_native_item(item) { item.span }
          mie_class_item(_,item) { item.span }
        };
}

fn check_item(e: @env, i: @ast::item, &&x: (), v: vt<()>) {
    fn typaram_names(tps: [ast::ty_param]) -> [ident] {
        let x: [ast::ident] = [];
        for tp: ast::ty_param in tps { x += [tp.ident]; }
        ret x;
    }
    visit::visit_item(i, x, v);
    alt i.node {
      ast::item_fn(decl, ty_params, _) {
        check_fn(*e, i.span, decl);
        ensure_unique(*e, i.span, typaram_names(ty_params), ident_id,
                      "type parameter");
      }
      ast::item_enum(_, ty_params) {
        ensure_unique(*e, i.span, typaram_names(ty_params), ident_id,
                      "type parameter");
      }
      _ { }
    }
}

fn check_pat(e: @env, ch: checker, p: @ast::pat) {
    pat_util::pat_bindings(e.def_map, p) {|_i, p_sp, n|
       add_name(ch, p_sp, path_to_ident(n));
    };
}

fn check_arm(e: @env, a: ast::arm, &&x: (), v: vt<()>) {
    visit::visit_arm(a, x, v);
    let ch0 = checker(*e, "binding");
    check_pat(e, ch0, a.pats[0]);
    let seen0 = ch0.seen;
    let i = vec::len(a.pats);
    while i > 1u {
        i -= 1u;
        let ch = checker(*e, "binding");
        check_pat(e, ch, a.pats[i]);

        // Ensure the bindings introduced in this pattern are the same as in
        // the first pattern.
        if vec::len(ch.seen) != vec::len(seen0) {
            e.sess.span_err(a.pats[i].span,
                            "inconsistent number of bindings");
        } else {
            for name: ident in ch.seen {
                if is_none(vec::find(seen0, bind str::eq(name, _))) {
                    // Fight the alias checker
                    let name_ = name;
                    e.sess.span_err(a.pats[i].span,
                                    "binding " + name_ +
                                        " does not occur in first pattern");
                }
            }
        }
    }
}

fn check_block(e: @env, b: ast::blk, &&x: (), v: vt<()>) {
    visit::visit_block(b, x, v);
    let values = checker(*e, "value");
    let types = checker(*e, "type");
    let mods = checker(*e, "module");
    for st: @ast::stmt in b.node.stmts {
        alt st.node {
          ast::stmt_decl(d, _) {
            alt d.node {
              ast::decl_local(locs) {
                let local_values = checker(*e, "value");
                for loc in locs {
                     pat_util::pat_bindings(e.def_map, loc.node.pat)
                         {|_i, p_sp, n|
                         let ident = path_to_ident(n);
                         add_name(local_values, p_sp, ident);
                         check_name(values, p_sp, ident);
                     };
                }
              }
              ast::decl_item(it) {
                alt it.node {
                  ast::item_enum(variants, _) {
                    add_name(types, it.span, it.ident);
                    for v: ast::variant in variants {
                        add_name(values, v.span, v.node.name);
                    }
                  }
                  ast::item_mod(_) | ast::item_native_mod(_) {
                    add_name(mods, it.span, it.ident);
                  }
                  ast::item_const(_, _) | ast::item_fn(_, _, _) {
                    add_name(values, it.span, it.ident);
                  }
                  ast::item_ty(_, _) | ast::item_iface(_, _) {
                    add_name(types, it.span, it.ident);
                  }
                  ast::item_res(_, _, _, _, _) {
                    add_name(types, it.span, it.ident);
                    add_name(values, it.span, it.ident);
                  }
                  _ { }
                }
              }
            }
          }
          _ { }
        }
    }
}

fn check_fn(e: env, sp: span, decl: ast::fn_decl) {
    fn arg_name(a: ast::arg) -> ident { ret a.ident; }
    ensure_unique(e, sp, decl.inputs, arg_name, "argument");
}

fn check_expr(e: @env, ex: @ast::expr, &&x: (), v: vt<()>) {
    alt ex.node {
      ast::expr_rec(fields, _) {
        fn field_name(f: ast::field) -> ident { ret f.node.ident; }
        ensure_unique(*e, ex.span, fields, field_name, "field");
      }
      _ { }
    }
    visit::visit_expr(ex, x, v);
}

fn check_ty(e: @env, ty: @ast::ty, &&x: (), v: vt<()>) {
    alt ty.node {
      ast::ty_rec(fields) {
        fn field_name(f: ast::ty_field) -> ident { ret f.node.ident; }
        ensure_unique(*e, ty.span, fields, field_name, "field");
      }
      _ { }
    }
    visit::visit_ty(ty, x, v);
}

type checker = @{mutable seen: [ident], kind: str, sess: session};

fn checker(e: env, kind: str) -> checker {
    let seen: [ident] = [];
    ret @{mutable seen: seen, kind: kind, sess: e.sess};
}

fn check_name(ch: checker, sp: span, name: ident) {
    for s: ident in ch.seen {
        if str::eq(s, name) {
            ch.sess.span_fatal(sp, "duplicate " + ch.kind + " name: " + name);
        }
    }
}
fn add_name(ch: checker, sp: span, name: ident) {
    check_name(ch, sp, name);
    ch.seen += [name];
}

fn ident_id(&&i: ident) -> ident { ret i; }

fn ensure_unique<T>(e: env, sp: span, elts: [T], id: fn(T) -> ident,
                    kind: str) {
    let ch = checker(e, kind);
    for elt: T in elts { add_name(ch, sp, id(elt)); }
}

fn check_exports(e: @env) {

    fn iter_mod(e: env, m: def, sp: span, _dr: dir,
                f: fn(ident: ident, def: def)) {
        let defid = def_id_of_def(m);

        if defid.crate != ast::local_crate {
            // FIXME: ought to support external export-globs eventually.
            e.sess.span_unimpl(sp, "glob-export of items in external crate");
        } else {

            let mid = def_id_of_def(m);
            assert mid.crate == ast::local_crate;
            let ixm = e.mod_map.get(mid.node);

            ixm.index.items() {|ident, mies|
                list::iter(mies) {|mie|
                    alt mie {
                      mie_item(item) {
                        let defs =
                            [ found_def_item(item, ns_val(value_or_enum)),
                             found_def_item(item, ns_type),
                             found_def_item(item, ns_module) ];
                        for d in defs {
                            alt d {
                              some(def) {
                                f(ident, def);
                              }
                              _ {}
                            }
                        }
                      }
                      _ {
                        let s = "glob-export from mod with non-items";
                        e.sess.span_unimpl(sp, s);
                      }
                    }
                }
            }
        }
    }



    fn lookup_glob_any(e: @env, info: @indexed_mod, sp: span, path: str,
                       ident: ident) -> bool {
        let lookup =
            bind lookup_glob_in_mod(*e, info, sp, ident, _, inside);
        let (m, v, t) = (lookup(ns_module),
                         lookup(ns_val(value_or_enum)),
                         lookup(ns_type));
        let full_path = path + ident;
        maybe_add_reexport(e, full_path, m);
        maybe_add_reexport(e, full_path, v);
        maybe_add_reexport(e, full_path, t);
        is_some(m) || is_some(v) || is_some(t)
    }

    fn maybe_add_reexport(e: @env, path: str, def: option<def>) {
        alt def {
          some(def) {
            alt e.exp_map.find(path) {
              some(v) {
                // If there are multiple reexports of the same def
                // using the same path, then we only need one copy
                if !vec::contains(*v, def) {
                    *v += [def];
                }
              }
              none { e.exp_map.insert(path, @mutable [def]); }
            }
          }
          _ {}
        }
    }

    fn check_export(e: @env, ident: str, _mod: @indexed_mod,
                    vi: @view_item) {
        let found_something = false;
        let full_path = _mod.path + ident;
        if _mod.index.contains_key(ident) {
            found_something = true;
            let xs = _mod.index.get(ident);
            list::iter(xs) {|x|
                alt x {
                  mie_import_ident(id, _) {
                    alt e.imports.get(id) {
                      resolved(v, t, m, _, rid, _) {
                        maybe_add_reexport(e, full_path, v);
                        maybe_add_reexport(e, full_path, t);
                        maybe_add_reexport(e, full_path, m);
                      }
                      _ { }
                    }
                  }
                  _ { }
                }
            }
        }
        found_something |= lookup_glob_any(e, _mod, vi.span,
                                           _mod.path, ident);
        if !found_something {
            e.sess.span_warn(vi.span,
                             #fmt("exported item %s is not defined", ident));
        }
    }

    fn check_enum_ok(e: @env, sp:span, id: ident, _mod: @indexed_mod)
        -> node_id {
        alt _mod.index.find(id) {
           none { e.sess.span_fatal(sp, #fmt("error: undefined id %s \
                         in an export", id)); }
           some(ms) {
             let maybe_id = list::find(ms) {|m|
                  alt m {
                     mie_item(an_item) {
                      alt an_item.node {
                          item_enum(_,_) { /* OK */ some(an_item.id) }
                          _ { none }
                      }
                     }
                     _ { none }
               }
             };
             alt maybe_id {
                some(an_id) { ret an_id; }
                _ { e.sess.span_fatal(sp, #fmt("error: %s does not refer \
                          to an enumeration", id)); }
             }
         }
      }
    }

    fn check_export_enum_list(e: @env, _mod: @indexed_mod,
                              span: codemap::span, id: ast::ident,
                              ids: [ast::path_list_ident]) {
        if vec::len(ids) == 0u {
            let _ = check_enum_ok(e, span, id, _mod);
        } else {
            let parent_id = check_enum_ok(e, span, id, _mod);
            for variant_id in ids {
                alt _mod.index.find(variant_id.node.name) {
                  some(ms) {
                    list::iter(ms) {|m|
                        alt m {
                          mie_enum_variant(_, _, actual_parent_id, _) {
                            if actual_parent_id != parent_id {
                                let msg = #fmt("variant %s \
                                                doesn't belong to enum %s",
                                               variant_id.node.name,
                                               id);
                                e.sess.span_err(span, msg);
                            }
                          }
                          _ {
                            e.sess.span_err(span,
                                            #fmt("%s is not a variant",
                                                 variant_id.node.name));
                          }
                        }
                    }
                  }
                  _ {
                    e.sess.span_err(span,
                                    #fmt("%s is not a variant",
                                         variant_id.node.name));
                  }
                }
            }
        }
    }

    e.mod_map.values {|_mod|
        alt _mod.m {
          some(m) {
            let glob_is_re_exported = new_int_hash();

            for vi in m.view_items {
                iter_export_paths(*vi) { |vp|
                    alt vp.node {
                      ast::view_path_simple(ident, _, _) {
                        check_export(e, ident, _mod, vi);
                      }
                      ast::view_path_list(path, ids, _) {
                        let id = if vec::len(*path) == 1u {
                            path[0]
                        } else {
                            e.sess.span_fatal(vp.span,
                                            #fmt("bad export name-list"))
                        };
                        check_export_enum_list(e, _mod, vp.span, id, ids);
                      }
                      ast::view_path_glob(_, node_id) {
                        glob_is_re_exported.insert(node_id, ());
                      }
                    }
                }
            }
            // Now follow the export-glob links and fill in the
            // globbed_exports and exp_map lists.
            for glob in _mod.glob_imports {
                alt check glob.path.node {
                  ast::view_path_glob(path, node_id) {
                    if ! glob_is_re_exported.contains_key(node_id) {
                        cont;
                    }
                  }
                }
                iter_mod(*e, glob.def,
                         glob.path.span, outside) {|ident, def|
                    let full_path = _mod.path + ident;
                    _mod.globbed_exports += [ident];
                    maybe_add_reexport(e, full_path, some(def));
                }
            }
          }
          none { }
        }
    }
}

// Impl resolution

type method_info = {did: def_id, n_tps: uint, ident: ast::ident};
type _impl = {did: def_id, ident: ast::ident, methods: [@method_info]};
type iscopes = list<@[@_impl]>;

fn resolve_impls(e: @env, c: @ast::crate) {
    visit::visit_crate(*c, nil, visit::mk_vt(@{
        visit_block: bind visit_block_with_impl_scope(e, _, _, _),
        visit_mod: bind visit_mod_with_impl_scope(e, _, _, _, _, _),
        visit_expr: bind resolve_impl_in_expr(e, _, _, _)
        with *visit::default_visitor()
    }));
}

fn find_impls_in_view_item(e: env, vi: @ast::view_item,
                           &impls: [@_impl], sc: option<iscopes>) {
    fn lookup_imported_impls(e: env, id: ast::node_id,
                             act: fn(@[@_impl])) {
        alt e.imports.get(id) {
          resolved(_, _, _, is, _, _) { act(is); }
          todo(node_id, name, path, span, scopes) {
            resolve_import(e, local_def(node_id), name, *path, span,
                           scopes);
            alt check e.imports.get(id) {
              resolved(_, _, _, is, _, _) { act(is); }
            }
          }
          _ {}
        }
    }

    iter_effective_import_paths(*vi) { |vp|
        alt vp.node {
          ast::view_path_simple(name, pt, id) {
            let found = [];
            if vec::len(*pt) == 1u {
                option::may(sc) {|sc|
                    list::iter(sc) {|level|
                        if vec::len(found) > 0u { ret; }
                        for imp in *level {
                            if imp.ident == pt[0] {
                                found += [@{ident: name with *imp}];
                            }
                        }
                        if vec::len(found) > 0u { impls += found; }
                    }
                }
            } else {
                lookup_imported_impls(e, id) {|is|
                    for i in *is { impls += [@{ident: name with *i}]; }
                }
            }
          }

          ast::view_path_list(base, names, _) {
            for nm in names {
                lookup_imported_impls(e, nm.node.id) {|is| impls += *is; }
            }
          }

          ast::view_path_glob(ids, id) {
            alt check e.imports.get(id) {
              is_glob(path, sc, sp) {
                alt follow_import(e, sc, *path, sp) {
                  some(def) { find_impls_in_mod(e, def, impls, none); }
                  _ {}
                }
              }
            }
          }
        }
    }
}

fn find_impls_in_item(e: env, i: @ast::item, &impls: [@_impl],
                      name: option<ident>,
                      ck_exports: option<@indexed_mod>) {
    alt i.node {
      ast::item_impl(_, ifce, _, mthds) {
        if alt name { some(n) { n == i.ident } _ { true } } &&
           alt ck_exports {
             some(m) { is_exported(e, i.ident, m) }
             _ { true }
           } {
            impls += [@{did: local_def(i.id),
                        ident: i.ident,
                        methods: vec::map(mthds, {|m|
                            @{did: local_def(m.id),
                              n_tps: vec::len(m.tps),
                              ident: m.ident}
                        })}];
        }
      }
      _ {}
    }
}

fn find_impls_in_mod_by_id(e: env, defid: def_id, &impls: [@_impl],
                           name: option<ident>) {
    let cached;
    alt e.impl_cache.find(defid) {
      some(some(v)) { cached = v; }
      some(none) { ret; }
      none {
        e.impl_cache.insert(defid, none);
        cached = if defid.crate == ast::local_crate {
            let tmp = [];
            let mi = e.mod_map.get(defid.node);
            let md = option::get(mi.m);
            for vi in md.view_items {
                find_impls_in_view_item(e, vi, tmp, none);
            }
            for i in md.items {
                find_impls_in_item(e, i, tmp, none, none);
            }
            @vec::filter(tmp) {|i| is_exported(e, i.ident, mi)}
        } else {
            csearch::get_impls_for_mod(e.sess.cstore, defid, none)
        };
        e.impl_cache.insert(defid, some(cached));
      }
    }
    alt name {
      some(n) {
        for im in *cached {
            if n == im.ident { impls += [im]; }
        }
      }
      _ { impls += *cached; }
    }
}

fn find_impls_in_mod(e: env, m: def, &impls: [@_impl],
                     name: option<ident>) {
    alt m {
      ast::def_mod(defid) {
        find_impls_in_mod_by_id(e, defid, impls, name);
      }
      _ {}
    }
}

fn visit_block_with_impl_scope(e: @env, b: ast::blk, sc: iscopes,
                               v: vt<iscopes>) {
    let impls = [];
    for vi in b.node.view_items {
        find_impls_in_view_item(*e, vi, impls, some(sc));
    }
    for st in b.node.stmts {
        alt st.node {
          ast::stmt_decl(@{node: ast::decl_item(i), _}, _) {
            find_impls_in_item(*e, i, impls, none, none);
          }
          _ {}
        }
    }
    let sc = if vec::len(impls) > 0u { cons(@impls, @sc) } else { sc };
    visit::visit_block(b, sc, v);
}

fn visit_mod_with_impl_scope(e: @env, m: ast::_mod, s: span, id: node_id,
                             sc: iscopes, v: vt<iscopes>) {
    let impls = [];
    for vi in m.view_items {
        find_impls_in_view_item(*e, vi, impls, some(sc));
    }
    for i in m.items { find_impls_in_item(*e, i, impls, none, none); }
    let impls = @impls;
    visit::visit_mod(m, s, id, if vec::len(*impls) > 0u {
                                   cons(impls, @sc)
                               } else {
                                   sc
                               }, v);
    e.impl_map.insert(id, cons(impls, @nil));
}

fn resolve_impl_in_expr(e: @env, x: @ast::expr, sc: iscopes, v: vt<iscopes>) {
    alt x.node {
      // Store the visible impls in all exprs that might need them
      ast::expr_field(_, _, _) | ast::expr_path(_) | ast::expr_cast(_, _) |
      ast::expr_binary(_, _, _) | ast::expr_unary(_, _) |
      ast::expr_assign_op(_, _, _) | ast::expr_index(_, _) {
        e.impl_map.insert(x.id, sc);
      }
      _ {}
    }
    visit::visit_expr(x, sc, v);
}

// Local Variables:
// mode: rust
// fill-column: 78;
// indent-tabs-mode: nil
// c-basic-offset: 4
// buffer-file-coding-system: utf-8-unix
// End:
