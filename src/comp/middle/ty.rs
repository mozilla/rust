
import std::int;
import std::str;
import std::uint;
import std::vec;
import std::box;
import std::ufind;
import std::map;
import std::map::hashmap;
import std::option;
import std::option::none;
import std::option::some;
import std::smallintmap;
import driver::session;
import front::ast;
import front::ast::def_id;
import front::ast::constr_arg_general;
import front::ast::mutability;
import front::ast::controlflow;
import front::creader;
import middle::metadata;
import util::common::*;

import util::data::interner;


// Data types
tag mode { mo_val; mo_alias(bool); }

type arg = rec(mode mode, t ty);

type field = rec(ast::ident ident, mt mt);

type method =
    rec(ast::proto proto,
        ast::ident ident,
        vec[arg] inputs,
        t output,
        controlflow cf,
        vec[@constr_def] constrs);

tag any_item {
    any_item_rust(@ast::item);
    any_item_native(@ast::native_item, ast::native_abi);
}

type item_table = hashmap[ast::def_id, any_item];
type constr_table = hashmap[ast::def_id, vec[constr_def]]; 

type mt = rec(t ty, ast::mutability mut);


// Contains information needed to resolve types and (in the future) look up
// the types of AST nodes.

type creader_cache = hashmap[tup(int, uint, uint), ty::t];

type ctxt =
    rec(@type_store ts,
        session::session sess,
        resolve::def_map def_map,
        node_type_table node_types,
        item_table items, // Only contains type items
        constr_table fn_constrs,
        type_cache tcache,
        creader_cache rcache,
        hashmap[t, str] short_names_cache,
        hashmap[@ast::ty, option::t[t]] ast_ty_to_ty_cache);

type ty_ctxt = ctxt;

 // Needed for disambiguation from unify::ctxt.
 // Convert from method type to function type.  Pretty easy; we just drop
 // 'ident'.
 fn method_ty_to_fn_ty(&ctxt cx, method m) -> t {
     ret mk_fn(cx, m.proto, m.inputs, m.output, m.cf, m.constrs);
 }

// Never construct these manually. These are interned.
//
// TODO: It'd be really nice to be able to hide this definition from the
// outside world, to enforce the above invariants.
type raw_t =
    rec(sty struct,
        option::t[str] cname,
        uint hash,
        bool has_params,
        bool has_vars);

type t = uint;


// NB: If you change this, you'll probably want to change the corresponding
// AST structure in front/ast::rs as well.
tag sty {
    ty_nil;
    ty_bot;
    ty_bool;
    ty_int;
    ty_float;
    ty_uint;
    ty_machine(ty_mach);
    ty_char;
    ty_str;
    ty_istr;
    ty_tag(def_id, vec[t]);
    ty_box(mt);
    ty_vec(mt);
    ty_ivec(mt);
    ty_ptr(mt);
    ty_port(t);
    ty_chan(t);
    ty_task;
    ty_tup(vec[mt]);
    ty_rec(vec[field]);
    ty_fn(ast::proto, vec[arg], t, controlflow, vec[@constr_def]);
    ty_native_fn(ast::native_abi, vec[arg], t);
    ty_obj(vec[method]);
    ty_var(int); // type variable

    ty_param(uint); // fn/tag type param

    ty_type;
    ty_native;
    // TODO: ty_fn_arg(t), for a possibly-aliased function argument

}

type constr_def = spanned[constr_general[uint]];
type constr_general[T] =  rec(path path,
                              vec[@constr_arg_general[T]] args,
                              def_id id);

// Data structures used in type unification
tag type_err {
    terr_mismatch;
    terr_controlflow_mismatch;
    terr_box_mutability;
    terr_vec_mutability;
    terr_tuple_size(uint, uint);
    terr_tuple_mutability;
    terr_record_size(uint, uint);
    terr_record_mutability;
    terr_record_fields(ast::ident, ast::ident);
    terr_meth_count;
    terr_obj_meths(ast::ident, ast::ident);
    terr_arg_count;
}

type ty_param_count_and_ty = tup(uint, t);

type type_cache = hashmap[ast::def_id, ty_param_count_and_ty];

const uint idx_nil = 0u;

const uint idx_bool = 1u;

const uint idx_int = 2u;

const uint idx_float = 3u;

const uint idx_uint = 4u;

const uint idx_i8 = 5u;

const uint idx_i16 = 6u;

const uint idx_i32 = 7u;

const uint idx_i64 = 8u;

const uint idx_u8 = 9u;

const uint idx_u16 = 10u;

const uint idx_u32 = 11u;

const uint idx_u64 = 12u;

const uint idx_f32 = 13u;

const uint idx_f64 = 14u;

const uint idx_char = 15u;

const uint idx_str = 16u;

const uint idx_istr = 17u;

const uint idx_task = 18u;

const uint idx_native = 19u;

const uint idx_type = 20u;

const uint idx_bot = 21u;

const uint idx_first_others = 22u;

type type_store = interner::interner[raw_t];

type ty_param_substs_opt_and_ty = tup(option::t[vec[ty::t]], ty::t);

type node_type_table =
    @mutable vec[mutable option::t[ty::ty_param_substs_opt_and_ty]];

fn populate_type_store(&ctxt cx) {
    intern(cx, ty_nil, none[str]);
    intern(cx, ty_bool, none[str]);
    intern(cx, ty_int, none[str]);
    intern(cx, ty_float, none[str]);
    intern(cx, ty_uint, none[str]);
    intern(cx, ty_machine(ty_i8), none[str]);
    intern(cx, ty_machine(ty_i16), none[str]);
    intern(cx, ty_machine(ty_i32), none[str]);
    intern(cx, ty_machine(ty_i64), none[str]);
    intern(cx, ty_machine(ty_u8), none[str]);
    intern(cx, ty_machine(ty_u16), none[str]);
    intern(cx, ty_machine(ty_u32), none[str]);
    intern(cx, ty_machine(ty_u64), none[str]);
    intern(cx, ty_machine(ty_f32), none[str]);
    intern(cx, ty_machine(ty_f64), none[str]);
    intern(cx, ty_char, none[str]);
    intern(cx, ty_str, none[str]);
    intern(cx, ty_istr, none[str]);
    intern(cx, ty_task, none[str]);
    intern(cx, ty_native, none[str]);
    intern(cx, ty_type, none[str]);
    intern(cx, ty_bot, none[str]);
    assert (vec::len(cx.ts.vect) == idx_first_others);
}

fn mk_rcache() -> creader_cache {
    fn hash_cache_entry(&tup(int, uint, uint) k) -> uint {
        ret (k._0 as uint) + k._1 + k._2;
    }
    fn eq_cache_entries(&tup(int, uint, uint) a, &tup(int, uint, uint) b) ->
       bool {
        ret a._0 == b._0 && a._1 == b._1 && a._2 == b._2;
    }
    auto h = hash_cache_entry;
    auto e = eq_cache_entries;
    ret map::mk_hashmap[tup(int, uint, uint), t](h, e);
}

fn mk_ctxt(session::session s, resolve::def_map dm, constr_table cs) -> ctxt {

    let vec[mutable option::t[ty::ty_param_substs_opt_and_ty]] ntt_sub =
        [mutable ];
    let node_type_table ntt = @mutable ntt_sub;
    auto tcache = new_def_hash[ty::ty_param_count_and_ty]();
    auto items = new_def_hash[any_item]();
    auto ts = @interner::mk[raw_t](hash_raw_ty, eq_raw_ty);
    auto cx =
        rec(ts=ts,
            sess=s,
            def_map=dm,
            node_types=ntt,
            items=items,
            fn_constrs = cs,
            tcache=tcache,
            rcache=mk_rcache(),
            short_names_cache=map::mk_hashmap[ty::t,
                                              str](ty::hash_ty, ty::eq_ty),
            ast_ty_to_ty_cache=map::mk_hashmap[@ast::ty,
                                               option::t[t]](ast::hash_ty,
                                                             ast::eq_ty));
    populate_type_store(cx);
    ret cx;
}


// Type constructors
fn mk_raw_ty(&ctxt cx, &sty st, &option::t[str] cname) -> raw_t {
    auto h = hash_type_info(st, cname);
    let bool has_params = false;
    let bool has_vars = false;
    fn derive_flags_t(&ctxt cx, &mutable bool has_params,
                      &mutable bool has_vars, &t tt) {
        auto rt = interner::get[raw_t](*cx.ts, tt);
        has_params = has_params || rt.has_params;
        has_vars = has_vars || rt.has_vars;
    }
    fn derive_flags_mt(&ctxt cx, &mutable bool has_params,
                       &mutable bool has_vars, &mt m) {
        derive_flags_t(cx, has_params, has_vars, m.ty);
    }
    fn derive_flags_arg(&ctxt cx, &mutable bool has_params,
                        &mutable bool has_vars, &arg a) {
        derive_flags_t(cx, has_params, has_vars, a.ty);
    }
    fn derive_flags_sig(&ctxt cx, &mutable bool has_params,
                        &mutable bool has_vars, &vec[arg] args, &t tt) {
        for (arg a in args) { derive_flags_arg(cx, has_params, has_vars, a); }
        derive_flags_t(cx, has_params, has_vars, tt);
    }
    alt (st) {
        case (ty_nil) {/* no-op */ }
        case (ty_bot) {/* no-op */ }
        case (ty_bool) {/* no-op */ }
        case (ty_int) {/* no-op */ }
        case (ty_float) {/* no-op */ }
        case (ty_uint) {/* no-op */ }
        case (ty_machine(_)) {/* no-op */ }
        case (ty_char) {/* no-op */ }
        case (ty_str) {/* no-op */ }
        case (ty_istr) {/* no-op */ }
        case (ty_task) {/* no-op */ }
        case (ty_type) {/* no-op */ }
        case (ty_native) {/* no-op */ }
        case (ty_param(_)) { has_params = true; }
        case (ty_var(_)) { has_vars = true; }
        case (ty_tag(_, ?tys)) {
            for (t tt in tys) {
                derive_flags_t(cx, has_params, has_vars, tt);
            }
        }
        case (ty_box(?m)) { derive_flags_mt(cx, has_params, has_vars, m); }
        case (ty_vec(?m)) { derive_flags_mt(cx, has_params, has_vars, m); }
        case (ty_ivec(?m)) { derive_flags_mt(cx, has_params, has_vars, m); }
        case (ty_ptr(?m)) { derive_flags_mt(cx, has_params, has_vars, m); }
        case (ty_port(?tt)) { derive_flags_t(cx, has_params, has_vars, tt); }
        case (ty_chan(?tt)) { derive_flags_t(cx, has_params, has_vars, tt); }
        case (ty_tup(?mts)) {
            for (mt m in mts) {
                derive_flags_mt(cx, has_params, has_vars, m);
            }
        }
        case (ty_rec(?flds)) {
            for (field f in flds) {
                derive_flags_mt(cx, has_params, has_vars, f.mt);
            }
        }
        case (ty_fn(_, ?args, ?tt, _, _)) {
            derive_flags_sig(cx, has_params, has_vars, args, tt);
        }
        case (ty_native_fn(_, ?args, ?tt)) {
            derive_flags_sig(cx, has_params, has_vars, args, tt);
        }
        case (ty_obj(?meths)) {
            for (method m in meths) {
                derive_flags_sig(cx, has_params, has_vars, m.inputs,
                                 m.output);
            }
        }
    }
    ret rec(struct=st,
            cname=cname,
            hash=h,
            has_params=has_params,
            has_vars=has_vars);
}

fn intern(&ctxt cx, &sty st, &option::t[str] cname) {
    interner::intern[raw_t](*cx.ts, mk_raw_ty(cx, st, cname));
}

fn gen_ty_full(&ctxt cx, &sty st, &option::t[str] cname) -> t {
    auto raw_type = mk_raw_ty(cx, st, cname);
    ret interner::intern[raw_t](*cx.ts, raw_type);
}


// These are private constructors to this module. External users should always
// use the mk_foo() functions below.
fn gen_ty(&ctxt cx, &sty st) -> t { ret gen_ty_full(cx, st, none[str]); }

fn mk_nil(&ctxt cx) -> t { ret idx_nil; }

fn mk_bot(&ctxt cx) -> t { ret idx_bot; }

fn mk_bool(&ctxt cx) -> t { ret idx_bool; }

fn mk_int(&ctxt cx) -> t { ret idx_int; }

fn mk_float(&ctxt cx) -> t { ret idx_float; }

fn mk_uint(&ctxt cx) -> t { ret idx_uint; }

fn mk_mach(&ctxt cx, &ty_mach tm) -> t {
    alt (tm) {
        case (ty_u8) { ret idx_u8; }
        case (ty_u16) { ret idx_u16; }
        case (ty_u32) { ret idx_u32; }
        case (ty_u64) { ret idx_u64; }
        case (ty_i8) { ret idx_i8; }
        case (ty_i16) { ret idx_i16; }
        case (ty_i32) { ret idx_i32; }
        case (ty_i64) { ret idx_i64; }
        case (ty_f32) { ret idx_f32; }
        case (ty_f64) { ret idx_f64; }
    }
}

fn mk_char(&ctxt cx) -> t { ret idx_char; }

fn mk_str(&ctxt cx) -> t { ret idx_str; }

fn mk_istr(&ctxt cx) -> t { ret idx_istr; }

fn mk_tag(&ctxt cx, &ast::def_id did, &vec[t] tys) -> t {
    ret gen_ty(cx, ty_tag(did, tys));
}

fn mk_box(&ctxt cx, &mt tm) -> t { ret gen_ty(cx, ty_box(tm)); }

fn mk_ptr(&ctxt cx, &mt tm) -> t { ret gen_ty(cx, ty_ptr(tm)); }

fn mk_imm_box(&ctxt cx, &t ty) -> t {
    ret mk_box(cx, rec(ty=ty, mut=ast::imm));
}

fn mk_vec(&ctxt cx, &mt tm) -> t { ret gen_ty(cx, ty_vec(tm)); }

fn mk_ivec(&ctxt cx, &mt tm) -> t { ret gen_ty(cx, ty_ivec(tm)); }

fn mk_imm_vec(&ctxt cx, &t typ) -> t {
    ret gen_ty(cx, ty_vec(rec(ty=typ, mut=ast::imm)));
}

fn mk_port(&ctxt cx, &t ty) -> t { ret gen_ty(cx, ty_port(ty)); }

fn mk_chan(&ctxt cx, &t ty) -> t { ret gen_ty(cx, ty_chan(ty)); }

fn mk_task(&ctxt cx) -> t { ret gen_ty(cx, ty_task); }

fn mk_tup(&ctxt cx, &vec[mt] tms) -> t { ret gen_ty(cx, ty_tup(tms)); }

fn mk_imm_tup(&ctxt cx, &vec[t] tys) -> t {
    // TODO: map

    let vec[ty::mt] mts = [];
    for (t typ in tys) { mts += [rec(ty=typ, mut=ast::imm)]; }
    ret mk_tup(cx, mts);
}

fn mk_rec(&ctxt cx, &vec[field] fs) -> t { ret gen_ty(cx, ty_rec(fs)); }

fn mk_fn(&ctxt cx, &ast::proto proto, &vec[arg] args, &t ty, &controlflow cf,
         &vec[@constr_def] constrs) -> t {
    ret gen_ty(cx, ty_fn(proto, args, ty, cf, constrs));
}

fn mk_native_fn(&ctxt cx, &ast::native_abi abi, &vec[arg] args, &t ty) -> t {
    ret gen_ty(cx, ty_native_fn(abi, args, ty));
}

fn mk_obj(&ctxt cx, &vec[method] meths) -> t {
    ret gen_ty(cx, ty_obj(meths));
}

fn mk_var(&ctxt cx, int v) -> t { ret gen_ty(cx, ty_var(v)); }

fn mk_param(&ctxt cx, uint n) -> t { ret gen_ty(cx, ty_param(n)); }

fn mk_type(&ctxt cx) -> t { ret idx_type; }

fn mk_native(&ctxt cx) -> t { ret idx_native; }


// Returns the one-level-deep type structure of the given type.
fn struct(&ctxt cx, &t typ) -> sty {
    ret interner::get[raw_t](*cx.ts, typ).struct;
}


// Returns the canonical name of the given type.
fn cname(&ctxt cx, &t typ) -> option::t[str] {
    ret interner::get[raw_t](*cx.ts, typ).cname;
}


// Stringification
fn path_to_str(&ast::path pth) -> str {
    auto result = str::connect(pth.node.idents, "::");
    if (vec::len[@ast::ty](pth.node.types) > 0u) {
        fn f(&@ast::ty t) -> str { ret pretty::pprust::ty_to_str(*t); }
        result += "[";
        result += str::connect(vec::map(f, pth.node.types), ",");
        result += "]";
    }
    ret result;
}


// Type folds
type ty_walk = fn(t) ;

fn walk_ty(&ctxt cx, ty_walk walker, t ty) {
    alt (struct(cx, ty)) {
        case (ty_nil) {/* no-op */ }
        case (ty_bot) {/* no-op */ }
        case (ty_bool) {/* no-op */ }
        case (ty_int) {/* no-op */ }
        case (ty_uint) {/* no-op */ }
        case (ty_float) {/* no-op */ }
        case (ty_machine(_)) {/* no-op */ }
        case (ty_char) {/* no-op */ }
        case (ty_str) {/* no-op */ }
        case (ty_istr) {/* no-op */ }
        case (ty_type) {/* no-op */ }
        case (ty_native) {/* no-op */ }
        case (ty_box(?tm)) { walk_ty(cx, walker, tm.ty); }
        case (ty_vec(?tm)) { walk_ty(cx, walker, tm.ty); }
        case (ty_ivec(?tm)) { walk_ty(cx, walker, tm.ty); }
        case (ty_port(?subty)) { walk_ty(cx, walker, subty); }
        case (ty_chan(?subty)) { walk_ty(cx, walker, subty); }
        case (ty_tag(?tid, ?subtys)) {
            for (t subty in subtys) { walk_ty(cx, walker, subty); }
        }
        case (ty_tup(?mts)) {
            for (mt tm in mts) { walk_ty(cx, walker, tm.ty); }
        }
        case (ty_rec(?fields)) {
            for (field fl in fields) { walk_ty(cx, walker, fl.mt.ty); }
        }
        case (ty_fn(?proto, ?args, ?ret_ty, _, _)) {
            for (arg a in args) { walk_ty(cx, walker, a.ty); }
            walk_ty(cx, walker, ret_ty);
        }
        case (ty_native_fn(?abi, ?args, ?ret_ty)) {
            for (arg a in args) { walk_ty(cx, walker, a.ty); }
            walk_ty(cx, walker, ret_ty);
        }
        case (ty_obj(?methods)) {
            let vec[method] new_methods = [];
            for (method m in methods) {
                for (arg a in m.inputs) { walk_ty(cx, walker, a.ty); }
                walk_ty(cx, walker, m.output);
            }
        }
        case (ty_var(_)) {/* no-op */ }
        case (ty_param(_)) {/* no-op */ }
    }
    walker(ty);
}

tag fold_mode {
    fm_var(fn(int) -> t );
    fm_param(fn(uint) -> t );
    fm_general(fn(t) -> t );
}

fn fold_ty(&ctxt cx, fold_mode fld, t ty_0) -> t {
    auto ty = ty_0;
    // Fast paths.

    alt (fld) {
        case (fm_var(_)) { if (!type_contains_vars(cx, ty)) { ret ty; } }
        case (fm_param(_)) { if (!type_contains_params(cx, ty)) { ret ty; } }
        case (fm_general(_)) {/* no fast path */ }
    }
    alt (struct(cx, ty)) {
        case (ty_nil) {/* no-op */ }
        case (ty_bot) {/* no-op */ }
        case (ty_bool) {/* no-op */ }
        case (ty_int) {/* no-op */ }
        case (ty_uint) {/* no-op */ }
        case (ty_float) {/* no-op */ }
        case (ty_machine(_)) {/* no-op */ }
        case (ty_char) {/* no-op */ }
        case (ty_str) {/* no-op */ }
        case (ty_istr) {/* no-op */ }
        case (ty_type) {/* no-op */ }
        case (ty_native) {/* no-op */ }
        case (ty_task) {/* no-op */ }
        case (ty_box(?tm)) {
            ty =
                copy_cname(cx,
                           mk_box(cx,
                                  rec(ty=fold_ty(cx, fld, tm.ty),
                                      mut=tm.mut)), ty);
        }
        case (ty_ptr(?tm)) {
            ty =
                copy_cname(cx,
                           mk_ptr(cx,
                                  rec(ty=fold_ty(cx, fld, tm.ty),
                                      mut=tm.mut)), ty);
        }
        case (ty_vec(?tm)) {
            ty =
                copy_cname(cx,
                           mk_vec(cx,
                                  rec(ty=fold_ty(cx, fld, tm.ty),
                                      mut=tm.mut)), ty);
        }
        case (ty_ivec(?tm)) {
            ty =
                copy_cname(cx,
                           mk_ivec(cx,
                                   rec(ty=fold_ty(cx, fld, tm.ty),
                                       mut=tm.mut)), ty);
        }
        case (ty_port(?subty)) {
            ty = copy_cname(cx, mk_port(cx, fold_ty(cx, fld, subty)), ty);
        }
        case (ty_chan(?subty)) {
            ty = copy_cname(cx, mk_chan(cx, fold_ty(cx, fld, subty)), ty);
        }
        case (ty_tag(?tid, ?subtys)) {
            let vec[t] new_subtys = [];
            for (t subty in subtys) {
                new_subtys += [fold_ty(cx, fld, subty)];
            }
            ty = copy_cname(cx, mk_tag(cx, tid, new_subtys), ty);
        }
        case (ty_tup(?mts)) {
            let vec[mt] new_mts = [];
            for (mt tm in mts) {
                auto new_subty = fold_ty(cx, fld, tm.ty);
                new_mts += [rec(ty=new_subty, mut=tm.mut)];
            }
            ty = copy_cname(cx, mk_tup(cx, new_mts), ty);
        }
        case (ty_rec(?fields)) {
            let vec[field] new_fields = [];
            for (field fl in fields) {
                auto new_ty = fold_ty(cx, fld, fl.mt.ty);
                auto new_mt = rec(ty=new_ty, mut=fl.mt.mut);
                new_fields += [rec(ident=fl.ident, mt=new_mt)];
            }
            ty = copy_cname(cx, mk_rec(cx, new_fields), ty);
        }
        case (ty_fn(?proto, ?args, ?ret_ty, ?cf, ?constrs)) {
            let vec[arg] new_args = [];
            for (arg a in args) {
                auto new_ty = fold_ty(cx, fld, a.ty);
                new_args += [rec(mode=a.mode, ty=new_ty)];
            }
            ty =
                copy_cname(cx,
                           mk_fn(cx, proto, new_args,
                                 fold_ty(cx, fld, ret_ty), cf, constrs), ty);
        }
        case (ty_native_fn(?abi, ?args, ?ret_ty)) {
            let vec[arg] new_args = [];
            for (arg a in args) {
                auto new_ty = fold_ty(cx, fld, a.ty);
                new_args += [rec(mode=a.mode, ty=new_ty)];
            }
            ty =
                copy_cname(cx,
                           mk_native_fn(cx, abi, new_args,
                                        fold_ty(cx, fld, ret_ty)), ty);
        }
        case (ty_obj(?methods)) {
            let vec[method] new_methods = [];
            for (method m in methods) {
                let vec[arg] new_args = [];
                for (arg a in m.inputs) {
                    new_args += [rec(mode=a.mode, ty=fold_ty(cx, fld, a.ty))];
                }
                new_methods +=
                    [rec(proto=m.proto,
                         ident=m.ident,
                         inputs=new_args,
                         output=fold_ty(cx, fld, m.output),
                         cf=m.cf,
                         constrs=m.constrs)];
            }
            ty = copy_cname(cx, mk_obj(cx, new_methods), ty);
        }
        case (ty_var(?id)) {
            alt (fld) {
                case (fm_var(?folder)) { ty = folder(id); }
                case (_) {/* no-op */ }
            }
        }
        case (ty_param(?id)) {
            alt (fld) {
                case (fm_param(?folder)) { ty = folder(id); }
                case (_) {/* no-op */ }
            }
        }
    }

    // If this is a general type fold, then we need to run it now.
    alt (fld) {
        case (fm_general(?folder)) { ret folder(ty); }
        case (_) { ret ty; }
    }
}


// Type utilities
fn rename(&ctxt cx, t typ, str new_cname) -> t {
    ret gen_ty_full(cx, struct(cx, typ), some[str](new_cname));
}


// Returns a type with the structural part taken from `struct_ty` and the
// canonical name from `cname_ty`.
fn copy_cname(&ctxt cx, t struct_ty, t cname_ty) -> t {
    ret gen_ty_full(cx, struct(cx, struct_ty), cname(cx, cname_ty));
}

fn type_is_nil(&ctxt cx, &t ty) -> bool {
    alt (struct(cx, ty)) {
        case (ty_nil) { ret true; }
        case (_) { ret false; }
    }
}

fn type_is_bot(&ctxt cx, &t ty) -> bool {
    alt (struct(cx, ty)) {
        case (ty_bot) { ret true; }
        case (_) { ret false; }
    }
}

fn type_is_bool(&ctxt cx, &t ty) -> bool {
    alt (struct(cx, ty)) {
        case (ty_bool) { ret true; }
        case (_) { ret false; }
    }
}

fn type_is_structural(&ctxt cx, &t ty) -> bool {
    alt (struct(cx, ty)) {
        case (ty_tup(_)) { ret true; }
        case (ty_rec(_)) { ret true; }
        case (ty_tag(_, _)) { ret true; }
        case (ty_fn(_, _, _, _, _)) { ret true; }
        case (ty_obj(_)) { ret true; }
        case (ty_ivec(_)) { ret true; }
        case (ty_istr) { ret true; }
        case (_) { ret false; }
    }
}

fn type_is_sequence(&ctxt cx, &t ty) -> bool {
    alt (struct(cx, ty)) {
        case (ty_str) { ret true; }
        case (ty_istr) { ret true; }
        case (ty_vec(_)) { ret true; }
        case (ty_ivec(_)) { ret true; }
        case (_) { ret false; }
    }
}

fn sequence_is_interior(&ctxt cx, &t ty) -> bool {
    alt (struct(cx, ty)) {
        case (
             // TODO: Or-patterns
             ty::ty_vec(_)) {
            ret false;
        }
        case (ty::ty_str) { ret false; }
        case (ty::ty_ivec(_)) { ret true; }
        case (ty::ty_istr) { ret true; }
        case (_) {
            cx.sess.bug("sequence_is_interior called on non-sequence type");
        }
    }
}

fn sequence_element_type(&ctxt cx, &t ty) -> t {
    alt (struct(cx, ty)) {
        case (ty_str) { ret mk_mach(cx, ty_u8); }
        case (ty_istr) { ret mk_mach(cx, ty_u8); }
        case (ty_vec(?mt)) { ret mt.ty; }
        case (ty_ivec(?mt)) { ret mt.ty; }
        case (_) {
            cx.sess.bug("sequence_element_type called on non-sequence value");
        }
    }
}

fn type_is_tup_like(&ctxt cx, &t ty) -> bool {
    alt (struct(cx, ty)) {
        case (ty_box(_)) { ret true; }
        case (ty_tup(_)) { ret true; }
        case (ty_rec(_)) { ret true; }
        case (ty_tag(_, _)) { ret true; }
        case (_) { ret false; }
    }
}

fn get_element_type(&ctxt cx, &t ty, uint i) -> t {
    assert (type_is_tup_like(cx, ty));
    alt (struct(cx, ty)) {
        case (ty_tup(?mts)) { ret mts.(i).ty; }
        case (ty_rec(?flds)) { ret flds.(i).mt.ty; }
    }
    // NB: This is not exhaustive -- struct(cx, ty) could be a box or a
    // tag.

    cx.sess.bug("get_element_type called on a value other than a " +
                    "tuple or record");
}

fn type_is_box(&ctxt cx, &t ty) -> bool {
    alt (struct(cx, ty)) {
        case (ty_box(_)) { ret true; }
        case (_) { ret false; }
    }
}

fn type_is_boxed(&ctxt cx, &t ty) -> bool {
    alt (struct(cx, ty)) {
        case (ty_str) { ret true; }
        case (ty_vec(_)) { ret true; }
        case (ty_box(_)) { ret true; }
        case (ty_port(_)) { ret true; }
        case (ty_chan(_)) { ret true; }
        case (ty_task) { ret true; }
        case (_) { ret false; }
    }
}

fn type_is_scalar(&ctxt cx, &t ty) -> bool {
    alt (struct(cx, ty)) {
        case (ty_nil) { ret true; }
        case (ty_bool) { ret true; }
        case (ty_int) { ret true; }
        case (ty_float) { ret true; }
        case (ty_uint) { ret true; }
        case (ty_machine(_)) { ret true; }
        case (ty_char) { ret true; }
        case (ty_type) { ret true; }
        case (ty_native) { ret true; }
        case (ty_ptr(_)) { ret true; }
        case (_) { ret false; }
    }
}

fn type_has_pointers(&ctxt cx, &t ty) -> bool {
    alt (struct(cx, ty)) {
        case (
             // scalar types
             ty_nil) {
            ret false;
        }
        case (ty_bot) { ret false; }
        case (ty_bool) { ret false; }
        case (ty_int) { ret false; }
        case (ty_float) { ret false; }
        case (ty_uint) { ret false; }
        case (ty_machine(_)) { ret false; }
        case (ty_char) { ret false; }
        case (ty_type) { ret false; }
        case (ty_native) { ret false; }
        case (ty_tup(?elts)) {
            for (mt m in elts) {
                if (type_has_pointers(cx, m.ty)) { ret true; }
            }
            ret false;
        }
        case (ty_rec(?flds)) {
            for (field f in flds) {
                if (type_has_pointers(cx, f.mt.ty)) { ret true; }
            }
            ret false;
        }
        case (ty_tag(?did, ?tps)) {
            auto variants = tag_variants(cx, did);
            for (variant_info variant in variants) {
                auto tup_ty = mk_imm_tup(cx, variant.args);
                // Perform any type parameter substitutions.

                tup_ty = substitute_type_params(cx, tps, tup_ty);
                if (type_has_pointers(cx, tup_ty)) { ret true; }
            }
            ret false;
        }
        case (_) { ret true; }
    }
}


// FIXME: should we just return true for native types in
// type_is_scalar?
fn type_is_native(&ctxt cx, &t ty) -> bool {
    alt (struct(cx, ty)) {
        case (ty_native) { ret true; }
        case (_) { ret false; }
    }
}

fn type_has_dynamic_size(&ctxt cx, &t ty) -> bool {
    alt (struct(cx, ty)) {
        case (ty_tup(?mts)) {
            auto i = 0u;
            while (i < vec::len[mt](mts)) {
                if (type_has_dynamic_size(cx, mts.(i).ty)) { ret true; }
                i += 1u;
            }
        }
        case (ty_rec(?fields)) {
            auto i = 0u;
            while (i < vec::len[field](fields)) {
                if (type_has_dynamic_size(cx, fields.(i).mt.ty)) { ret true; }
                i += 1u;
            }
        }
        case (ty_tag(_, ?subtys)) {
            auto i = 0u;
            while (i < vec::len[t](subtys)) {
                if (type_has_dynamic_size(cx, subtys.(i))) { ret true; }
                i += 1u;
            }
        }
        case (ty_param(_)) { ret true; }
        case (_) {/* fall through */ }
    }
    ret false;
}

fn type_is_integral(&ctxt cx, &t ty) -> bool {
    alt (struct(cx, ty)) {
        case (ty_int) { ret true; }
        case (ty_uint) { ret true; }
        case (ty_machine(?m)) {
            alt (m) {
                case (ty_i8) { ret true; }
                case (ty_i16) { ret true; }
                case (ty_i32) { ret true; }
                case (ty_i64) { ret true; }
                case (ty_u8) { ret true; }
                case (ty_u16) { ret true; }
                case (ty_u32) { ret true; }
                case (ty_u64) { ret true; }
                case (_) { ret false; }
            }
        }
        case (ty_char) { ret true; }
        case (_) { ret false; }
    }
}

fn type_is_fp(&ctxt cx, &t ty) -> bool {
    alt (struct(cx, ty)) {
        case (ty_machine(?tm)) {
            alt (tm) {
                case (ty_f32) { ret true; }
                case (ty_f64) { ret true; }
                case (_) { ret false; }
            }
        }
        case (ty_float) { ret true; }
        case (_) { ret false; }
    }
}

fn type_is_signed(&ctxt cx, &t ty) -> bool {
    alt (struct(cx, ty)) {
        case (ty_int) { ret true; }
        case (ty_machine(?tm)) {
            alt (tm) {
                case (ty_i8) { ret true; }
                case (ty_i16) { ret true; }
                case (ty_i32) { ret true; }
                case (ty_i64) { ret true; }
                case (_) { ret false; }
            }
        }
        case (_) { ret false; }
    }
}

fn type_param(&ctxt cx, &t ty) -> option::t[uint] {
    alt (struct(cx, ty)) {
        case (ty_param(?id)) { ret some[uint](id); }
        case (_) {/* fall through */ }
    }
    ret none[uint];
}

fn def_to_str(&ast::def_id did) -> str { ret #fmt("%d:%d", did._0, did._1); }


// Type hashing. This function is private to this module (and slow); external
// users should use `hash_ty()` instead.
fn hash_type_structure(&sty st) -> uint {
    fn hash_uint(uint id, uint n) -> uint {
        auto h = id;
        h += h << 5u + n;
        ret h;
    }
    fn hash_def(uint id, ast::def_id did) -> uint {
        auto h = id;
        h += h << 5u + (did._0 as uint);
        h += h << 5u + (did._1 as uint);
        ret h;
    }
    fn hash_subty(uint id, &t subty) -> uint {
        auto h = id;
        h += h << 5u + hash_ty(subty);
        ret h;
    }
    fn hash_fn(uint id, &vec[arg] args, &t rty) -> uint {
        auto h = id;
        for (arg a in args) { h += h << 5u + hash_ty(a.ty); }
        h += h << 5u + hash_ty(rty);
        ret h;
    }
    alt (st) {
        case (ty_nil) { ret 0u; }
        case (ty_bool) { ret 1u; }
        case (ty_int) { ret 2u; }
        case (ty_float) { ret 3u; }
        case (ty_uint) { ret 4u; }
        case (ty_machine(?tm)) {
            alt (tm) {
                case (ty_i8) { ret 5u; }
                case (ty_i16) { ret 6u; }
                case (ty_i32) { ret 7u; }
                case (ty_i64) { ret 8u; }
                case (ty_u8) { ret 9u; }
                case (ty_u16) { ret 10u; }
                case (ty_u32) { ret 11u; }
                case (ty_u64) { ret 12u; }
                case (ty_f32) { ret 13u; }
                case (ty_f64) { ret 14u; }
            }
        }
        case (ty_char) { ret 15u; }
        case (ty_str) { ret 16u; }
        case (ty_istr) { ret 17u; }
        case (ty_tag(?did, ?tys)) {
            auto h = hash_def(18u, did);
            for (t typ in tys) { h += h << 5u + hash_ty(typ); }
            ret h;
        }
        case (ty_box(?mt)) { ret hash_subty(19u, mt.ty); }
        case (ty_vec(?mt)) { ret hash_subty(20u, mt.ty); }
        case (ty_ivec(?mt)) { ret hash_subty(21u, mt.ty); }
        case (ty_port(?typ)) { ret hash_subty(22u, typ); }
        case (ty_chan(?typ)) { ret hash_subty(23u, typ); }
        case (ty_task) { ret 24u; }
        case (ty_tup(?mts)) {
            auto h = 25u;
            for (mt tm in mts) { h += h << 5u + hash_ty(tm.ty); }
            ret h;
        }
        case (ty_rec(?fields)) {
            auto h = 26u;
            for (field f in fields) { h += h << 5u + hash_ty(f.mt.ty); }
            ret h;
        }
        case (
             // ???
             ty_fn(_, ?args, ?rty, _, _)) {
            ret hash_fn(27u, args, rty);
        }
        case (ty_native_fn(_, ?args, ?rty)) { ret hash_fn(28u, args, rty); }
        case (ty_obj(?methods)) {
            auto h = 29u;
            for (method m in methods) { h += h << 5u + str::hash(m.ident); }
            ret h;
        }
        case (ty_var(?v)) { ret hash_uint(30u, v as uint); }
        case (ty_param(?pid)) { ret hash_uint(31u, pid); }
        case (ty_type) { ret 32u; }
        case (ty_native) { ret 33u; }
        case (ty_bot) { ret 34u; }
        case (ty_ptr(?mt)) { ret hash_subty(35u, mt.ty); }
    }
}

fn hash_type_info(&sty st, &option::t[str] cname_opt) -> uint {
    auto h = hash_type_structure(st);
    alt (cname_opt) {
        case (none) {/* no-op */ }
        case (some(?s)) { h += h << 5u + str::hash(s); }
    }
    ret h;
}

fn hash_raw_ty(&raw_t rt) -> uint { ret rt.hash; }

fn hash_ty(&t typ) -> uint { ret typ; }


// Type equality. This function is private to this module (and slow); external
// users should use `eq_ty()` instead.
fn eq_int(&uint x, &uint y) -> bool { ret x == y; }

fn arg_eq[T](&fn(&T, &T) -> bool  eq, @ast::constr_arg_general[T] a,
             @ast::constr_arg_general[T] b) -> bool {
    alt (a.node) {
        case (ast::carg_base) {
            alt (b.node) {
                case (ast::carg_base) { ret true; }
                case (_) { ret false; }
            }
        }
        case (ast::carg_ident(?s)) {
            alt (b.node) {
                case (ast::carg_ident(?t)) { ret eq(s, t); }
                case (_) { ret false; }
            }
        }
        case (ast::carg_lit(?l)) {
            alt (b.node) {
                case (ast::carg_lit(?m)) { ret lit_eq(l, m); }
                case (_) { ret false; }
            }
        }
    }
}

fn args_eq[T](fn(&T, &T) -> bool  eq, vec[@ast::constr_arg_general[T]] a,
              vec[@ast::constr_arg_general[T]] b) -> bool {
    let uint i = 0u;
    for (@ast::constr_arg_general[T] arg in a) {
        if (!arg_eq(eq, arg, b.(i))) { ret false; }
        i += 1u;
    }
    ret true;
}

fn constr_eq(&@constr_def c, &@constr_def d) -> bool {
    ret path_to_str(c.node.path) == path_to_str(d.node.path) &&
             // FIXME: hack
            args_eq(eq_int, c.node.args, d.node.args);
}

fn constrs_eq(&vec[@constr_def] cs, &vec[@constr_def] ds) -> bool {
    if (vec::len(cs) != vec::len(ds)) { ret false; }
    auto i = 0u;
    for (@constr_def c in cs) {
        if (!constr_eq(c, ds.(i))) { ret false; }
        i += 1u;
    }
    ret true;
}

fn equal_type_structures(&sty a, &sty b) -> bool {
    fn equal_mt(&mt a, &mt b) -> bool {
        ret a.mut == b.mut && eq_ty(a.ty, b.ty);
    }
    fn equal_fn(&vec[arg] args_a, &t rty_a, &vec[arg] args_b, &t rty_b) ->
       bool {
        if (!eq_ty(rty_a, rty_b)) { ret false; }
        auto len = vec::len[arg](args_a);
        if (len != vec::len[arg](args_b)) { ret false; }
        auto i = 0u;
        while (i < len) {
            auto arg_a = args_a.(i);
            auto arg_b = args_b.(i);
            if (arg_a.mode != arg_b.mode) { ret false; }
            if (!eq_ty(arg_a.ty, arg_b.ty)) { ret false; }
            i += 1u;
        }
        ret true;
    }
    fn equal_def(&ast::def_id did_a, &ast::def_id did_b) -> bool {
        ret did_a._0 == did_b._0 && did_a._1 == did_b._1;
    }
    alt (a) {
        case (ty_nil) {
            alt (b) { case (ty_nil) { ret true; } case (_) { ret false; } }
        }
        case (ty_bot) {
            alt (b) { case (ty_bot) { ret true; } case (_) { ret false; } }
        }
        case (ty_bool) {
            alt (b) { case (ty_bool) { ret true; } case (_) { ret false; } }
        }
        case (ty_int) {
            alt (b) { case (ty_int) { ret true; } case (_) { ret false; } }
        }
        case (ty_float) {
            alt (b) { case (ty_float) { ret true; } case (_) { ret false; } }
        }
        case (ty_uint) {
            alt (b) { case (ty_uint) { ret true; } case (_) { ret false; } }
        }
        case (ty_machine(?tm_a)) {
            alt (b) {
                case (ty_machine(?tm_b)) {
                    ret hash_type_structure(a) == hash_type_structure(b);
                }
                case (_) { ret false; }
            }
        }
        case (ty_char) {
            alt (b) { case (ty_char) { ret true; } case (_) { ret false; } }
        }
        case (ty_str) {
            alt (b) { case (ty_str) { ret true; } case (_) { ret false; } }
        }
        case (ty_istr) {
            alt (b) { case (ty_istr) { ret true; } case (_) { ret false; } }
        }
        case (ty_tag(?id_a, ?tys_a)) {
            alt (b) {
                case (ty_tag(?id_b, ?tys_b)) {
                    if (!equal_def(id_a, id_b)) { ret false; }
                    auto len = vec::len[t](tys_a);
                    if (len != vec::len[t](tys_b)) { ret false; }
                    auto i = 0u;
                    while (i < len) {
                        if (!eq_ty(tys_a.(i), tys_b.(i))) { ret false; }
                        i += 1u;
                    }
                    ret true;
                }
                case (_) { ret false; }
            }
        }
        case (ty_box(?mt_a)) {
            alt (b) {
                case (ty_box(?mt_b)) { ret equal_mt(mt_a, mt_b); }
                case (_) { ret false; }
            }
        }
        case (ty_vec(?mt_a)) {
            alt (b) {
                case (ty_vec(?mt_b)) { ret equal_mt(mt_a, mt_b); }
                case (_) { ret false; }
            }
        }
        case (ty_ivec(?mt_a)) {
            alt (b) {
                case (ty_ivec(?mt_b)) { ret equal_mt(mt_a, mt_b); }
                case (_) { ret false; }
            }
        }
        case (ty_ptr(?mt_a)) {
            alt (b) {
                case (ty_ptr(?mt_b)) { ret equal_mt(mt_a, mt_b); }
                case (_) { ret false; }
            }
        }
        case (ty_port(?t_a)) {
            alt (b) {
                case (ty_port(?t_b)) { ret eq_ty(t_a, t_b); }
                case (_) { ret false; }
            }
        }
        case (ty_chan(?t_a)) {
            alt (b) {
                case (ty_chan(?t_b)) { ret eq_ty(t_a, t_b); }
                case (_) { ret false; }
            }
        }
        case (ty_task) {
            alt (b) { case (ty_task) { ret true; } case (_) { ret false; } }
        }
        case (ty_tup(?mts_a)) {
            alt (b) {
                case (ty_tup(?mts_b)) {
                    auto len = vec::len[mt](mts_a);
                    if (len != vec::len[mt](mts_b)) { ret false; }
                    auto i = 0u;
                    while (i < len) {
                        if (!equal_mt(mts_a.(i), mts_b.(i))) { ret false; }
                        i += 1u;
                    }
                    ret true;
                }
                case (_) { ret false; }
            }
        }
        case (ty_rec(?flds_a)) {
            alt (b) {
                case (ty_rec(?flds_b)) {
                    auto len = vec::len[field](flds_a);
                    if (len != vec::len[field](flds_b)) { ret false; }
                    auto i = 0u;
                    while (i < len) {
                        auto fld_a = flds_a.(i);
                        auto fld_b = flds_b.(i);
                        if (!str::eq(fld_a.ident, fld_b.ident) ||
                                !equal_mt(fld_a.mt, fld_b.mt)) {
                            ret false;
                        }
                        i += 1u;
                    }
                    ret true;
                }
                case (_) { ret false; }
            }
        }
        case (ty_fn(?p_a, ?args_a, ?rty_a, ?cf_a, ?constrs_a)) {
            alt (b) {
                case (ty_fn(?p_b, ?args_b, ?rty_b, ?cf_b, ?constrs_b)) {
                    ret p_a == p_b && cf_a == cf_b &&
                            constrs_eq(constrs_a, constrs_b) &&
                            equal_fn(args_a, rty_a, args_b, rty_b);
                }
                case (_) { ret false; }
            }
        }
        case (ty_native_fn(?abi_a, ?args_a, ?rty_a)) {
            alt (b) {
                case (ty_native_fn(?abi_b, ?args_b, ?rty_b)) {
                    ret abi_a == abi_b &&
                            equal_fn(args_a, rty_a, args_b, rty_b);
                }
                case (_) { ret false; }
            }
        }
        case (ty_obj(?methods_a)) {
            alt (b) {
                case (ty_obj(?methods_b)) {
                    auto len = vec::len[method](methods_a);
                    if (len != vec::len[method](methods_b)) { ret false; }
                    auto i = 0u;
                    while (i < len) {
                        auto m_a = methods_a.(i);
                        auto m_b = methods_b.(i);
                        if (m_a.proto != m_b.proto ||
                                !str::eq(m_a.ident, m_b.ident) ||
                                !equal_fn(m_a.inputs, m_a.output, m_b.inputs,
                                          m_b.output)) {
                            ret false;
                        }
                        i += 1u;
                    }
                    ret true;
                }
                case (_) { ret false; }
            }
        }
        case (ty_var(?v_a)) {
            alt (b) {
                case (ty_var(?v_b)) { ret v_a == v_b; }
                case (_) { ret false; }
            }
        }
        case (ty_param(?pid_a)) {
            alt (b) {
                case (ty_param(?pid_b)) { ret pid_a == pid_b; }
                case (_) { ret false; }
            }
        }
        case (ty_type) {
            alt (b) { case (ty_type) { ret true; } case (_) { ret false; } }
        }
        case (ty_native) {
            alt (b) { case (ty_native) { ret true; } case (_) { ret false; } }
        }
    }
}


// An expensive type equality function. This function is private to this
// module.
//
// FIXME: Use structural comparison, but this loops forever and segfaults.
fn eq_raw_ty(&raw_t a, &raw_t b) -> bool {
    // Check hashes (fast path).

    if (a.hash != b.hash) { ret false; }
    // Check canonical names.

    alt (a.cname) {
        case (none) {
            alt (b.cname) {
                case (none[str]) {/* ok */ }
                case (_) { ret false; }
            }
        }
        case (some(?s_a)) {
            alt (b.cname) {
                case (some(?s_b)) { if (!str::eq(s_a, s_b)) { ret false; } }
                case (_) { ret false; }
            }
        }
    }
    // Check structures.

    ret equal_type_structures(a.struct, b.struct);
}


// This is the equality function the public should use. It works as long as
// the types are interned.
fn eq_ty(&t a, &t b) -> bool { ret a == b; }


// Type lookups
fn ann_to_ty_param_substs_opt_and_ty(&ctxt cx, &ast::ann ann) ->
   ty_param_substs_opt_and_ty {

    // Pull out the node type table.
    alt ({ cx.node_types.(ann.id) }) {
        case (none) {
            cx.sess.bug("ann_to_ty_param_substs_opt_and_ty() called on an " +
                            "untyped node");
        }
        case (some(?tpot)) { ret tpot; }
    }
}

fn ann_to_type(&ctxt cx, &ast::ann ann) -> t {
    ret ann_to_ty_param_substs_opt_and_ty(cx, ann)._1;
}

fn ann_to_type_params(&ctxt cx, &ast::ann ann) -> vec[t] {
    alt (ann_to_ty_param_substs_opt_and_ty(cx, ann)._0) {
        case (none) { let vec[t] result = []; ret result; }
        case (some(?tps)) { ret tps; }
    }
}

fn ann_has_type_params(&ctxt cx, &ast::ann ann) -> bool {
    auto tpt = ann_to_ty_param_substs_opt_and_ty(cx, ann);
    ret !option::is_none[vec[t]](tpt._0);
}


// Returns a type with type parameter substitutions performed if applicable.
fn ty_param_substs_opt_and_ty_to_monotype(&ctxt cx,
                                          &ty_param_substs_opt_and_ty tpot) ->
   t {
    alt (tpot._0) {
        case (none) { ret tpot._1; }
        case (some(?tps)) { ret substitute_type_params(cx, tps, tpot._1); }
    }
}


// Returns the type of an annotation, with type parameter substitutions
// performed if applicable.
fn ann_to_monotype(&ctxt cx, ast::ann a) -> t {
    auto tpot = ann_to_ty_param_substs_opt_and_ty(cx, a);
    ret ty_param_substs_opt_and_ty_to_monotype(cx, tpot);
}


// Returns the number of distinct type parameters in the given type.
fn count_ty_params(&ctxt cx, t ty) -> uint {
    fn counter(&ctxt cx, @mutable vec[uint] param_indices, t ty) {
        alt (struct(cx, ty)) {
            case (ty_param(?param_idx)) {
                auto seen = false;
                for (uint other_param_idx in *param_indices) {
                    if (param_idx == other_param_idx) { seen = true; }
                }
                if (!seen) { *param_indices += [param_idx]; }
            }
            case (_) {/* fall through */ }
        }
    }
    let vec[uint] v = []; // FIXME: typechecker botch

    let @mutable vec[uint] param_indices = @mutable v;
    auto f = bind counter(cx, param_indices, _);
    walk_ty(cx, f, ty);
    ret vec::len[uint](*param_indices);
}

fn type_contains_vars(&ctxt cx, &t typ) -> bool {
    ret interner::get[raw_t](*cx.ts, typ).has_vars;
}

fn type_contains_params(&ctxt cx, &t typ) -> bool {
    ret interner::get[raw_t](*cx.ts, typ).has_params;
}


// Type accessors for substructures of types
fn ty_fn_args(&ctxt cx, &t fty) -> vec[arg] {
    alt (struct(cx, fty)) {
        case (ty::ty_fn(_, ?a, _, _, _)) { ret a; }
        case (ty::ty_native_fn(_, ?a, _)) { ret a; }
    }
    cx.sess.bug("ty_fn_args() called on non-fn type");
}

fn ty_fn_proto(&ctxt cx, &t fty) -> ast::proto {
    alt (struct(cx, fty)) { case (ty::ty_fn(?p, _, _, _, _)) { ret p; } }
    cx.sess.bug("ty_fn_proto() called on non-fn type");
}

fn ty_fn_abi(&ctxt cx, &t fty) -> ast::native_abi {
    alt (struct(cx, fty)) { case (ty::ty_native_fn(?a, _, _)) { ret a; } }
    cx.sess.bug("ty_fn_abi() called on non-native-fn type");
}

fn ty_fn_ret(&ctxt cx, &t fty) -> t {
    alt (struct(cx, fty)) {
        case (ty::ty_fn(_, _, ?r, _, _)) { ret r; }
        case (ty::ty_native_fn(_, _, ?r)) { ret r; }
    }
    cx.sess.bug("ty_fn_ret() called on non-fn type");
}

fn is_fn_ty(&ctxt cx, &t fty) -> bool {
    alt (struct(cx, fty)) {
        case (ty::ty_fn(_, _, _, _, _)) { ret true; }
        case (ty::ty_native_fn(_, _, _)) { ret true; }
        case (_) { ret false; }
    }
}

fn ty_var_id(&ctxt cx, t typ) -> int {
    alt (struct(cx, typ)) {
        case (ty::ty_var(?vid)) { ret vid; }
        case (_) { log_err "ty_var_id called on non-var ty"; fail; }
    }
}


// Type accessors for AST nodes
fn block_ty(&ctxt cx, &ast::block b) -> t { ret ann_to_type(cx, b.node.a); }


// Returns the type of a pattern as a monotype. Like @expr_ty, this function
// doesn't provide type parameter substitutions.
fn pat_ty(&ctxt cx, &@ast::pat pat) -> t {
    ret ann_to_monotype(cx, pat_ann(pat));
}

fn item_ann(&@ast::item it) -> ast::ann {
    alt (it.node) {
        case (ast::item_const(_, _, _, _, _, ?a)) { ret a; }
        case (ast::item_fn(_, _, _, _, _, ?a)) { ret a; }
        case (ast::item_mod(_, _, _, _)) {
            log_err "a module was passed to item_ann(), " +
                        "but modules haven't annotations";
            fail;
        }
        case (ast::item_native_mod(_, _, _, _)) {
            log_err "a native module was passed to item_ann(), " +
                        "but native modules haven't annotations";
            fail;
        }
        case (ast::item_ty(_, _, _, _, _, ?a)) { ret a; }
        case (ast::item_tag(_, _, _, _, _, ?a)) { ret a; }
        case (ast::item_obj(_, _, _, _, _, ?a)) { ret a; }
    }
}

fn expr_ann(&@ast::expr e) -> ast::ann {
    alt (e.node) {
        case (ast::expr_vec(_, _, _, ?a)) { ret a; }
        case (ast::expr_tup(_, ?a)) { ret a; }
        case (ast::expr_rec(_, _, ?a)) { ret a; }
        case (ast::expr_call(_, _, ?a)) { ret a; }
        case (ast::expr_bind(_, _, ?a)) { ret a; }
        case (ast::expr_binary(_, _, _, ?a)) { ret a; }
        case (ast::expr_unary(_, _, ?a)) { ret a; }
        case (ast::expr_lit(_, ?a)) { ret a; }
        case (ast::expr_cast(_, _, ?a)) { ret a; }
        case (ast::expr_if(_, _, _, ?a)) { ret a; }
        case (ast::expr_while(_, _, ?a)) { ret a; }
        case (ast::expr_for(_, _, _, ?a)) { ret a; }
        case (ast::expr_for_each(_, _, _, ?a)) { ret a; }
        case (ast::expr_do_while(_, _, ?a)) { ret a; }
        case (ast::expr_alt(_, _, ?a)) { ret a; }
        case (ast::expr_fn(_, ?a)) { ret a; }
        case (ast::expr_block(_, ?a)) { ret a; }
        case (ast::expr_move(_, _, ?a)) { ret a; }
        case (ast::expr_assign(_, _, ?a)) { ret a; }
        case (ast::expr_swap(_,_,?a)) { ret a; }
        case (ast::expr_assign_op(_, _, _, ?a)) { ret a; }
        case (ast::expr_send(_, _, ?a)) { ret a; }
        case (ast::expr_recv(_, _, ?a)) { ret a; }
        case (ast::expr_field(_, _, ?a)) { ret a; }
        case (ast::expr_index(_, _, ?a)) { ret a; }
        case (ast::expr_path(_, ?a)) { ret a; }
        case (ast::expr_ext(_, _, _, _, ?a)) { ret a; }
        case (ast::expr_fail(?a, _)) { ret a; }
        case (ast::expr_ret(_, ?a)) { ret a; }
        case (ast::expr_put(_, ?a)) { ret a; }
        case (ast::expr_be(_, ?a)) { ret a; }
        case (ast::expr_log(_, _, ?a)) { ret a; }
        case (ast::expr_assert(_, ?a)) { ret a; }
        case (ast::expr_check(_, ?a)) { ret a; }
        case (ast::expr_port(?a)) { ret a; }
        case (ast::expr_chan(_, ?a)) { ret a; }
        case (ast::expr_anon_obj(_, _, _, ?a)) { ret a; }
        case (ast::expr_break(?a)) { ret a; }
        case (ast::expr_cont(?a)) { ret a; }
        case (ast::expr_self_method(_, ?a)) { ret a; }
        case (ast::expr_spawn(_, _, _, _, ?a)) { ret a; }
    }
}


// Returns the type of an expression as a monotype.
//
// NB: This type doesn't provide type parameter substitutions; e.g. if you
// ask for the type of "id" in "id(3)", it will return "fn(&int) -> int"
// instead of "fn(&T) -> T with T = int". If this isn't what you want, see
// expr_ty_params_and_ty() below.
fn expr_ty(&ctxt cx, &@ast::expr expr) -> t {
    ret ann_to_monotype(cx, expr_ann(expr));
}

fn expr_ty_params_and_ty(&ctxt cx, &@ast::expr expr) -> tup(vec[t], t) {
    auto a = expr_ann(expr);
    ret tup(ann_to_type_params(cx, a), ann_to_type(cx, a));
}

fn expr_has_ty_params(&ctxt cx, &@ast::expr expr) -> bool {
    ret ann_has_type_params(cx, expr_ann(expr));
}

fn decl_local_ty(&ctxt cx, &@ast::local_ l) -> t {
    ret ann_to_type(cx, l.ann);
}

fn stmt_ann(&@ast::stmt s) -> ast::ann {
    alt (s.node) {
        case (ast::stmt_decl(_, ?a)) { ret a; }
        case (ast::stmt_expr(_, ?a)) { ret a; }
        case (ast::stmt_crate_directive(_)) {
            log_err "ty::stmt_ann(): crate directive found";
            fail;
        }
    }
}

fn pat_ann(&@ast::pat p) -> ast::ann {
    alt (p.node) {
        case (ast::pat_wild(?a)) { ret a; }
        case (ast::pat_bind(_, _, ?a)) { ret a; }
        case (ast::pat_lit(_, ?a)) { ret a; }
        case (ast::pat_tag(_, _, ?a)) { ret a; }
    }
}


// Expression utilities
fn field_num(&session::session sess, &span sp, &ast::ident id) -> uint {
    let uint accum = 0u;
    let uint i = 0u;
    for (u8 c in id) {
        if (i == 0u) {
            if (c != '_' as u8) {
                sess.span_err(sp,
                              "bad numeric field on tuple: " +
                                  "missing leading underscore");
            }
        } else {
            if ('0' as u8 <= c && c <= '9' as u8) {
                accum *= 10u;
                accum += (c as uint) - ('0' as uint);
            } else {
                auto s = "";
                s += str::unsafe_from_byte(c);
                sess.span_err(sp,
                              "bad numeric field on tuple: " +
                                  " non-digit character: " + s);
            }
        }
        i += 1u;
    }
    ret accum;
}

fn field_idx(&session::session sess, &span sp, &ast::ident id,
             &vec[field] fields) -> uint {
    let uint i = 0u;
    for (field f in fields) { if (str::eq(f.ident, id)) { ret i; } i += 1u; }
    sess.span_err(sp, "unknown field '" + id + "' of record");
}

fn method_idx(&session::session sess, &span sp, &ast::ident id,
              &vec[method] meths) -> uint {
    let uint i = 0u;
    for (method m in meths) { if (str::eq(m.ident, id)) { ret i; } i += 1u; }
    sess.span_err(sp, "unknown method '" + id + "' of obj");
}

fn sort_methods(&vec[method] meths) -> vec[method] {
    fn method_lteq(&method a, &method b) -> bool {
        ret str::lteq(a.ident, b.ident);
    }
    ret std::sort::merge_sort[method](bind method_lteq(_, _), meths);
}

fn is_lval(&@ast::expr expr) -> bool {
    alt (expr.node) {
        case (ast::expr_field(_, _, _)) { ret true; }
        case (ast::expr_index(_, _, _)) { ret true; }
        case (ast::expr_path(_, _)) { ret true; }
        case (ast::expr_unary(ast::deref, _, _)) { ret true; }
        case (_) { ret false; }
    }
}


// Type unification via Robinson's algorithm (Robinson 1965). Implemented as
// described in Hoder and Voronkov:
//
//     http://www.cs.man.ac.uk/~hoderk/ubench/unification_full.pdf
mod unify {
    tag result { ures_ok(t); ures_err(type_err); }
    tag union_result { unres_ok; unres_err(type_err); }
    tag fixup_result {
        fix_ok(t); // fixup succeeded

        fix_err(int); // fixup failed because a type variable was unresolved

    }
    type var_bindings =
        rec(ufind::ufind sets, smallintmap::smallintmap[t] types);

    type ctxt = rec(@var_bindings vb, ty_ctxt tcx);

    fn mk_var_bindings() -> @var_bindings {
        ret @rec(sets=ufind::make(), types=smallintmap::mk[t]());
    }

    // Unifies two sets.
    fn union(&@ctxt cx, uint set_a, uint set_b) -> union_result {
        ufind::grow(cx.vb.sets, uint::max(set_a, set_b) + 1u);
        auto root_a = ufind::find(cx.vb.sets, set_a);
        auto root_b = ufind::find(cx.vb.sets, set_b);
        ufind::union(cx.vb.sets, set_a, set_b);
        auto root_c = ufind::find(cx.vb.sets, set_a);
        alt (smallintmap::find[t](cx.vb.types, root_a)) {
            case (none[t]) {
                alt (smallintmap::find[t](cx.vb.types, root_b)) {
                    case (none[t]) { ret unres_ok; }
                    case (some[t](?t_b)) {
                        smallintmap::insert[t](cx.vb.types, root_c, t_b);
                        ret unres_ok;
                    }
                }
            }
            case (some[t](?t_a)) {
                alt (smallintmap::find[t](cx.vb.types, root_b)) {
                    case (none[t]) {
                        smallintmap::insert[t](cx.vb.types, root_c, t_a);
                        ret unres_ok;
                    }
                    case (some[t](?t_b)) {
                        alt (unify_step(cx, t_a, t_b)) {
                            case (ures_ok(?t_c)) {
                                smallintmap::insert[t](cx.vb.types, root_c,
                                                       t_c);
                                ret unres_ok;
                            }
                            case (ures_err(?terr)) { ret unres_err(terr); }
                        }
                    }
                }
            }
        }
    }
    fn record_var_binding(&@ctxt cx, int key, t typ) -> result {
        ufind::grow(cx.vb.sets, (key as uint) + 1u);
        auto root = ufind::find(cx.vb.sets, key as uint);
        auto result_type = typ;
        alt (smallintmap::find[t](cx.vb.types, root)) {
            case (some(?old_type)) {
                alt (unify_step(cx, old_type, typ)) {
                    case (ures_ok(?unified_type)) {
                        result_type = unified_type;
                    }
                    case (?res) { ret res; }
                }
            }
            case (none) {/* fall through */ }
        }
        smallintmap::insert[t](cx.vb.types, root, result_type);
        ret ures_ok(typ);
    }

    // Wraps the given type in an appropriate cname.
    //
    // TODO: This doesn't do anything yet. We should carry the cname up from
    // the expected and/or actual types when unification results in a type
    // identical to one or both of the two. The precise algorithm for this is
    // something we'll probably need to develop over time.

    // Simple structural type comparison.
    fn struct_cmp(@ctxt cx, t expected, t actual) -> result {
        if (struct(cx.tcx, expected) == struct(cx.tcx, actual)) {
            ret ures_ok(expected);
        }
        ret ures_err(terr_mismatch);
    }

    // Unifies two mutability flags.
    fn unify_mut(ast::mutability expected, ast::mutability actual) ->
       option::t[ast::mutability] {
        if (expected == actual) { ret some[ast::mutability](expected); }
        if (expected == ast::maybe_mut) { ret some[ast::mutability](actual); }
        if (actual == ast::maybe_mut) { ret some[ast::mutability](expected); }
        ret none[ast::mutability];
    }
    tag fn_common_res {
        fn_common_res_err(result);
        fn_common_res_ok(vec[arg], t);
    }
    fn unify_fn_common(&@ctxt cx, &t expected, &t actual,
                       &vec[arg] expected_inputs, &t expected_output,
                       &vec[arg] actual_inputs, &t actual_output) ->
       fn_common_res {
        auto expected_len = vec::len[arg](expected_inputs);
        auto actual_len = vec::len[arg](actual_inputs);
        if (expected_len != actual_len) {
            ret fn_common_res_err(ures_err(terr_arg_count));
        }
        // TODO: as above, we should have an iter2 iterator.

        let vec[arg] result_ins = [];
        auto i = 0u;
        while (i < expected_len) {
            auto expected_input = expected_inputs.(i);
            auto actual_input = actual_inputs.(i);
            // Unify the result modes.

            auto result_mode;
            if (expected_input.mode != actual_input.mode) {
                // FIXME this is the wrong error

                ret fn_common_res_err(ures_err(terr_arg_count));
            } else { result_mode = expected_input.mode; }
            auto result = unify_step(cx, expected_input.ty, actual_input.ty);
            alt (result) {
                case (ures_ok(?rty)) {
                    result_ins += [rec(mode=result_mode, ty=rty)];
                }
                case (_) { ret fn_common_res_err(result); }
            }
            i += 1u;
        }
        // Check the output.

        auto result = unify_step(cx, expected_output, actual_output);
        alt (result) {
            case (ures_ok(?rty)) { ret fn_common_res_ok(result_ins, rty); }
            case (_) { ret fn_common_res_err(result); }
        }
    }
    fn unify_fn(&@ctxt cx, &ast::proto e_proto, &ast::proto a_proto,
                &t expected, &t actual, &vec[arg] expected_inputs,
                &t expected_output, &vec[arg] actual_inputs, &t actual_output,
                &controlflow expected_cf, &controlflow actual_cf,
                &vec[@constr_def] expected_constrs,
                &vec[@constr_def] actual_constrs) -> result {
        if (e_proto != a_proto) { ret ures_err(terr_mismatch); }
        alt (expected_cf) {
            case (ast::return) { }
            case ( // ok
                 ast::noreturn) {
                alt (actual_cf) {
                    case (ast::noreturn) {
                        // ok

                    }
                    case (_) {
                        /* even though typestate checking is mostly
                           responsible for checking control flow annotations,
                           this check is necessary to ensure that the
                           annotation in an object method matches the
                           declared object type */

                        ret ures_err(terr_controlflow_mismatch);
                    }
                }
            }
        }
        auto t =
            unify_fn_common(cx, expected, actual, expected_inputs,
                            expected_output, actual_inputs, actual_output);
        alt (t) {
            case (fn_common_res_err(?r)) { ret r; }
            case (fn_common_res_ok(?result_ins, ?result_out)) {
                auto t2 =
                    mk_fn(cx.tcx, e_proto, result_ins, result_out, actual_cf,
                          actual_constrs);
                ret ures_ok(t2);
            }
        }
    }
    fn unify_native_fn(&@ctxt cx, &ast::native_abi e_abi,
                       &ast::native_abi a_abi, &t expected, &t actual,
                       &vec[arg] expected_inputs, &t expected_output,
                       &vec[arg] actual_inputs, &t actual_output) -> result {
        if (e_abi != a_abi) { ret ures_err(terr_mismatch); }
        auto t =
            unify_fn_common(cx, expected, actual, expected_inputs,
                            expected_output, actual_inputs, actual_output);
        alt (t) {
            case (fn_common_res_err(?r)) { ret r; }
            case (fn_common_res_ok(?result_ins, ?result_out)) {
                auto t2 = mk_native_fn(cx.tcx, e_abi, result_ins, result_out);
                ret ures_ok(t2);
            }
        }
    }
    fn unify_obj(&@ctxt cx, &t expected, &t actual,
                 &vec[method] expected_meths, &vec[method] actual_meths) ->
       result {
        let vec[method] result_meths = [];
        let uint i = 0u;
        let uint expected_len = vec::len[method](expected_meths);
        let uint actual_len = vec::len[method](actual_meths);
        if (expected_len != actual_len) { ret ures_err(terr_meth_count); }
        while (i < expected_len) {
            auto e_meth = expected_meths.(i);
            auto a_meth = actual_meths.(i);
            if (!str::eq(e_meth.ident, a_meth.ident)) {
                ret ures_err(terr_obj_meths(e_meth.ident, a_meth.ident));
            }
            auto r =
                unify_fn(cx, e_meth.proto, a_meth.proto, expected, actual,
                         e_meth.inputs, e_meth.output, a_meth.inputs,
                         a_meth.output, e_meth.cf, a_meth.cf, e_meth.constrs,
                         a_meth.constrs);
            alt (r) {
                case (ures_ok(?tfn)) {
                    alt (struct(cx.tcx, tfn)) {
                        case (ty_fn(?proto, ?ins, ?out, ?cf, ?constrs)) {
                            result_meths +=
                                [rec(inputs=ins,
                                     output=out,
                                     cf=cf,
                                     constrs=constrs with e_meth)];
                        }
                    }
                }
                case (_) { ret r; }
            }
            i += 1u;
        }
        auto t = mk_obj(cx.tcx, result_meths);
        ret ures_ok(t);
    }

    // If the given type is a variable, returns the structure of that type.
    fn resolve_type_structure(&ty_ctxt tcx, &@var_bindings vb, t typ) ->
       fixup_result {
        alt (struct(tcx, typ)) {
            case (ty_var(?vid)) {
                if (vid as uint >= ufind::set_count(vb.sets)) {
                    ret fix_err(vid);
                }
                auto root_id = ufind::find(vb.sets, vid as uint);
                alt (smallintmap::find[t](vb.types, root_id)) {
                    case (none[t]) { ret fix_err(vid); }
                    case (some[t](?rt)) { ret fix_ok(rt); }
                }
            }
            case (_) { ret fix_ok(typ); }
        }
    }
    fn unify_step(&@ctxt cx, &t expected, &t actual) -> result {
        // TODO: rewrite this using tuple pattern matching when available, to
        // avoid all this rightward drift and spikiness.

        // TODO: occurs check, to make sure we don't loop forever when
        // unifying e.g. 'a and option['a]

        // Fast path.

        if (eq_ty(expected, actual)) { ret ures_ok(expected); }
        // Stage 1: Handle the cases in which one side or another is a type
        // variable.

        alt (struct(cx.tcx, actual)) {
            case (
                 // If the RHS is a variable type, then just do the
                 // appropriate binding.
                 ty::ty_var(?actual_id)) {
                auto actual_n = actual_id as uint;
                alt (struct(cx.tcx, expected)) {
                    case (ty::ty_var(?expected_id)) {
                        auto expected_n = expected_id as uint;
                        union(cx, expected_n, actual_n);
                    }
                    case (_) {

                        // Just bind the type variable to the expected type.
                        alt (record_var_binding(cx, actual_id, expected)) {
                            case (ures_ok(_)) {/* fall through */ }
                            case (?res) { ret res; }
                        }
                    }
                }
                ret ures_ok(mk_var(cx.tcx, actual_id));
            }
            case (_) {/* empty */ }
        }
        alt (struct(cx.tcx, expected)) {
            case (ty::ty_var(?expected_id)) {
                // Add a binding. (`actual` can't actually be a var here.)

                alt (record_var_binding(cx, expected_id, actual)) {
                    case (ures_ok(_)) {/* fall through */ }
                    case (?res) { ret res; }
                }
                ret ures_ok(mk_var(cx.tcx, expected_id));
            }
            case (_) {/* fall through */ }
        }
        // Stage 2: Handle all other cases.

        alt (struct(cx.tcx, actual)) {
            case (ty::ty_bot) { ret ures_ok(expected); }
            case (_) {/* fall through */ }
        }
        alt (struct(cx.tcx, expected)) {
            case (ty::ty_nil) { ret struct_cmp(cx, expected, actual); }
            case (
                 // _|_ unifies with anything
                 ty::ty_bot) {
                ret ures_ok(actual);
            }
            case (ty::ty_bool) { ret struct_cmp(cx, expected, actual); }
            case (ty::ty_int) { ret struct_cmp(cx, expected, actual); }
            case (ty::ty_uint) { ret struct_cmp(cx, expected, actual); }
            case (ty::ty_machine(_)) { ret struct_cmp(cx, expected, actual); }
            case (ty::ty_float) { ret struct_cmp(cx, expected, actual); }
            case (ty::ty_char) { ret struct_cmp(cx, expected, actual); }
            case (ty::ty_str) { ret struct_cmp(cx, expected, actual); }
            case (ty::ty_istr) { ret struct_cmp(cx, expected, actual); }
            case (ty::ty_type) { ret struct_cmp(cx, expected, actual); }
            case (ty::ty_native) { ret struct_cmp(cx, expected, actual); }
            case (ty::ty_param(_)) { ret struct_cmp(cx, expected, actual); }
            case (ty::ty_tag(?expected_id, ?expected_tps)) {
                alt (struct(cx.tcx, actual)) {
                    case (ty::ty_tag(?actual_id, ?actual_tps)) {
                        if (expected_id._0 != actual_id._0 ||
                                expected_id._1 != actual_id._1) {
                            ret ures_err(terr_mismatch);
                        }
                        // TODO: factor this cruft out, see the TODO in the
                        // ty::ty_tup case

                        let vec[t] result_tps = [];
                        auto i = 0u;
                        auto expected_len = vec::len[t](expected_tps);
                        while (i < expected_len) {
                            auto expected_tp = expected_tps.(i);
                            auto actual_tp = actual_tps.(i);
                            auto result =
                                unify_step(cx, expected_tp, actual_tp);
                            alt (result) {
                                case (ures_ok(?rty)) {
                                    vec::push[t](result_tps, rty);
                                }
                                case (_) { ret result; }
                            }
                            i += 1u;
                        }
                        ret ures_ok(mk_tag(cx.tcx, expected_id, result_tps));
                    }
                    case (_) {/* fall through */ }
                }
                ret ures_err(terr_mismatch);
            }
            case (ty::ty_box(?expected_mt)) {
                alt (struct(cx.tcx, actual)) {
                    case (ty::ty_box(?actual_mt)) {
                        auto mut;
                        alt (unify_mut(expected_mt.mut, actual_mt.mut)) {
                            case (none) { ret ures_err(terr_box_mutability); }
                            case (some(?m)) { mut = m; }
                        }
                        auto result =
                            unify_step(cx, expected_mt.ty, actual_mt.ty);
                        alt (result) {
                            case (ures_ok(?result_sub)) {
                                auto mt = rec(ty=result_sub, mut=mut);
                                ret ures_ok(mk_box(cx.tcx, mt));
                            }
                            case (_) { ret result; }
                        }
                    }
                    case (_) { ret ures_err(terr_mismatch); }
                }
            }
            case (ty::ty_vec(?expected_mt)) {
                alt (struct(cx.tcx, actual)) {
                    case (ty::ty_vec(?actual_mt)) {
                        auto mut;
                        alt (unify_mut(expected_mt.mut, actual_mt.mut)) {
                            case (none) { ret ures_err(terr_vec_mutability); }
                            case (some(?m)) { mut = m; }
                        }
                        auto result =
                            unify_step(cx, expected_mt.ty, actual_mt.ty);
                        alt (result) {
                            case (ures_ok(?result_sub)) {
                                auto mt = rec(ty=result_sub, mut=mut);
                                ret ures_ok(mk_vec(cx.tcx, mt));
                            }
                            case (_) { ret result; }
                        }
                    }
                    case (_) { ret ures_err(terr_mismatch); }
                }
            }
            case (ty::ty_ivec(?expected_mt)) {
                alt (struct(cx.tcx, actual)) {
                    case (ty::ty_ivec(?actual_mt)) {
                        auto mut;
                        alt (unify_mut(expected_mt.mut, actual_mt.mut)) {
                            case (none) { ret ures_err(terr_vec_mutability); }
                            case (some(?m)) { mut = m; }
                        }
                        auto result =
                            unify_step(cx, expected_mt.ty, actual_mt.ty);
                        alt (result) {
                            case (ures_ok(?result_sub)) {
                                auto mt = rec(ty=result_sub, mut=mut);
                                ret ures_ok(mk_ivec(cx.tcx, mt));
                            }
                            case (_) { ret result; }
                        }
                    }
                    case (_) { ret ures_err(terr_mismatch); }
                }
            }
            case (ty::ty_port(?expected_sub)) {
                alt (struct(cx.tcx, actual)) {
                    case (ty::ty_port(?actual_sub)) {
                        auto result =
                            unify_step(cx, expected_sub, actual_sub);
                        alt (result) {
                            case (ures_ok(?result_sub)) {
                                ret ures_ok(mk_port(cx.tcx, result_sub));
                            }
                            case (_) { ret result; }
                        }
                    }
                    case (_) { ret ures_err(terr_mismatch); }
                }
            }
            case (ty::ty_chan(?expected_sub)) {
                alt (struct(cx.tcx, actual)) {
                    case (ty::ty_chan(?actual_sub)) {
                        auto result =
                            unify_step(cx, expected_sub, actual_sub);
                        alt (result) {
                            case (ures_ok(?result_sub)) {
                                ret ures_ok(mk_chan(cx.tcx, result_sub));
                            }
                            case (_) { ret result; }
                        }
                    }
                    case (_) { ret ures_err(terr_mismatch); }
                }
            }
            case (ty::ty_tup(?expected_elems)) {
                alt (struct(cx.tcx, actual)) {
                    case (ty::ty_tup(?actual_elems)) {
                        auto expected_len = vec::len[ty::mt](expected_elems);
                        auto actual_len = vec::len[ty::mt](actual_elems);
                        if (expected_len != actual_len) {
                            auto err =
                                terr_tuple_size(expected_len, actual_len);
                            ret ures_err(err);
                        }
                        // TODO: implement an iterator that can iterate over
                        // two arrays simultaneously.

                        let vec[ty::mt] result_elems = [];
                        auto i = 0u;
                        while (i < expected_len) {
                            auto expected_elem = expected_elems.(i);
                            auto actual_elem = actual_elems.(i);
                            auto mut;
                            alt (unify_mut(expected_elem.mut,
                                           actual_elem.mut)) {
                                case (none) {
                                    auto err = terr_tuple_mutability;
                                    ret ures_err(err);
                                }
                                case (some(?m)) { mut = m; }
                            }
                            auto result =
                                unify_step(cx, expected_elem.ty,
                                           actual_elem.ty);
                            alt (result) {
                                case (ures_ok(?rty)) {
                                    auto mt = rec(ty=rty, mut=mut);
                                    result_elems += [mt];
                                }
                                case (_) { ret result; }
                            }
                            i += 1u;
                        }
                        ret ures_ok(mk_tup(cx.tcx, result_elems));
                    }
                    case (_) { ret ures_err(terr_mismatch); }
                }
            }
            case (ty::ty_rec(?expected_fields)) {
                alt (struct(cx.tcx, actual)) {
                    case (ty::ty_rec(?actual_fields)) {
                        auto expected_len = vec::len[field](expected_fields);
                        auto actual_len = vec::len[field](actual_fields);
                        if (expected_len != actual_len) {
                            auto err =
                                terr_record_size(expected_len, actual_len);
                            ret ures_err(err);
                        }
                        // TODO: implement an iterator that can iterate over
                        // two arrays simultaneously.

                        let vec[field] result_fields = [];
                        auto i = 0u;
                        while (i < expected_len) {
                            auto expected_field = expected_fields.(i);
                            auto actual_field = actual_fields.(i);
                            auto mut;
                            alt (unify_mut(expected_field.mt.mut,
                                           actual_field.mt.mut)) {
                                case (none) {
                                    ret ures_err(terr_record_mutability);
                                }
                                case (some(?m)) { mut = m; }
                            }
                            if (!str::eq(expected_field.ident,
                                         actual_field.ident)) {
                                auto err =
                                    terr_record_fields(expected_field.ident,
                                                       actual_field.ident);
                                ret ures_err(err);
                            }
                            auto result =
                                unify_step(cx, expected_field.mt.ty,
                                           actual_field.mt.ty);
                            alt (result) {
                                case (ures_ok(?rty)) {
                                    auto mt = rec(ty=rty, mut=mut);
                                    vec::push[field](result_fields,
                                                     rec(mt=mt
                                                         with
                                                             expected_field));
                                }
                                case (_) { ret result; }
                            }
                            i += 1u;
                        }
                        ret ures_ok(mk_rec(cx.tcx, result_fields));
                    }
                    case (_) { ret ures_err(terr_mismatch); }
                }
            }
            case (ty::ty_fn(?ep, ?expected_inputs, ?expected_output,
                            ?expected_cf, ?expected_constrs)) {
                alt (struct(cx.tcx, actual)) {
                    case (ty::ty_fn(?ap, ?actual_inputs, ?actual_output,
                                    ?actual_cf, ?actual_constrs)) {
                        ret unify_fn(cx, ep, ap, expected, actual,
                                     expected_inputs, expected_output,
                                     actual_inputs, actual_output,
                                     expected_cf, actual_cf, expected_constrs,
                                     actual_constrs);
                    }
                    case (_) { ret ures_err(terr_mismatch); }
                }
            }
            case (ty::ty_native_fn(?e_abi, ?expected_inputs,
                                   ?expected_output)) {
                alt (struct(cx.tcx, actual)) {
                    case (ty::ty_native_fn(?a_abi, ?actual_inputs,
                                           ?actual_output)) {
                        ret unify_native_fn(cx, e_abi, a_abi, expected,
                                            actual, expected_inputs,
                                            expected_output, actual_inputs,
                                            actual_output);
                    }
                    case (_) { ret ures_err(terr_mismatch); }
                }
            }
            case (ty::ty_obj(?expected_meths)) {
                alt (struct(cx.tcx, actual)) {
                    case (ty::ty_obj(?actual_meths)) {
                        ret unify_obj(cx, expected, actual, expected_meths,
                                      actual_meths);
                    }
                    case (_) { ret ures_err(terr_mismatch); }
                }
            }
        }
    }
    fn unify(&t expected, &t actual, &@var_bindings vb, &ty_ctxt tcx) ->
       result {
        auto cx = @rec(vb=vb, tcx=tcx);
        ret unify_step(cx, expected, actual);
    }
    fn dump_var_bindings(ty_ctxt tcx, @var_bindings vb) {
        auto i = 0u;
        while (i < vec::len[ufind::node](vb.sets.nodes)) {
            auto sets = "";
            auto j = 0u;
            while (j < vec::len[option::t[uint]](vb.sets.nodes)) {
                if (ufind::find(vb.sets, j) == i) { sets += #fmt(" %u", j); }
                j += 1u;
            }
            auto typespec;
            alt (smallintmap::find[t](vb.types, i)) {
                case (none[t]) { typespec = ""; }
                case (some[t](?typ)) {
                    typespec = " =" + pretty::ppaux::ty_to_str(tcx, typ);
                }
            }
            log_err #fmt("set %u:%s%s", i, typespec, sets);
            i += 1u;
        }
    }

    // Fixups and substitutions
    fn fixup_vars(ty_ctxt tcx, @var_bindings vb, t typ) -> fixup_result {
        fn subst_vars(ty_ctxt tcx, @var_bindings vb,
                      @mutable option::t[int] unresolved, int vid) -> t {
            if (vid as uint >= ufind::set_count(vb.sets)) {
                *unresolved = some[int](vid);
                ret ty::mk_var(tcx, vid);
            }
            auto root_id = ufind::find(vb.sets, vid as uint);
            alt (smallintmap::find[t](vb.types, root_id)) {
                case (none[t]) {
                    *unresolved = some[int](vid);
                    ret ty::mk_var(tcx, vid);
                }
                case (some[t](?rt)) {
                    ret fold_ty(tcx,
                                fm_var(bind subst_vars(tcx, vb, unresolved,
                                                       _)), rt);
                }
            }
        }
        auto unresolved = @mutable none[int];
        auto rty =
            fold_ty(tcx, fm_var(bind subst_vars(tcx, vb, unresolved, _)),
                    typ);
        auto ur = *unresolved;
        alt (ur) {
            case (none[int]) { ret fix_ok(rty); }
            case (some[int](?var_id)) { ret fix_err(var_id); }
        }
    }
    fn resolve_type_var(&ty_ctxt tcx, &@var_bindings vb, int vid) ->
       fixup_result {
        if (vid as uint >= ufind::set_count(vb.sets)) { ret fix_err(vid); }
        auto root_id = ufind::find(vb.sets, vid as uint);
        alt (smallintmap::find[t](vb.types, root_id)) {
            case (none[t]) { ret fix_err(vid); }
            case (some[t](?rt)) { ret fixup_vars(tcx, vb, rt); }
        }
    }
}

fn type_err_to_str(&ty::type_err err) -> str {
    alt (err) {
        case (terr_mismatch) { ret "types differ"; }
        case (terr_controlflow_mismatch) {
            ret "returning function used where non-returning function" +
                    " was expected";
        }
        case (terr_box_mutability) {
            ret "boxed values differ in mutability";
        }
        case (terr_vec_mutability) { ret "vectors differ in mutability"; }
        case (terr_tuple_size(?e_sz, ?a_sz)) {
            ret "expected a tuple with " + uint::to_str(e_sz, 10u) +
                    " elements but found one with " + uint::to_str(a_sz, 10u)
                    + " elements";
        }
        case (terr_tuple_mutability) {
            ret "tuple elements differ in mutability";
        }
        case (terr_record_size(?e_sz, ?a_sz)) {
            ret "expected a record with " + uint::to_str(e_sz, 10u) +
                    " fields but found one with " + uint::to_str(a_sz, 10u) +
                    " fields";
        }
        case (terr_record_mutability) {
            ret "record elements differ in mutability";
        }
        case (terr_record_fields(?e_fld, ?a_fld)) {
            ret "expected a record with field '" + e_fld +
                    "' but found one with field '" + a_fld + "'";
        }
        case (terr_arg_count) {
            ret "incorrect number of function parameters";
        }
        case (terr_meth_count) { ret "incorrect number of object methods"; }
        case (terr_obj_meths(?e_meth, ?a_meth)) {
            ret "expected an obj with method '" + e_meth +
                    "' but found one with method '" + a_meth + "'";
        }
    }
}


// Converts type parameters in a type to type variables and returns the
// resulting type along with a list of type variable IDs.
fn bind_params_in_type(&ctxt cx, fn() -> int  next_ty_var, t typ,
                       uint ty_param_count) -> tup(vec[int], t) {
    let vec[int] param_var_ids = [];
    auto i = 0u;
    while (i < ty_param_count) { param_var_ids += [next_ty_var()]; i += 1u; }
    fn binder(ctxt cx, vec[int] param_var_ids, fn() -> int  next_ty_var,
              uint index) -> t {
        ret mk_var(cx, param_var_ids.(index));
    }
    auto new_typ =
        fold_ty(cx, fm_param(bind binder(cx, param_var_ids, next_ty_var, _)),
                typ);
    ret tup(param_var_ids, new_typ);
}


// Replaces type parameters in the given type using the given list of
// substitions.
fn substitute_type_params(&ctxt cx, vec[ty::t] substs, t typ) -> t {
    if (!type_contains_params(cx, typ)) { ret typ; }
    fn substituter(ctxt cx, vec[ty::t] substs, uint idx) -> t {
        // FIXME: bounds check can fail

        ret substs.(idx);
    }
    ret fold_ty(cx, fm_param(bind substituter(cx, substs, _)), typ);
}

fn def_has_ty_params(&ast::def def) -> bool {
    alt (def) {
        case (ast::def_fn(_)) { ret true; }
        case (ast::def_obj(_)) { ret true; }
        case (ast::def_obj_field(_)) { ret false; }
        case (ast::def_mod(_)) { ret false; }
        case (ast::def_const(_)) { ret false; }
        case (ast::def_arg(_)) { ret false; }
        case (ast::def_local(_)) { ret false; }
        case (ast::def_variant(_, _)) { ret true; }
        case (ast::def_ty(_)) { ret false; }
        case (ast::def_ty_arg(_)) { ret false; }
        case (ast::def_binding(_)) { ret false; }
        case (ast::def_use(_)) { ret false; }
        case (ast::def_native_ty(_)) { ret false; }
        case (ast::def_native_fn(_)) { ret true; }
    }
}


// Tag information
type variant_info = rec(vec[ty::t] args, ty::t ctor_ty, ast::def_id id);

fn tag_variants(&ctxt cx, &ast::def_id id) -> vec[variant_info] {
    if (cx.sess.get_targ_crate_num() != id._0) {
        ret creader::get_tag_variants(cx, id);
    }
    assert (cx.items.contains_key(id));
    alt (cx.items.get(id)) {
        case (any_item_rust(?item)) {
            alt (item.node) {
                case (ast::item_tag(_, ?variants, _, _, _, _)) {
                    let vec[variant_info] result = [];
                    for (ast::variant variant in variants) {
                        auto ctor_ty = ann_to_monotype(cx, variant.node.ann);
                        let vec[t] arg_tys = [];
                        if (vec::len[ast::variant_arg](variant.node.args) >
                                0u) {
                            for (arg a in ty_fn_args(cx, ctor_ty)) {
                                arg_tys += [a.ty];
                            }
                        }
                        auto did = variant.node.id;
                        result +=
                            [rec(args=arg_tys, ctor_ty=ctor_ty, id=did)];
                    }
                    ret result;
                }
            }
        }
    }
}


// Returns information about the tag variant with the given ID:
fn tag_variant_with_id(&ctxt cx, &ast::def_id tag_id, &ast::def_id variant_id)
   -> variant_info {
    auto variants = tag_variants(cx, tag_id);
    auto i = 0u;
    while (i < vec::len[variant_info](variants)) {
        auto variant = variants.(i);
        if (def_eq(variant.id, variant_id)) { ret variant; }
        i += 1u;
    }
    cx.sess.bug("tag_variant_with_id(): no variant exists with that ID");
}


// If the given item is in an external crate, looks up its type and adds it to
// the type cache. Returns the type parameters and type.
fn lookup_item_type(ctxt cx, ast::def_id did) -> ty_param_count_and_ty {
    if (did._0 == cx.sess.get_targ_crate_num()) {
        // The item is in this crate. The caller should have added it to the
        // type cache already; we simply return it.

        ret cx.tcache.get(did);
    }
    alt (cx.tcache.find(did)) {
        case (some(?tpt)) { ret tpt; }
        case (none) {
            auto tyt = creader::get_type(cx, did);
            cx.tcache.insert(did, tyt);
            ret tyt;
        }
    }
}

fn ret_ty_of_fn_ty(ctxt cx, t a_ty) -> t {
    alt (ty::struct(cx, a_ty)) {
        case (ty::ty_fn(_, _, ?ret_ty, _, _)) { ret ret_ty; }
        case (_) {
            cx.sess.bug("ret_ty_of_fn_ty() called on non-function type");
        }
    }
}

fn ret_ty_of_fn(ctxt cx, ast::ann ann) -> t {
    ret ret_ty_of_fn_ty(cx, ann_to_type(cx, ann));
}


// NB: This function requires that the given type has no variables. So, inside
// typeck, you should use typeck::strip_boxes() instead.
fn strip_boxes(&ctxt cx, &ty::t t) -> ty::t {
    auto t1 = t;
    while (true) {
        alt (struct(cx, t1)) {
            case (ty::ty_box(?inner)) { t1 = inner.ty; }
            case (_) { ret t1; }
        }
    }
    fail;
}
// Local Variables:
// mode: rust
// fill-column: 78;
// indent-tabs-mode: nil
// c-basic-offset: 4
// buffer-file-coding-system: utf-8-unix
// compile-command: "make -k -C $RBUILD 2>&1 | sed -e 's/\\/x\\//x:\\//g'";
// End:
