// trans.rs: Translate the completed AST to the LLVM IR.
//
// Some functions here, such as trans_block and trans_expr, return a value --
// the result of the translation to LLVM -- while others, such as trans_fn,
// trans_obj, and trans_item, are called only for the side effect of adding a
// particular definition to the LLVM IR output we're producing.
//
// Hopefully useful general knowledge about trans:
//
//   * There's no way to find out the ty::t type of a ValueRef.  Doing so
//     would be "trying to get the eggs out of an omelette" (credit:
//     pcwalton).  You can, instead, find out its TypeRef by calling val_ty,
//     but many TypeRefs correspond to one ty::t; for instance, tup(int, int,
//     int) and rec(x=int, y=int, z=int) will have the same TypeRef.
import std::int;
import std::str;
import std::uint;
import std::str::rustrt::sbuf;
import std::map;
import std::map::hashmap;
import std::option;
import std::option::some;
import std::option::none;
import std::fs;
import std::time;
import syntax::ast;
import syntax::walk;
import driver::session;
import middle::ty;
import middle::freevars::*;
import back::link;
import back::x86;
import back::abi;
import back::upcall;
import syntax::visit;
import visit::vt;
import util::common;
import util::common::*;
import std::map::new_int_hash;
import std::map::new_str_hash;
import syntax::codemap::span;
import lib::llvm::llvm;
import lib::llvm::builder;
import lib::llvm::target_data;
import lib::llvm::type_names;
import lib::llvm::mk_target_data;
import lib::llvm::mk_type_names;
import lib::llvm::llvm::ModuleRef;
import lib::llvm::llvm::ValueRef;
import lib::llvm::llvm::TypeRef;
import lib::llvm::llvm::TypeHandleRef;
import lib::llvm::llvm::BuilderRef;
import lib::llvm::llvm::BasicBlockRef;
import lib::llvm::False;
import lib::llvm::True;
import lib::llvm::Bool;
import link::mangle_internal_name_by_type_only;
import link::mangle_internal_name_by_seq;
import link::mangle_internal_name_by_path;
import link::mangle_internal_name_by_path_and_seq;
import link::mangle_exported_name;
import metadata::creader;
import metadata::csearch;
import metadata::cstore;
import util::ppaux::ty_to_str;
import util::ppaux::ty_to_short_str;
import syntax::print::pprust::expr_to_str;
import syntax::print::pprust::path_to_str;

import trans_common::*;

import trans_comm::trans_port;
import trans_comm::trans_chan;
import trans_comm::trans_spawn;
import trans_comm::trans_send;
import trans_comm::trans_recv;

// This function now fails if called on a type with dynamic size (as its
// return value was always meaningless in that case anyhow). Beware!
//
// TODO: Enforce via a predicate.
fn type_of(&@crate_ctxt cx, &span sp, &ty::t t) -> TypeRef {
    if (ty::type_has_dynamic_size(cx.tcx, t)) {
        cx.sess.span_fatal(sp,
                         "type_of() called on a type with dynamic size: " +
                             ty_to_str(cx.tcx, t));
    }
    ret type_of_inner(cx, sp, t);
}

fn type_of_explicit_args(&@crate_ctxt cx, &span sp, &ty::arg[] inputs)
        -> TypeRef[] {
    let TypeRef[] atys = ~[];
    for (ty::arg arg in inputs) {
        if (ty::type_has_dynamic_size(cx.tcx, arg.ty)) {
            assert (arg.mode != ty::mo_val);
            atys += ~[T_typaram_ptr(cx.tn)];
        } else {
            let TypeRef t;
            alt (arg.mode) {
                case (ty::mo_alias(_)) {
                    t = T_ptr(type_of_inner(cx, sp, arg.ty));
                }
                case (_) { t = type_of_inner(cx, sp, arg.ty); }
            }
            atys += ~[t];
        }
    }
    ret atys;
}


// NB: must keep 4 fns in sync:
//
//  - type_of_fn_full
//  - create_llargs_for_fn_args.
//  - new_fn_ctxt
//  - trans_args
fn type_of_fn_full(&@crate_ctxt cx, &span sp, ast::proto proto,
                   bool is_method, &ty::arg[] inputs,
                   &ty::t output, uint ty_param_count) -> TypeRef {
    let TypeRef[] atys = ~[];

    // Arg 0: Output pointer.
    if (ty::type_has_dynamic_size(cx.tcx, output)) {
        atys += ~[T_typaram_ptr(cx.tn)];
    } else {
        atys += ~[T_ptr(type_of_inner(cx, sp, output))];
    }

    // Arg 1: task pointer.
    atys += ~[T_taskptr(*cx)];

    // Arg 2: Env (closure-bindings / self-obj)
    if (is_method) {
        atys += ~[cx.rust_object_type];
    } else {
        atys += ~[T_opaque_closure_ptr(*cx)];
    }

    // Args >3: ty params, if not acquired via capture...
    if (!is_method) {
        auto i = 0u;
        while (i < ty_param_count) {
            atys += ~[T_ptr(cx.tydesc_type)];
            i += 1u;
        }
    }
    if (proto == ast::proto_iter) {
        // If it's an iter, the 'output' type of the iter is actually the
        // *input* type of the function we're given as our iter-block
        // argument.
        atys +=
            ~[T_fn_pair(*cx,
                        type_of_fn_full(cx, sp, ast::proto_fn, false,
                                        ~[rec(mode=ty::mo_alias(false),
                                             ty=output)], ty::mk_nil(cx.tcx),
                                        0u))];
    }

    // ... then explicit args.
    atys += type_of_explicit_args(cx, sp, inputs);
    ret T_fn(atys, llvm::LLVMVoidType());
}

fn type_of_fn(&@crate_ctxt cx, &span sp, ast::proto proto,
              &ty::arg[] inputs, &ty::t output, uint ty_param_count) ->
   TypeRef {
    ret type_of_fn_full(cx, sp, proto, false, inputs, output,
                        ty_param_count);
}

fn type_of_native_fn(&@crate_ctxt cx, &span sp, ast::native_abi abi,
                     &ty::arg[] inputs, &ty::t output, uint ty_param_count)
   -> TypeRef {
    let TypeRef[] atys = ~[];
    if (abi == ast::native_abi_rust) {
        atys += ~[T_taskptr(*cx)];
        auto i = 0u;
        while (i < ty_param_count) {
            atys += ~[T_ptr(cx.tydesc_type)];
            i += 1u;
        }
    }
    atys += type_of_explicit_args(cx, sp, inputs);
    ret T_fn(atys, type_of_inner(cx, sp, output));
}

fn type_of_inner(&@crate_ctxt cx, &span sp, &ty::t t) -> TypeRef {
    // Check the cache.

    if (cx.lltypes.contains_key(t)) { ret cx.lltypes.get(t); }
    let TypeRef llty = 0 as TypeRef;
    alt (ty::struct(cx.tcx, t)) {
        case (ty::ty_native(_)) { llty = T_ptr(T_i8()); }
        case (ty::ty_nil) { llty = T_nil(); }
        case (ty::ty_bot) {
            llty = T_nil(); /* ...I guess? */

        }
        case (ty::ty_bool) { llty = T_bool(); }
        case (ty::ty_int) { llty = T_int(); }
        case (ty::ty_float) { llty = T_float(); }
        case (ty::ty_uint) { llty = T_int(); }
        case (ty::ty_machine(?tm)) {
            alt (tm) {
                case (ast::ty_i8) { llty = T_i8(); }
                case (ast::ty_u8) { llty = T_i8(); }
                case (ast::ty_i16) { llty = T_i16(); }
                case (ast::ty_u16) { llty = T_i16(); }
                case (ast::ty_i32) { llty = T_i32(); }
                case (ast::ty_u32) { llty = T_i32(); }
                case (ast::ty_i64) { llty = T_i64(); }
                case (ast::ty_u64) { llty = T_i64(); }
                case (ast::ty_f32) { llty = T_f32(); }
                case (ast::ty_f64) { llty = T_f64(); }
            }
        }
        case (ty::ty_char) { llty = T_char(); }
        case (ty::ty_str) { llty = T_ptr(T_str()); }
        case (ty::ty_istr) { llty = T_ivec(T_i8()); }
        case (ty::ty_tag(?did, _)) { llty = type_of_tag(cx, sp, did, t); }
        case (ty::ty_box(?mt)) {
            llty = T_ptr(T_box(type_of_inner(cx, sp, mt.ty)));
        }
        case (ty::ty_vec(?mt)) {
            llty = T_ptr(T_vec(type_of_inner(cx, sp, mt.ty)));
        }
        case (ty::ty_ivec(?mt)) {
            if (ty::type_has_dynamic_size(cx.tcx, mt.ty)) {
                llty = T_opaque_ivec();
            } else { llty = T_ivec(type_of_inner(cx, sp, mt.ty)); }
        }
        case (ty::ty_ptr(?mt)) { llty = T_ptr(type_of_inner(cx, sp, mt.ty)); }
        case (ty::ty_port(?t)) {
            llty = T_ptr(T_port(type_of_inner(cx, sp, t)));
        }
        case (ty::ty_chan(?t)) {
            llty = T_ptr(T_chan(type_of_inner(cx, sp, t)));
        }
        case (ty::ty_task) { llty = T_taskptr(*cx); }
        case (ty::ty_tup(?elts)) {
            let TypeRef[] tys = ~[];
            for (ty::mt elt in elts) {
                tys += ~[type_of_inner(cx, sp, elt.ty)];
            }
            llty = T_struct(tys);
        }
        case (ty::ty_rec(?fields)) {
            let TypeRef[] tys = ~[];
            for (ty::field f in fields) {
                tys += ~[type_of_inner(cx, sp, f.mt.ty)];
            }
            llty = T_struct(tys);
        }
        case (ty::ty_fn(?proto, ?args, ?out, _, _)) {
            llty = T_fn_pair(*cx, type_of_fn(cx, sp, proto, args, out, 0u));
        }
        case (ty::ty_native_fn(?abi, ?args, ?out)) {
            auto nft = native_fn_wrapper_type(cx, sp, 0u, t);
            llty = T_fn_pair(*cx, nft);
        }
        case (ty::ty_obj(?meths)) {
            llty = cx.rust_object_type;
        }
        case (ty::ty_res(_, ?sub, ?tps)) {
            auto sub1 = ty::substitute_type_params(cx.tcx, tps, sub);
            ret T_struct(~[T_i32(), type_of_inner(cx, sp, sub1)]);
        }
        case (ty::ty_var(_)) {
            cx.tcx.sess.span_fatal(sp, "trans::type_of called on ty_var");
        }
        case (ty::ty_param(_)) { llty = T_i8(); }
        case (ty::ty_type) { llty = T_ptr(cx.tydesc_type); }
    }
    assert (llty as int != 0);
    cx.lltypes.insert(t, llty);
    ret llty;
}

fn type_of_tag(&@crate_ctxt cx, &span sp, &ast::def_id did, &ty::t t)
    -> TypeRef {
    auto degen = std::ivec::len(ty::tag_variants(cx.tcx, did)) == 1u;
    if (ty::type_has_dynamic_size(cx.tcx, t)) {
        if (degen) { ret T_i8(); }
        else { ret T_opaque_tag(cx.tn); }
    } else {
        auto size = static_size_of_tag(cx, sp, t);
        if (!degen) { ret T_tag(cx.tn, size); }
        // LLVM does not like 0-size arrays, apparently
        if (size == 0u) { size = 1u; }
        ret T_array(T_i8(), size);
    }
}


fn type_of_arg(@local_ctxt cx, &span sp, &ty::arg arg) -> TypeRef {
    alt (ty::struct(cx.ccx.tcx, arg.ty)) {
        case (ty::ty_param(_)) {
            if (arg.mode != ty::mo_val) { ret T_typaram_ptr(cx.ccx.tn); }
        }
        case (_) {
            // fall through

        }
    }
    auto typ;
    if (arg.mode != ty::mo_val) {
        typ = T_ptr(type_of_inner(cx.ccx, sp, arg.ty));
    } else { typ = type_of_inner(cx.ccx, sp, arg.ty); }
    ret typ;
}

fn type_of_ty_param_count_and_ty(@local_ctxt lcx, &span sp,
                                 &ty::ty_param_count_and_ty tpt) -> TypeRef {
    alt (ty::struct(lcx.ccx.tcx, tpt._1)) {
        case (ty::ty_fn(?proto, ?inputs, ?output, _, _)) {
            auto llfnty =
                type_of_fn(lcx.ccx, sp, proto, inputs, output, tpt._0);
            ret T_fn_pair(*lcx.ccx, llfnty);
        }
        case (_) {
            // fall through

        }
    }
    ret type_of(lcx.ccx, sp, tpt._1);
}

fn type_of_or_i8(&@block_ctxt bcx, ty::t typ) -> TypeRef {
    if (ty::type_has_dynamic_size(bcx.fcx.lcx.ccx.tcx, typ)) { ret T_i8(); }
    ret type_of(bcx.fcx.lcx.ccx, bcx.sp, typ);
}


// Name sanitation. LLVM will happily accept identifiers with weird names, but
// gas doesn't!
fn sanitize(&str s) -> str {
    auto result = "";
    for (u8 c in s) {
        if (c == '@' as u8) {
            result += "boxed_";
        } else {
            if (c == ',' as u8) {
                result += "_";
            } else {
                if (c == '{' as u8 || c == '(' as u8) {
                    result += "_of_";
                } else {
                    if (c != 10u8 && c != '}' as u8 && c != ')' as u8 &&
                            c != ' ' as u8 && c != '\t' as u8 &&
                            c != ';' as u8) {
                        auto v = [c];
                        result += str::from_bytes(v);
                    }
                }
            }
        }
    }
    ret result;
}


fn log_fn_time(&@crate_ctxt ccx, str name, &time::timeval start,
               &time::timeval end) {
    auto elapsed = 1000 * ((end.sec - start.sec) as int) +
        ((end.usec as int) - (start.usec as int)) / 1000;
    *ccx.stats.fn_times += ~[tup(name, elapsed)];
}


fn decl_fn(ModuleRef llmod, &str name, uint cc, TypeRef llty) -> ValueRef {
    let ValueRef llfn = llvm::LLVMAddFunction(llmod, str::buf(name), llty);
    llvm::LLVMSetFunctionCallConv(llfn, cc);
    ret llfn;
}

fn decl_cdecl_fn(ModuleRef llmod, &str name, TypeRef llty) -> ValueRef {
    ret decl_fn(llmod, name, lib::llvm::LLVMCCallConv, llty);
}

fn decl_fastcall_fn(ModuleRef llmod, &str name, TypeRef llty) -> ValueRef {
    ret decl_fn(llmod, name, lib::llvm::LLVMFastCallConv, llty);
}


// Only use this if you are going to actually define the function. It's
// not valid to simply declare a function as internal.
fn decl_internal_fastcall_fn(ModuleRef llmod, &str name, TypeRef llty) ->
   ValueRef {
    auto llfn = decl_fn(llmod, name, lib::llvm::LLVMFastCallConv, llty);
    llvm::LLVMSetLinkage(llfn,
                         lib::llvm::LLVMInternalLinkage as llvm::Linkage);
    ret llfn;
}

fn decl_glue(ModuleRef llmod, &crate_ctxt cx, &str s) -> ValueRef {
    ret decl_cdecl_fn(llmod, s, T_fn(~[T_taskptr(cx)], T_void()));
}

fn get_extern_fn(&hashmap[str, ValueRef] externs, ModuleRef llmod, &str name,
                 uint cc, TypeRef ty) -> ValueRef {
    if (externs.contains_key(name)) { ret externs.get(name); }
    auto f = decl_fn(llmod, name, cc, ty);
    externs.insert(name, f);
    ret f;
}

fn get_extern_const(&hashmap[str, ValueRef] externs, ModuleRef llmod,
                    &str name, TypeRef ty) -> ValueRef {
    if (externs.contains_key(name)) { ret externs.get(name); }
    auto c = llvm::LLVMAddGlobal(llmod, ty, str::buf(name));
    externs.insert(name, c);
    ret c;
}

fn get_simple_extern_fn(&hashmap[str, ValueRef] externs, ModuleRef llmod,
                        &str name, int n_args) -> ValueRef {
    auto inputs = std::ivec::init_elt[TypeRef](T_int(), n_args as uint);
    auto output = T_int();
    auto t = T_fn(inputs, output);
    ret get_extern_fn(externs, llmod, name, lib::llvm::LLVMCCallConv, t);
}

fn trans_native_call(&builder b, @glue_fns glues, ValueRef lltaskptr,
                     &hashmap[str, ValueRef] externs, &type_names tn,
                     ModuleRef llmod, &str name, bool pass_task,
                     &ValueRef[] args) -> ValueRef {
    let int n = std::ivec::len[ValueRef](args) as int;
    let ValueRef llnative = get_simple_extern_fn(externs, llmod,
                                                 name, n);
    let ValueRef[] call_args = ~[];
    for (ValueRef a in args) { call_args += ~[b.ZExtOrBitCast(a, T_int())]; }
    ret b.Call(llnative, call_args);
}

fn trans_non_gc_free(&@block_ctxt cx, ValueRef v) -> result {
    cx.build.Call(cx.fcx.lcx.ccx.upcalls.free,
                  ~[cx.fcx.lltaskptr, cx.build.PointerCast(v, T_ptr(T_i8())),
                    C_int(0)]);
    ret rslt(cx, C_int(0));
}

fn trans_shared_free(&@block_ctxt cx, ValueRef v) -> result {
    cx.build.Call(cx.fcx.lcx.ccx.upcalls.shared_free,
                  ~[cx.fcx.lltaskptr,
                    cx.build.PointerCast(v, T_ptr(T_i8()))]);
    ret rslt(cx, C_int(0));
}

fn umax(&@block_ctxt cx, ValueRef a, ValueRef b) -> ValueRef {
    auto cond = cx.build.ICmp(lib::llvm::LLVMIntULT, a, b);
    ret cx.build.Select(cond, b, a);
}

fn umin(&@block_ctxt cx, ValueRef a, ValueRef b) -> ValueRef {
    auto cond = cx.build.ICmp(lib::llvm::LLVMIntULT, a, b);
    ret cx.build.Select(cond, a, b);
}

fn align_to(&@block_ctxt cx, ValueRef off, ValueRef align) -> ValueRef {
    auto mask = cx.build.Sub(align, C_int(1));
    auto bumped = cx.build.Add(off, mask);
    ret cx.build.And(bumped, cx.build.Not(mask));
}


// Returns the real size of the given type for the current target.
fn llsize_of_real(&@crate_ctxt cx, TypeRef t) -> uint {
    ret llvm::LLVMStoreSizeOfType(cx.td.lltd, t);
}

fn llsize_of(TypeRef t) -> ValueRef {
    ret llvm::LLVMConstIntCast(lib::llvm::llvm::LLVMSizeOf(t), T_int(),
                               False);
}

fn llalign_of(TypeRef t) -> ValueRef {
    ret llvm::LLVMConstIntCast(lib::llvm::llvm::LLVMAlignOf(t), T_int(),
                               False);
}

fn size_of(&@block_ctxt cx, &ty::t t) -> result {
    if (!ty::type_has_dynamic_size(cx.fcx.lcx.ccx.tcx, t)) {
        ret rslt(cx, llsize_of(type_of(cx.fcx.lcx.ccx, cx.sp, t)));
    }
    ret dynamic_size_of(cx, t);
}

fn align_of(&@block_ctxt cx, &ty::t t) -> result {
    if (!ty::type_has_dynamic_size(cx.fcx.lcx.ccx.tcx, t)) {
        ret rslt(cx, llalign_of(type_of(cx.fcx.lcx.ccx, cx.sp, t)));
    }
    ret dynamic_align_of(cx, t);
}

fn alloca(&@block_ctxt cx, TypeRef t) -> ValueRef {
    ret new_builder(cx.fcx.llstaticallocas).Alloca(t);
}

fn array_alloca(&@block_ctxt cx, TypeRef t, ValueRef n) -> ValueRef {
    ret new_builder(cx.fcx.lldynamicallocas).ArrayAlloca(t, n);
}


// Creates a simpler, size-equivalent type. The resulting type is guaranteed
// to have (a) the same size as the type that was passed in; (b) to be non-
// recursive. This is done by replacing all boxes in a type with boxed unit
// types.
fn simplify_type(&@crate_ctxt ccx, &ty::t typ) -> ty::t {
    fn simplifier(@crate_ctxt ccx, ty::t typ) -> ty::t {
        alt (ty::struct(ccx.tcx, typ)) {
            case (ty::ty_box(_)) {
                ret ty::mk_imm_box(ccx.tcx, ty::mk_nil(ccx.tcx));
            }
            case (ty::ty_vec(_)) {
                ret ty::mk_imm_vec(ccx.tcx, ty::mk_nil(ccx.tcx));
            }
            case (ty::ty_fn(_, _, _, _, _)) {
                ret ty::mk_imm_tup(ccx.tcx,
                                   ~[ty::mk_imm_box(ccx.tcx,
                                                    ty::mk_nil(ccx.tcx)),
                                     ty::mk_imm_box(ccx.tcx,
                                                    ty::mk_nil(ccx.tcx))]);
            }
            case (ty::ty_obj(_)) {
                ret ty::mk_imm_tup(ccx.tcx,
                                   ~[ty::mk_imm_box(ccx.tcx,
                                                    ty::mk_nil(ccx.tcx)),
                                     ty::mk_imm_box(ccx.tcx,
                                                    ty::mk_nil(ccx.tcx))]);
            }
            case (ty::ty_res(_, ?sub, ?tps)) {
                auto sub1 = ty::substitute_type_params(ccx.tcx, tps, sub);
                ret ty::mk_imm_tup(ccx.tcx, ~[ty::mk_int(ccx.tcx),
                                              simplify_type(ccx, sub1)]);
            }
            case (_) { ret typ; }
        }
    }
    ret ty::fold_ty(ccx.tcx, ty::fm_general(bind simplifier(ccx, _)), typ);
}


// Computes the size of the data part of a non-dynamically-sized tag.
fn static_size_of_tag(&@crate_ctxt cx, &span sp, &ty::t t) -> uint {
    if (ty::type_has_dynamic_size(cx.tcx, t)) {
        cx.tcx.sess.span_fatal(sp,
                             "dynamically sized type passed to " +
                                 "static_size_of_tag()");
    }
    if (cx.tag_sizes.contains_key(t)) { ret cx.tag_sizes.get(t); }
    alt (ty::struct(cx.tcx, t)) {
        case (ty::ty_tag(?tid, ?subtys)) {
            // Compute max(variant sizes).

            auto max_size = 0u;
            auto variants = ty::tag_variants(cx.tcx, tid);
            for (ty::variant_info variant in variants) {
                auto tup_ty = simplify_type(cx, ty::mk_imm_tup(cx.tcx,
                                                               variant.args));
                // Perform any type parameter substitutions.

                tup_ty = ty::substitute_type_params(cx.tcx, subtys, tup_ty);
                // Here we possibly do a recursive call.

                auto this_size = llsize_of_real(cx, type_of(cx, sp, tup_ty));
                if (max_size < this_size) { max_size = this_size; }
            }
            cx.tag_sizes.insert(t, max_size);
            ret max_size;
        }
        case (_) {
            cx.tcx.sess.span_fatal(sp,
                                 "non-tag passed to " +
                                     "static_size_of_tag()");
        }
    }
}

fn dynamic_size_of(&@block_ctxt cx, ty::t t) -> result {
    fn align_elements(&@block_ctxt cx, &ty::t[] elts) -> result {
        //
        // C padding rules:
        //
        //
        //   - Pad after each element so that next element is aligned.
        //   - Pad after final structure member so that whole structure
        //     is aligned to max alignment of interior.
        //

        auto off = C_int(0);
        auto max_align = C_int(1);
        auto bcx = cx;
        for (ty::t e in elts) {
            auto elt_align = align_of(bcx, e);
            bcx = elt_align.bcx;
            auto elt_size = size_of(bcx, e);
            bcx = elt_size.bcx;
            auto aligned_off = align_to(bcx, off, elt_align.val);
            off = bcx.build.Add(aligned_off, elt_size.val);
            max_align = umax(bcx, max_align, elt_align.val);
        }
        off = align_to(bcx, off, max_align);
        ret rslt(bcx, off);
    }
    alt (ty::struct(cx.fcx.lcx.ccx.tcx, t)) {
        case (ty::ty_param(?p)) {
            auto szptr =
                field_of_tydesc(cx, t, false, abi::tydesc_field_size);
            ret rslt(szptr.bcx, szptr.bcx.build.Load(szptr.val));
        }
        case (ty::ty_tup(?elts)) {
            let ty::t[] tys = ~[];
            for (ty::mt mt in elts) { tys += ~[mt.ty]; }
            ret align_elements(cx, tys);
        }
        case (ty::ty_rec(?flds)) {
            let ty::t[] tys = ~[];
            for (ty::field f in flds) { tys += ~[f.mt.ty]; }
            ret align_elements(cx, tys);
        }
        case (ty::ty_tag(?tid, ?tps)) {
            auto bcx = cx;
            // Compute max(variant sizes).

            let ValueRef max_size = alloca(bcx, T_int());
            bcx.build.Store(C_int(0), max_size);
            auto variants = ty::tag_variants(bcx.fcx.lcx.ccx.tcx, tid);
            for (ty::variant_info variant in variants) {
                // Perform type substitution on the raw argument types.

                let ty::t[] raw_tys = variant.args;
                let ty::t[] tys = ~[];
                for (ty::t raw_ty in raw_tys) {
                    auto t = ty::substitute_type_params(cx.fcx.lcx.ccx.tcx,
                                                        tps, raw_ty);
                    tys += ~[t];
                }
                auto rslt = align_elements(bcx, tys);
                bcx = rslt.bcx;
                auto this_size = rslt.val;
                auto old_max_size = bcx.build.Load(max_size);
                bcx.build.Store(umax(bcx, this_size, old_max_size), max_size);
            }
            auto max_size_val = bcx.build.Load(max_size);
            auto total_size = if (std::ivec::len(variants) != 1u) {
                bcx.build.Add(max_size_val, llsize_of(T_int()))
            } else { max_size_val };
            ret rslt(bcx, total_size);
        }
        case (ty::ty_ivec(?mt)) {
            auto rs = size_of(cx, mt.ty);
            auto bcx = rs.bcx;
            auto llunitsz = rs.val;
            auto llsz = bcx.build.Add(llsize_of(T_opaque_ivec()),
                bcx.build.Mul(llunitsz, C_uint(abi::ivec_default_length)));
            ret rslt(bcx, llsz);
        }
    }
}

fn dynamic_align_of(&@block_ctxt cx, &ty::t t) -> result {
    alt (ty::struct(cx.fcx.lcx.ccx.tcx, t)) {
        case (ty::ty_param(?p)) {
            auto aptr =
                field_of_tydesc(cx, t, false, abi::tydesc_field_align);
            ret rslt(aptr.bcx, aptr.bcx.build.Load(aptr.val));
        }
        case (ty::ty_tup(?elts)) {
            auto a = C_int(1);
            auto bcx = cx;
            for (ty::mt e in elts) {
                auto align = align_of(bcx, e.ty);
                bcx = align.bcx;
                a = umax(bcx, a, align.val);
            }
            ret rslt(bcx, a);
        }
        case (ty::ty_rec(?flds)) {
            auto a = C_int(1);
            auto bcx = cx;
            for (ty::field f in flds) {
                auto align = align_of(bcx, f.mt.ty);
                bcx = align.bcx;
                a = umax(bcx, a, align.val);
            }
            ret rslt(bcx, a);
        }
        case (ty::ty_tag(_, _)) {
            ret rslt(cx, C_int(1)); // FIXME: stub
        }
        case (ty::ty_ivec(?tm)) {
            auto rs = align_of(cx, tm.ty);
            auto bcx = rs.bcx;
            auto llunitalign = rs.val;
            auto llalign = umax(bcx, llalign_of(T_int()), llunitalign);
            ret rslt(bcx, llalign);
        }
    }
}


// Replacement for the LLVM 'GEP' instruction when field-indexing into a
// tuple-like structure (tup, rec) with a static index. This one is driven off
// ty::struct and knows what to do when it runs into a ty_param stuck in the
// middle of the thing it's GEP'ing into. Much like size_of and align_of,
// above.
fn GEP_tup_like(&@block_ctxt cx, &ty::t t, ValueRef base, &int[] ixs)
        -> result {
    assert (ty::type_is_tup_like(cx.fcx.lcx.ccx.tcx, t));
    // It might be a static-known type. Handle this.

    if (!ty::type_has_dynamic_size(cx.fcx.lcx.ccx.tcx, t)) {
        let ValueRef[] v = ~[];
        for (int i in ixs) { v += ~[C_int(i)]; }
        ret rslt(cx, cx.build.GEP(base, v));
    }
    // It is a dynamic-containing type that, if we convert directly to an LLVM
    // TypeRef, will be all wrong; there's no proper LLVM type to represent
    // it, and the lowering function will stick in i8* values for each
    // ty_param, which is not right; the ty_params are all of some dynamic
    // size.
    //
    // What we must do instead is sadder. We must look through the indices
    // manually and split the input type into a prefix and a target. We then
    // measure the prefix size, bump the input pointer by that amount, and
    // cast to a pointer-to-target type.

    // Given a type, an index vector and an element number N in that vector,
    // calculate index X and the type that results by taking the first X-1
    // elements of the type and splitting the Xth off. Return the prefix as
    // well as the innermost Xth type.

    fn split_type(&@crate_ctxt ccx, &ty::t t, &int[] ixs, uint n)
            -> rec(ty::t[] prefix, ty::t target) {
        let uint len = std::ivec::len[int](ixs);
        // We don't support 0-index or 1-index GEPs: The former is nonsense
        // and the latter would only be meaningful if we supported non-0
        // values for the 0th index (we don't).

        assert (len > 1u);
        if (n == 0u) {
            // Since we're starting from a value that's a pointer to a
            // *single* structure, the first index (in GEP-ese) should just be
            // 0, to yield the pointee.

            assert (ixs.(n) == 0);
            ret split_type(ccx, t, ixs, n + 1u);
        }
        assert (n < len);
        let int ix = ixs.(n);
        let ty::t[] prefix = ~[];
        let int i = 0;
        while (i < ix) {
            prefix += ~[ty::get_element_type(ccx.tcx, t, i as uint)];
            i += 1;
        }
        auto selected = ty::get_element_type(ccx.tcx, t, i as uint);
        if (n == len - 1u) {
            // We are at the innermost index.

            ret rec(prefix=prefix, target=selected);
        } else {
            // Not the innermost index; call self recursively to dig deeper.
            // Once we get an inner result, append it current prefix and
            // return to caller.

            auto inner = split_type(ccx, selected, ixs, n + 1u);
            prefix += inner.prefix;
            ret rec(prefix=prefix with inner);
        }
    }
    // We make a fake prefix tuple-type here; luckily for measuring sizes
    // the tuple parens are associative so it doesn't matter that we've
    // flattened the incoming structure.

    auto s = split_type(cx.fcx.lcx.ccx, t, ixs, 0u);

    auto args = ~[];
    for (ty::t typ in s.prefix) { args += ~[typ]; }
    auto prefix_ty = ty::mk_imm_tup(cx.fcx.lcx.ccx.tcx, args);

    auto bcx = cx;
    auto sz = size_of(bcx, prefix_ty);
    bcx = sz.bcx;
    auto raw = bcx.build.PointerCast(base, T_ptr(T_i8()));
    auto bumped = bcx.build.GEP(raw, ~[sz.val]);
    if (ty::type_has_dynamic_size(cx.fcx.lcx.ccx.tcx, s.target)) {
        ret rslt(bcx, bumped);
    }
    auto typ = T_ptr(type_of(bcx.fcx.lcx.ccx, bcx.sp, s.target));
    ret rslt(bcx, bcx.build.PointerCast(bumped, typ));
}


// Replacement for the LLVM 'GEP' instruction when field indexing into a tag.
// This function uses GEP_tup_like() above and automatically performs casts as
// appropriate. @llblobptr is the data part of a tag value; its actual type is
// meaningless, as it will be cast away.
fn GEP_tag(@block_ctxt cx, ValueRef llblobptr, &ast::def_id tag_id,
           &ast::def_id variant_id, &ty::t[] ty_substs, int ix) -> result {
    auto variant =
        ty::tag_variant_with_id(cx.fcx.lcx.ccx.tcx, tag_id, variant_id);
    // Synthesize a tuple type so that GEP_tup_like() can work its magic.
    // Separately, store the type of the element we're interested in.

    auto arg_tys = variant.args;
    auto elem_ty = ty::mk_nil(cx.fcx.lcx.ccx.tcx); // typestate infelicity

    auto i = 0;
    let ty::t[] true_arg_tys = ~[];
    for (ty::t aty in arg_tys) {
        auto arg_ty =
            ty::substitute_type_params(cx.fcx.lcx.ccx.tcx, ty_substs, aty);
        true_arg_tys += ~[arg_ty];
        if (i == ix) { elem_ty = arg_ty; }
        i += 1;
    }
    auto tup_ty = ty::mk_imm_tup(cx.fcx.lcx.ccx.tcx, true_arg_tys);
    // Cast the blob pointer to the appropriate type, if we need to (i.e. if
    // the blob pointer isn't dynamically sized).

    let ValueRef llunionptr;
    if (!ty::type_has_dynamic_size(cx.fcx.lcx.ccx.tcx, tup_ty)) {
        auto llty = type_of(cx.fcx.lcx.ccx, cx.sp, tup_ty);
        llunionptr = cx.build.TruncOrBitCast(llblobptr, T_ptr(llty));
    } else { llunionptr = llblobptr; }
    // Do the GEP_tup_like().

    auto rs = GEP_tup_like(cx, tup_ty, llunionptr, ~[0, ix]);
    // Cast the result to the appropriate type, if necessary.

    auto val;
    if (!ty::type_has_dynamic_size(cx.fcx.lcx.ccx.tcx, elem_ty)) {
        auto llelemty = type_of(rs.bcx.fcx.lcx.ccx, cx.sp, elem_ty);
        val = rs.bcx.build.PointerCast(rs.val, T_ptr(llelemty));
    } else { val = rs.val; }
    ret rslt(rs.bcx, val);
}


// trans_raw_malloc: expects a type indicating which pointer type we want and
// a size indicating how much space we want malloc'd.
fn trans_raw_malloc(&@block_ctxt cx, TypeRef llptr_ty, ValueRef llsize) ->
   result {
    // FIXME: need a table to collect tydesc globals.

    auto tydesc = C_null(T_ptr(cx.fcx.lcx.ccx.tydesc_type));
    auto rval =
        cx.build.Call(cx.fcx.lcx.ccx.upcalls.malloc,
                      ~[cx.fcx.lltaskptr, llsize, tydesc]);
    ret rslt(cx, cx.build.PointerCast(rval, llptr_ty));
}

// trans_shared_malloc: expects a type indicating which pointer type we want
// and a size indicating how much space we want malloc'd.
fn trans_shared_malloc(&@block_ctxt cx, TypeRef llptr_ty, ValueRef llsize) ->
   result {
    // FIXME: need a table to collect tydesc globals.

    auto tydesc = C_null(T_ptr(cx.fcx.lcx.ccx.tydesc_type));
    auto rval =
        cx.build.Call(cx.fcx.lcx.ccx.upcalls.shared_malloc,
                      ~[cx.fcx.lltaskptr, llsize, tydesc]);
    ret rslt(cx, cx.build.PointerCast(rval, llptr_ty));
}

// trans_malloc_boxed: expects an unboxed type and returns a pointer to enough
// space for something of that type, along with space for a reference count;
// in other words, it allocates a box for something of that type.
fn trans_malloc_boxed(&@block_ctxt cx, ty::t t) -> result {
    // Synthesize a fake box type structurally so we have something
    // to measure the size of.

    // We synthesize two types here because we want both the type of the
    // pointer and the pointee.  boxed_body is the type that we measure the
    // size of; box_ptr is the type that's converted to a TypeRef and used as
    // the pointer cast target in trans_raw_malloc.

    auto boxed_body =
        ty::mk_imm_tup(cx.fcx.lcx.ccx.tcx,
                       // The mk_int here is the space being
                       // reserved for the refcount.
                       ~[ty::mk_int(cx.fcx.lcx.ccx.tcx), t]);
    auto box_ptr = ty::mk_imm_box(cx.fcx.lcx.ccx.tcx, t);
    auto sz = size_of(cx, boxed_body);
    // Grab the TypeRef type of box_ptr, because that's what trans_raw_malloc
    // wants.

    auto llty = type_of(cx.fcx.lcx.ccx, cx.sp, box_ptr);
    ret trans_raw_malloc(sz.bcx, llty, sz.val);
}


// Type descriptor and type glue stuff

// Given a type and a field index into its corresponding type descriptor,
// returns an LLVM ValueRef of that field from the tydesc, generating the
// tydesc if necessary.
fn field_of_tydesc(&@block_ctxt cx, &ty::t t, bool escapes, int field) ->
   result {
    auto ti = none[@tydesc_info];
    auto tydesc = get_tydesc(cx, t, escapes, ti);
    ret rslt(tydesc.bcx,
             tydesc.bcx.build.GEP(tydesc.val, ~[C_int(0), C_int(field)]));
}


// Given a type containing ty params, build a vector containing a ValueRef for
// each of the ty params it uses (from the current frame) and a vector of the
// indices of the ty params present in the type. This is used solely for
// constructing derived tydescs.
fn linearize_ty_params(&@block_ctxt cx, &ty::t t) -> tup(uint[], ValueRef[]) {
    let ValueRef[] param_vals = ~[];
    let uint[] param_defs = ~[];
    type rr = rec(@block_ctxt cx,
                  mutable ValueRef[] vals,
                  mutable uint[] defs);

    fn linearizer(@rr r, ty::t t) {
        alt (ty::struct(r.cx.fcx.lcx.ccx.tcx, t)) {
            case (ty::ty_param(?pid)) {
                let bool seen = false;
                for (uint d in r.defs) { if (d == pid) { seen = true; } }
                if (!seen) {
                    r.vals += ~[r.cx.fcx.lltydescs.(pid)];
                    r.defs += ~[pid];
                }
            }
            case (_) { }
        }
    }
    auto x = @rec(cx=cx, mutable vals=param_vals, mutable defs=param_defs);
    auto f = bind linearizer(x, _);
    ty::walk_ty(cx.fcx.lcx.ccx.tcx, f, t);
    ret tup(x.defs, x.vals);
}

fn trans_stack_local_derived_tydesc(&@block_ctxt cx, ValueRef llsz,
                                    ValueRef llalign, ValueRef llroottydesc,
                                    ValueRef llparamtydescs) -> ValueRef {
    auto llmyroottydesc = alloca(cx, cx.fcx.lcx.ccx.tydesc_type);
    // By convention, desc 0 is the root descriptor.

    llroottydesc = cx.build.Load(llroottydesc);
    cx.build.Store(llroottydesc, llmyroottydesc);
    // Store a pointer to the rest of the descriptors.

    auto llfirstparam = cx.build.GEP(llparamtydescs, ~[C_int(0), C_int(0)]);
    cx.build.Store(llfirstparam,
                   cx.build.GEP(llmyroottydesc, ~[C_int(0), C_int(0)]));
    cx.build.Store(llsz, cx.build.GEP(llmyroottydesc, ~[C_int(0), C_int(1)]));
    cx.build.Store(llalign,
                   cx.build.GEP(llmyroottydesc, ~[C_int(0), C_int(2)]));
    ret llmyroottydesc;
}

fn get_derived_tydesc(&@block_ctxt cx, &ty::t t, bool escapes,
                      &mutable option::t[@tydesc_info] static_ti) -> result {
    alt (cx.fcx.derived_tydescs.find(t)) {
        case (some(?info)) {

            // If the tydesc escapes in this context, the cached derived
            // tydesc also has to be one that was marked as escaping.
            if (!(escapes && !info.escapes)) { ret rslt(cx, info.lltydesc); }
        }
        case (none) {/* fall through */ }
    }
    cx.fcx.lcx.ccx.stats.n_derived_tydescs += 1u;
    auto bcx = new_raw_block_ctxt(cx.fcx, cx.fcx.llderivedtydescs);
    let uint n_params = ty::count_ty_params(bcx.fcx.lcx.ccx.tcx, t);
    auto tys = linearize_ty_params(bcx, t);
    assert (n_params == std::ivec::len[uint](tys._0));
    assert (n_params == std::ivec::len[ValueRef](tys._1));
    auto root_ti = get_static_tydesc(bcx, t, tys._0);
    static_ti = some[@tydesc_info](root_ti);
    lazily_emit_all_tydesc_glue(cx, static_ti);
    auto root = root_ti.tydesc;
    auto sz = size_of(bcx, t);
    bcx = sz.bcx;
    auto align = align_of(bcx, t);
    bcx = align.bcx;
    auto v;
    if (escapes) {
        auto tydescs =
            alloca(bcx, /* for root*/

                   T_array(T_ptr(bcx.fcx.lcx.ccx.tydesc_type),
                           1u + n_params));
        auto i = 0;
        auto tdp = bcx.build.GEP(tydescs, ~[C_int(0), C_int(i)]);
        bcx.build.Store(root, tdp);
        i += 1;
        for (ValueRef td in tys._1) {
            auto tdp = bcx.build.GEP(tydescs, ~[C_int(0), C_int(i)]);
            bcx.build.Store(td, tdp);
            i += 1;
        }
        auto lltydescsptr =
            bcx.build.PointerCast(tydescs,
                                  T_ptr(T_ptr(bcx.fcx.lcx.ccx.tydesc_type)));
        auto td_val =
            bcx.build.Call(bcx.fcx.lcx.ccx.upcalls.get_type_desc,
                           ~[bcx.fcx.lltaskptr, C_null(T_ptr(T_nil())),
                             sz.val, align.val, C_int(1u + n_params as int),
                             lltydescsptr]);
        v = td_val;
    } else {
        auto llparamtydescs =
            alloca(bcx,
                   T_array(T_ptr(bcx.fcx.lcx.ccx.tydesc_type), n_params));
        auto i = 0;
        for (ValueRef td in tys._1) {
            auto tdp = bcx.build.GEP(llparamtydescs, ~[C_int(0), C_int(i)]);
            bcx.build.Store(td, tdp);
            i += 1;
        }
        v =
            trans_stack_local_derived_tydesc(bcx, sz.val, align.val, root,
                                             llparamtydescs);
    }
    bcx.fcx.derived_tydescs.insert(t, rec(lltydesc=v, escapes=escapes));
    ret rslt(cx, v);
}

fn get_tydesc(&@block_ctxt cx, &ty::t orig_t, bool escapes,
              &mutable option::t[@tydesc_info] static_ti) -> result {

    auto t = ty::strip_cname(cx.fcx.lcx.ccx.tcx, orig_t);

    // Is the supplied type a type param? If so, return the passed-in tydesc.
    alt (ty::type_param(cx.fcx.lcx.ccx.tcx, t)) {
        case (some(?id)) { ret rslt(cx, cx.fcx.lltydescs.(id)); }
        case (none) {/* fall through */ }
    }

    // Does it contain a type param? If so, generate a derived tydesc.
    if (ty::type_contains_params(cx.fcx.lcx.ccx.tcx, t)) {
        ret get_derived_tydesc(cx, t, escapes, static_ti);
    }

    // Otherwise, generate a tydesc if necessary, and return it.
    auto info = get_static_tydesc(cx, t, ~[]);
    static_ti = some[@tydesc_info](info);
    ret rslt(cx, info.tydesc);
}

fn get_static_tydesc(&@block_ctxt cx, &ty::t orig_t, &uint[] ty_params)
        -> @tydesc_info {
    auto t = ty::strip_cname(cx.fcx.lcx.ccx.tcx, orig_t);

    alt (cx.fcx.lcx.ccx.tydescs.find(t)) {
        case (some(?info)) { ret info; }
        case (none) {
            cx.fcx.lcx.ccx.stats.n_static_tydescs += 1u;
            auto info = declare_tydesc(cx.fcx.lcx, cx.sp, t, ty_params);
            cx.fcx.lcx.ccx.tydescs.insert(t, info);
            ret info;
        }
    }
}

fn set_no_inline(ValueRef f) {
    llvm::LLVMAddFunctionAttr(f,
                              lib::llvm::LLVMNoInlineAttribute as
                                  lib::llvm::llvm::Attribute);
}

// Tell LLVM to emit the information necessary to unwind the stack for the
// function f.
fn set_uwtable(ValueRef f) {
    llvm::LLVMAddFunctionAttr(f,
                              lib::llvm::LLVMUWTableAttribute as
                                  lib::llvm::llvm::Attribute);
}

fn set_always_inline(ValueRef f) {
    llvm::LLVMAddFunctionAttr(f,
                              lib::llvm::LLVMAlwaysInlineAttribute as
                                  lib::llvm::llvm::Attribute);
}

fn set_glue_inlining(&@local_ctxt cx, ValueRef f, &ty::t t) {
    if (ty::type_is_structural(cx.ccx.tcx, t)) {
        set_no_inline(f);
    } else { set_always_inline(f); }
}


// Generates the declaration for (but doesn't emit) a type descriptor.
fn declare_tydesc(&@local_ctxt cx, &span sp, &ty::t t, &uint[] ty_params)
        -> @tydesc_info {
    log "+++ declare_tydesc " + ty_to_str(cx.ccx.tcx, t);
    auto ccx = cx.ccx;
    auto llsize;
    auto llalign;
    if (!ty::type_has_dynamic_size(ccx.tcx, t)) {
        auto llty = type_of(ccx, sp, t);
        llsize = llsize_of(llty);
        llalign = llalign_of(llty);
    } else {
        // These will be overwritten as the derived tydesc is generated, so
        // we create placeholder values.

        llsize = C_int(0);
        llalign = C_int(0);
    }
    auto name;
    if (cx.ccx.sess.get_opts().debuginfo) {
        name = mangle_internal_name_by_type_only(cx.ccx, t, "tydesc");
        name = sanitize(name);
    } else { name = mangle_internal_name_by_seq(cx.ccx, "tydesc"); }
    auto gvar =
        llvm::LLVMAddGlobal(ccx.llmod, ccx.tydesc_type, str::buf(name));
    auto info =
        @rec(ty=t,
             tydesc=gvar,
             size=llsize,
             align=llalign,
             mutable copy_glue=none[ValueRef],
             mutable drop_glue=none[ValueRef],
             mutable free_glue=none[ValueRef],
             mutable cmp_glue=none[ValueRef],
             ty_params=ty_params);
    log "--- declare_tydesc " + ty_to_str(cx.ccx.tcx, t);
    ret info;
}

tag make_generic_glue_helper_fn {
    mgghf_single(fn(&@block_ctxt, ValueRef, &ty::t) );
    mgghf_cmp;
}

fn declare_generic_glue(&@local_ctxt cx, &ty::t t, TypeRef llfnty, &str name)
   -> ValueRef {
    auto fn_nm;
    if (cx.ccx.sess.get_opts().debuginfo) {
        fn_nm = mangle_internal_name_by_type_only(cx.ccx, t, "glue_" + name);
        fn_nm = sanitize(fn_nm);
    } else { fn_nm = mangle_internal_name_by_seq(cx.ccx, "glue_" + name); }
    auto llfn = decl_cdecl_fn(cx.ccx.llmod, fn_nm, llfnty);
    set_glue_inlining(cx, llfn, t);
    ret llfn;
}

fn make_generic_glue_inner(&@local_ctxt cx, &span sp, &ty::t t, ValueRef llfn,
                           &make_generic_glue_helper_fn helper,
                           &uint[] ty_params) -> ValueRef {
    auto fcx = new_fn_ctxt(cx, sp, llfn);
    llvm::LLVMSetLinkage(llfn,
                         lib::llvm::LLVMInternalLinkage as llvm::Linkage);
    cx.ccx.stats.n_glues_created += 1u;
    // Any nontrivial glue is with values passed *by alias*; this is a
    // requirement since in many contexts glue is invoked indirectly and
    // the caller has no idea if it's dealing with something that can be
    // passed by value.

    auto llty;
    if (ty::type_has_dynamic_size(cx.ccx.tcx, t)) {
        llty = T_ptr(T_i8());
    } else { llty = T_ptr(type_of(cx.ccx, sp, t)); }
    auto ty_param_count = std::ivec::len[uint](ty_params);
    auto lltyparams = llvm::LLVMGetParam(llfn, 3u);
    auto copy_args_bcx = new_raw_block_ctxt(fcx, fcx.llcopyargs);
    auto lltydescs = ~[mutable];
    auto p = 0u;
    while (p < ty_param_count) {
        auto llparam = copy_args_bcx.build.GEP(lltyparams,
                                               ~[C_int(p as int)]);
        llparam = copy_args_bcx.build.Load(llparam);
        std::ivec::grow_set(lltydescs, ty_params.(p), 0 as ValueRef, llparam);
        p += 1u;
    }

    // TODO: Implement some kind of freeze operation in the standard library.
    auto lltydescs_frozen = ~[];
    for (ValueRef lltydesc in lltydescs) { lltydescs_frozen += ~[lltydesc]; }
    fcx.lltydescs = lltydescs_frozen;

    auto bcx = new_top_block_ctxt(fcx);
    auto lltop = bcx.llbb;
    auto llrawptr0 = llvm::LLVMGetParam(llfn, 4u);
    auto llval0 = bcx.build.BitCast(llrawptr0, llty);
    alt (helper) {
        case (mgghf_single(?single_fn)) { single_fn(bcx, llval0, t); }
        case (mgghf_cmp) {
            auto llrawptr1 = llvm::LLVMGetParam(llfn, 5u);
            auto llval1 = bcx.build.BitCast(llrawptr1, llty);
            auto llcmpval = llvm::LLVMGetParam(llfn, 6u);
            make_cmp_glue(bcx, llval0, llval1, t, llcmpval);
        }
    }
    finish_fn(fcx, lltop);
    ret llfn;
}

fn make_generic_glue(&@local_ctxt cx, &span sp, &ty::t t, ValueRef llfn,
                     &make_generic_glue_helper_fn helper,
                     &uint[] ty_params, &str name) -> ValueRef {
    if !cx.ccx.sess.get_opts().stats {
        ret make_generic_glue_inner(cx, sp, t, llfn, helper, ty_params);
    }

    auto start = time::get_time();
    auto llval = make_generic_glue_inner(cx, sp, t, llfn, helper, ty_params);
    auto end = time::get_time();
    log_fn_time(cx.ccx, "glue " + name + " " + ty_to_short_str(cx.ccx.tcx, t),
                start, end);
    ret llval;
}

fn emit_tydescs(&@crate_ctxt ccx) {
    for each (@tup(ty::t, @tydesc_info) pair in ccx.tydescs.items()) {
        auto glue_fn_ty = T_ptr(T_glue_fn(*ccx));
        auto cmp_fn_ty = T_ptr(T_cmp_glue_fn(*ccx));
        auto ti = pair._1;
        auto copy_glue =
            alt ({ ti.copy_glue }) {
                case (none) {
                    ccx.stats.n_null_glues += 1u;
                    C_null(glue_fn_ty)
                }
                case (some(?v)) { ccx.stats.n_real_glues += 1u; v }
            };
        auto drop_glue =
            alt ({ ti.drop_glue }) {
                case (none) {
                    ccx.stats.n_null_glues += 1u;
                    C_null(glue_fn_ty)
                }
                case (some(?v)) { ccx.stats.n_real_glues += 1u; v }
            };
        auto free_glue =
            alt ({ ti.free_glue }) {
                case (none) {
                    ccx.stats.n_null_glues += 1u;
                    C_null(glue_fn_ty)
                }
                case (some(?v)) { ccx.stats.n_real_glues += 1u; v }
            };
        auto cmp_glue =
            alt ({ ti.cmp_glue }) {
                case (none) {
                    ccx.stats.n_null_glues += 1u;
                    C_null(cmp_fn_ty)
                }
                case (some(?v)) { ccx.stats.n_real_glues += 1u; v }
            };
        auto tydesc =
            C_named_struct(ccx.tydesc_type,
                     ~[C_null(T_ptr(T_ptr(ccx.tydesc_type))), ti.size,
                       ti.align, copy_glue, // copy_glue
                       drop_glue, // drop_glue
                       free_glue, // free_glue
                       C_null(glue_fn_ty), // sever_glue
                       C_null(glue_fn_ty), // mark_glue
                       C_null(glue_fn_ty), // obj_drop_glue
                       C_null(glue_fn_ty), // is_stateful
                       cmp_glue]); // cmp_glue

        auto gvar = ti.tydesc;
        llvm::LLVMSetInitializer(gvar, tydesc);
        llvm::LLVMSetGlobalConstant(gvar, True);
        llvm::LLVMSetLinkage(gvar,
                             lib::llvm::LLVMInternalLinkage as llvm::Linkage);
    }
}

fn make_copy_glue(&@block_ctxt cx, ValueRef v, &ty::t t) {
    // NB: v is an *alias* of type t here, not a direct value.

    auto bcx;
    if (ty::type_is_boxed(cx.fcx.lcx.ccx.tcx, t)) {
        bcx = incr_refcnt_of_boxed(cx, cx.build.Load(v)).bcx;
    } else if (ty::type_is_structural(cx.fcx.lcx.ccx.tcx, t)) {
        bcx = duplicate_heap_parts_if_necessary(cx, v, t).bcx;
        bcx = iter_structural_ty(bcx, v, t, bind copy_ty(_, _, _)).bcx;
    } else { bcx = cx; }
    bcx.build.RetVoid();
}

fn incr_refcnt_of_boxed(&@block_ctxt cx, ValueRef box_ptr) -> result {
    auto rc_ptr =
        cx.build.GEP(box_ptr, ~[C_int(0), C_int(abi::box_rc_field_refcnt)]);
    auto rc = cx.build.Load(rc_ptr);
    auto rc_adj_cx = new_sub_block_ctxt(cx, "rc++");
    auto next_cx = new_sub_block_ctxt(cx, "next");
    auto const_test =
        cx.build.ICmp(lib::llvm::LLVMIntEQ, C_int(abi::const_refcount as int),
                      rc);
    cx.build.CondBr(const_test, next_cx.llbb, rc_adj_cx.llbb);
    rc = rc_adj_cx.build.Add(rc, C_int(1));
    rc_adj_cx.build.Store(rc, rc_ptr);
    rc_adj_cx.build.Br(next_cx.llbb);
    ret rslt(next_cx, C_nil());
}

fn make_free_glue(&@block_ctxt cx, ValueRef v0, &ty::t t) {
    // NB: v is an *alias* of type t here, not a direct value.

    auto rs = alt (ty::struct(cx.fcx.lcx.ccx.tcx, t)) {
        case (ty::ty_str) {
            auto v = cx.build.Load(v0);
            trans_non_gc_free(cx, v)
        }
        case (ty::ty_vec(_)) {
            auto v = cx.build.Load(v0);
            auto rs = iter_sequence(cx, v, t, bind drop_ty(_, _, _));
            // FIXME: switch gc/non-gc on layer of the type.
            trans_non_gc_free(rs.bcx, v)
        }
        case (ty::ty_box(?body_mt)) {
            auto v = cx.build.Load(v0);
            auto body =
                cx.build.GEP(v, ~[C_int(0), C_int(abi::box_rc_field_body)]);
            auto body_ty = body_mt.ty;
            auto body_val = load_if_immediate(cx, body, body_ty);
            auto rs = drop_ty(cx, body_val, body_ty);
            // FIXME: switch gc/non-gc on layer of the type.
            trans_non_gc_free(rs.bcx, v)
        }
        case (ty::ty_port(_)) {
            auto v = cx.build.Load(v0);
            cx.build.Call(cx.fcx.lcx.ccx.upcalls.del_port,
                          ~[cx.fcx.lltaskptr,
                            cx.build.PointerCast(v, T_opaque_port_ptr())]);
            rslt(cx, C_int(0))
        }
        case (ty::ty_chan(_)) {
            auto v = cx.build.Load(v0);
            cx.build.Call(cx.fcx.lcx.ccx.upcalls.del_chan,
                          ~[cx.fcx.lltaskptr,
                            cx.build.PointerCast(v, T_opaque_chan_ptr())]);
            rslt(cx, C_int(0))
        }
        case (ty::ty_task) {
            // TODO: call upcall_kill
            rslt(cx, C_nil())
        }
        case (ty::ty_obj(_)) {
            auto box_cell =
                cx.build.GEP(v0, ~[C_int(0), C_int(abi::obj_field_box)]);
            auto b = cx.build.Load(box_cell);

            auto ccx = cx.fcx.lcx.ccx;
            auto llbox_ty = T_opaque_obj_ptr(*ccx);
            b = cx.build.PointerCast(b, llbox_ty);

            auto body =
                cx.build.GEP(b, ~[C_int(0), C_int(abi::box_rc_field_body)]);
            auto tydescptr =
                cx.build.GEP(body,
                             ~[C_int(0), C_int(abi::obj_body_elt_tydesc)]);
            auto tydesc = cx.build.Load(tydescptr);
            auto cx_ = maybe_call_dtor(cx, v0);
            // Call through the obj's own fields-drop glue first.

            auto ti = none[@tydesc_info];
            call_tydesc_glue_full(cx_, body, tydesc,
                                  abi::tydesc_field_drop_glue, ti);
            // Then free the body.
            // FIXME: switch gc/non-gc on layer of the type.
            trans_non_gc_free(cx_, b)
        }
        case (ty::ty_fn(_, _, _, _, _)) {
            auto box_cell =
                cx.build.GEP(v0, ~[C_int(0), C_int(abi::fn_field_box)]);
            auto v = cx.build.Load(box_cell);
            // Call through the closure's own fields-drop glue first.

            auto body =
                cx.build.GEP(v, ~[C_int(0), C_int(abi::box_rc_field_body)]);
            auto bindings =
                cx.build.GEP(body,
                             ~[C_int(0), C_int(abi::closure_elt_bindings)]);
            auto tydescptr =
                cx.build.GEP(body,
                             ~[C_int(0), C_int(abi::closure_elt_tydesc)]);
            auto ti = none[@tydesc_info];
            call_tydesc_glue_full(cx, bindings, cx.build.Load(tydescptr),
                                  abi::tydesc_field_drop_glue, ti);
            // Then free the body.
            // FIXME: switch gc/non-gc on layer of the type.
            trans_non_gc_free(cx, v)
        }
        case (_) { rslt(cx, C_nil()) }
    };
    rs.bcx.build.RetVoid();
}

fn maybe_free_ivec_heap_part(&@block_ctxt cx, ValueRef v0, ty::t unit_ty) ->
   result {
    auto llunitty = type_of_or_i8(cx, unit_ty);
    auto stack_len =
        cx.build.Load(cx.build.InBoundsGEP(v0,
                                           ~[C_int(0),
                                             C_uint(abi::ivec_elt_len)]));
    auto maybe_on_heap_cx = new_sub_block_ctxt(cx, "maybe_on_heap");
    auto next_cx = new_sub_block_ctxt(cx, "next");
    auto maybe_on_heap =
        cx.build.ICmp(lib::llvm::LLVMIntEQ, stack_len, C_int(0));
    cx.build.CondBr(maybe_on_heap, maybe_on_heap_cx.llbb, next_cx.llbb);
    // Might be on the heap. Load the heap pointer and free it. (It's ok to
    // free a null pointer.)

    auto stub_ptr =
        maybe_on_heap_cx.build.PointerCast(v0, T_ptr(T_ivec_heap(llunitty)));
    auto heap_ptr =
        {
            auto v = ~[C_int(0), C_uint(abi::ivec_heap_stub_elt_ptr)];
            auto m = maybe_on_heap_cx.build.InBoundsGEP(stub_ptr, v);
            maybe_on_heap_cx.build.Load(m)
        };
    auto after_free_cx = trans_shared_free(maybe_on_heap_cx, heap_ptr).bcx;
    after_free_cx.build.Br(next_cx.llbb);
    ret rslt(next_cx, C_nil());
}

fn make_drop_glue(&@block_ctxt cx, ValueRef v0, &ty::t t) {
    // NB: v0 is an *alias* of type t here, not a direct value.
    auto ccx = cx.fcx.lcx.ccx;
    auto rs = alt (ty::struct(ccx.tcx, t)) {
        case (ty::ty_str) { decr_refcnt_maybe_free(cx, v0, v0, t) }
        case (ty::ty_vec(_)) { decr_refcnt_maybe_free(cx, v0, v0, t) }
        case (ty::ty_ivec(?tm)) {
            auto v1;
            if (ty::type_has_dynamic_size(ccx.tcx, tm.ty)) {
                v1 = cx.build.PointerCast(v0, T_ptr(T_opaque_ivec()));
            } else {
                v1 = v0;
            }

            auto rslt = iter_structural_ty(cx, v1, t, drop_ty);
            maybe_free_ivec_heap_part(rslt.bcx, v1, tm.ty)
        }
        case (ty::ty_box(_)) { decr_refcnt_maybe_free(cx, v0, v0, t) }
        case (ty::ty_port(_)) { decr_refcnt_maybe_free(cx, v0, v0, t) }
        case (ty::ty_chan(_)) { decr_refcnt_maybe_free(cx, v0, v0, t) }
        case (ty::ty_task) { decr_refcnt_maybe_free(cx, v0, v0, t) }
        case (ty::ty_obj(_)) {
            auto box_cell =
                cx.build.GEP(v0, ~[C_int(0), C_int(abi::obj_field_box)]);
            decr_refcnt_maybe_free(cx, box_cell, v0, t)
        }
        case (ty::ty_res(?did, ?inner, ?tps)) {
            trans_res_drop(cx, v0, did, inner, tps)
        }
        case (ty::ty_fn(_, _, _, _, _)) {
            auto box_cell =
                cx.build.GEP(v0, ~[C_int(0), C_int(abi::fn_field_box)]);
            decr_refcnt_maybe_free(cx, box_cell, v0, t)
        }
        case (_) {
            if (ty::type_has_pointers(ccx.tcx, t) &&
                    ty::type_is_structural(ccx.tcx, t)) {
                iter_structural_ty(cx, v0, t, bind drop_ty(_, _, _))
            } else { rslt(cx, C_nil()) }
        }
    };
    rs.bcx.build.RetVoid();
}

fn trans_res_drop(@block_ctxt cx, ValueRef rs, &ast::def_id did,
                  ty::t inner_t, &ty::t[] tps) -> result {
    auto ccx = cx.fcx.lcx.ccx;
    auto inner_t_s = ty::substitute_type_params(ccx.tcx, tps, inner_t);
    auto tup_ty = ty::mk_imm_tup(ccx.tcx, ~[ty::mk_int(ccx.tcx), inner_t_s]);
    auto drop_cx = new_sub_block_ctxt(cx, "drop res");
    auto next_cx = new_sub_block_ctxt(cx, "next");

    auto drop_flag = GEP_tup_like(cx,  tup_ty, rs, ~[0, 0]);
    cx = drop_flag.bcx;
    auto null_test = cx.build.IsNull(cx.build.Load(drop_flag.val));
    cx.build.CondBr(null_test, next_cx.llbb, drop_cx.llbb);
    cx = drop_cx;

    auto val = GEP_tup_like(cx, tup_ty, rs, ~[0, 1]);
    cx = val.bcx;
    // Find and call the actual destructor.
    auto dtor_pair = if (did._0 == ast::local_crate) {
        alt (ccx.fn_pairs.find(did._1)) {
            case (some(?x)) { x }
            case (_) { ccx.tcx.sess.bug("internal error in trans_res_drop") }
        }
    } else {
        auto params = csearch::get_type_param_count(ccx.sess.get_cstore(),
                                                    did);
        auto f_t = type_of_fn(ccx, cx.sp, ast::proto_fn,
                              ~[rec(mode=ty::mo_alias(false), ty=inner_t)],
                              ty::mk_nil(ccx.tcx), params);
        get_extern_const(ccx.externs, ccx.llmod,
                         csearch::get_symbol(ccx.sess.get_cstore(), did),
                         T_fn_pair(*ccx, f_t))
    };
    auto dtor_addr = cx.build.Load
        (cx.build.GEP(dtor_pair, ~[C_int(0), C_int(abi::fn_field_code)]));
    auto dtor_env = cx.build.Load
        (cx.build.GEP(dtor_pair, ~[C_int(0), C_int(abi::fn_field_box)]));
    auto args = ~[cx.fcx.llretptr, cx.fcx.lltaskptr, dtor_env];
    for (ty::t tp in tps) {
        let option::t[@tydesc_info] ti = none;
        auto td = get_tydesc(cx, tp, false, ti);
        args += ~[td.val];
        cx = td.bcx;
    }
    // Kludge to work around the fact that we know the precise type of the
    // value here, but the dtor expects a type that still has opaque pointers
    // for type variables.
    auto val_llty = lib::llvm::fn_ty_param_tys
        (llvm::LLVMGetElementType(llvm::LLVMTypeOf(dtor_addr)))
        .(std::ivec::len(args));
    auto val_cast = cx.build.BitCast(val.val, val_llty);
    cx.build.FastCall(dtor_addr, args + ~[val_cast]);

    cx = drop_slot(cx, val.val, inner_t_s).bcx;
    cx.build.Store(C_int(0), drop_flag.val);
    cx.build.Br(next_cx.llbb);
    ret rslt(next_cx, C_nil());
}

fn decr_refcnt_maybe_free(&@block_ctxt cx, ValueRef box_ptr_alias,
                          ValueRef full_alias, &ty::t t) -> result {
    auto ccx = cx.fcx.lcx.ccx;
    auto load_rc_cx = new_sub_block_ctxt(cx, "load rc");
    auto rc_adj_cx = new_sub_block_ctxt(cx, "rc--");
    auto free_cx = new_sub_block_ctxt(cx, "free");
    auto next_cx = new_sub_block_ctxt(cx, "next");
    auto box_ptr = cx.build.Load(box_ptr_alias);
    auto llbox_ty = T_opaque_obj_ptr(*ccx);
    box_ptr = cx.build.PointerCast(box_ptr, llbox_ty);
    auto null_test = cx.build.IsNull(box_ptr);
    cx.build.CondBr(null_test, next_cx.llbb, load_rc_cx.llbb);
    auto rc_ptr =
        load_rc_cx.build.GEP(box_ptr,
                             ~[C_int(0), C_int(abi::box_rc_field_refcnt)]);
    auto rc = load_rc_cx.build.Load(rc_ptr);
    auto const_test =
        load_rc_cx.build.ICmp(lib::llvm::LLVMIntEQ,
                              C_int(abi::const_refcount as int), rc);
    load_rc_cx.build.CondBr(const_test, next_cx.llbb, rc_adj_cx.llbb);
    rc = rc_adj_cx.build.Sub(rc, C_int(1));
    rc_adj_cx.build.Store(rc, rc_ptr);
    auto zero_test = rc_adj_cx.build.ICmp(lib::llvm::LLVMIntEQ, C_int(0), rc);
    rc_adj_cx.build.CondBr(zero_test, free_cx.llbb, next_cx.llbb);
    auto free_res =
        free_ty(free_cx, load_if_immediate(free_cx, full_alias, t), t);
    free_res.bcx.build.Br(next_cx.llbb);
    auto t_else = T_nil();
    auto v_else = C_nil();
    auto phi =
        next_cx.build.Phi(t_else, ~[v_else, v_else, v_else, free_res.val],
                          ~[cx.llbb, load_rc_cx.llbb, rc_adj_cx.llbb,
                            free_res.bcx.llbb]);
    ret rslt(next_cx, phi);
}


// Structural comparison: a rather involved form of glue.
fn maybe_name_value(&@crate_ctxt cx, ValueRef v, &str s) {
    if (cx.sess.get_opts().save_temps) {
        llvm::LLVMSetValueName(v, str::buf(s));
    }
}

fn make_cmp_glue(&@block_ctxt cx, ValueRef lhs0, ValueRef rhs0, &ty::t t,
                 ValueRef llop) {
    auto lhs = load_if_immediate(cx, lhs0, t);
    auto rhs = load_if_immediate(cx, rhs0, t);
    if (ty::type_is_scalar(cx.fcx.lcx.ccx.tcx, t)) {
        make_scalar_cmp_glue(cx, lhs, rhs, t, llop);
    } else if (ty::type_is_box(cx.fcx.lcx.ccx.tcx, t)) {
        lhs = cx.build.GEP(lhs, ~[C_int(0), C_int(abi::box_rc_field_body)]);
        rhs = cx.build.GEP(rhs, ~[C_int(0), C_int(abi::box_rc_field_body)]);
        auto t_inner =
            alt (ty::struct(cx.fcx.lcx.ccx.tcx, t)) {
                case (ty::ty_box(?ti)) { ti.ty }
            };
        auto rslt = compare(cx, lhs, rhs, t_inner, llop);
        rslt.bcx.build.Store(rslt.val, cx.fcx.llretptr);
        rslt.bcx.build.RetVoid();
    } else if (ty::type_is_structural(cx.fcx.lcx.ccx.tcx, t) ||
                   ty::type_is_sequence(cx.fcx.lcx.ccx.tcx, t)) {
        auto scx = new_sub_block_ctxt(cx, "structural compare start");
        auto next = new_sub_block_ctxt(cx, "structural compare end");
        cx.build.Br(scx.llbb);
        /*
         * We're doing lexicographic comparison here. We start with the
         * assumption that the two input elements are equal. Depending on
         * operator, this means that the result is either true or false;
         * equality produces 'true' for ==, <= and >=. It produces 'false' for
         * !=, < and >.
         *
         * We then move one element at a time through the structure checking
         * for pairwise element equality: If we have equality, our assumption
         * about overall sequence equality is not modified, so we have to move
         * to the next element.
         *
         * If we do not have pairwise element equality, we have reached an
         * element that 'decides' the lexicographic comparison. So we exit the
         * loop with a flag that indicates the true/false sense of that
         * decision, by testing the element again with the operator we're
         * interested in.
         *
         * When we're lucky, LLVM should be able to fold some of these two
         * tests together (as they're applied to the same operands and in some
         * cases are sometimes redundant). But we don't bother trying to
         * optimize combinations like that, at this level.
         */

        auto flag = alloca(scx, T_i1());
        maybe_name_value(cx.fcx.lcx.ccx, flag, "flag");
        auto r;
        if (ty::type_is_sequence(cx.fcx.lcx.ccx.tcx, t)) {
            // If we hit == all the way through the minimum-shared-length
            // section, default to judging the relative sequence lengths.

            auto lhs_fill;
            auto rhs_fill;
            auto bcx;
            if (ty::sequence_is_interior(cx.fcx.lcx.ccx.tcx, t)) {
                auto st = ty::sequence_element_type(cx.fcx.lcx.ccx.tcx, t);
                auto lad =
                    ivec::get_len_and_data(scx, lhs, st);
                bcx = lad._2;
                lhs_fill = lad._0;
                lad =
                    ivec::get_len_and_data(bcx, rhs, st);
                bcx = lad._2;
                rhs_fill = lad._0;
            } else {
                lhs_fill = vec_fill(scx, lhs);
                rhs_fill = vec_fill(scx, rhs);
                bcx = scx;
            }
            r = compare_scalar_values(bcx, lhs_fill, rhs_fill,
                                      unsigned_int, llop);
            r.bcx.build.Store(r.val, flag);
        } else {
            // == and <= default to true if they find == all the way. <
            // defaults to false if it finds == all the way.

            auto result_if_equal =
                scx.build.ICmp(lib::llvm::LLVMIntNE, llop,
                               C_u8(abi::cmp_glue_op_lt));
            scx.build.Store(result_if_equal, flag);
            r = rslt(scx, C_nil());
        }
        fn inner(@block_ctxt last_cx, bool load_inner, ValueRef flag,
                 ValueRef llop, &@block_ctxt cx, ValueRef av0, ValueRef bv0,
                 ty::t t) -> result {
            auto cnt_cx = new_sub_block_ctxt(cx, "continue_comparison");
            auto stop_cx = new_sub_block_ctxt(cx, "stop_comparison");
            auto av = av0;
            auto bv = bv0;
            if (load_inner) {
                // If `load_inner` is true, then the pointer type will always
                // be i8, because the data part of a vector always has type
                // i8[]. So we need to cast it to the proper type.

                if (!ty::type_has_dynamic_size(last_cx.fcx.lcx.ccx.tcx, t)) {
                    auto llelemty =
                        T_ptr(type_of(last_cx.fcx.lcx.ccx, last_cx.sp, t));
                    av = cx.build.PointerCast(av, llelemty);
                    bv = cx.build.PointerCast(bv, llelemty);
                }
                av = load_if_immediate(cx, av, t);
                bv = load_if_immediate(cx, bv, t);
            }

            // First 'eq' comparison: if so, continue to next elts.
            auto eq_r = compare(cx, av, bv, t, C_u8(abi::cmp_glue_op_eq));
            eq_r.bcx.build.CondBr(eq_r.val, cnt_cx.llbb, stop_cx.llbb);

            // Second 'op' comparison: find out how this elt-pair decides.
            auto stop_r = compare(stop_cx, av, bv, t, llop);
            stop_r.bcx.build.Store(stop_r.val, flag);
            stop_r.bcx.build.Br(last_cx.llbb);
            ret rslt(cnt_cx, C_nil());
        }
        if (ty::type_is_structural(cx.fcx.lcx.ccx.tcx, t)) {
            r =
                iter_structural_ty_full(r.bcx, lhs, rhs, t,
                                        bind inner(next, false, flag, llop, _,
                                                   _, _, _));
        } else {
            auto lhs_p0 = vec_p0(r.bcx, lhs);
            auto rhs_p0 = vec_p0(r.bcx, rhs);
            auto min_len =
                umin(r.bcx, vec_fill(r.bcx, lhs), vec_fill(r.bcx, rhs));
            auto rhs_lim = r.bcx.build.GEP(rhs_p0, ~[min_len]);
            auto elt_ty = ty::sequence_element_type(cx.fcx.lcx.ccx.tcx, t);
            r = size_of(r.bcx, elt_ty);
            r =
                iter_sequence_raw(r.bcx, lhs_p0, rhs_p0, rhs_lim, r.val,
                                  bind inner(next, true, flag, llop, _, _, _,
                                             elt_ty));
        }
        r.bcx.build.Br(next.llbb);
        auto v = next.build.Load(flag);
        next.build.Store(v, cx.fcx.llretptr);
        next.build.RetVoid();
    } else {
        // FIXME: compare obj, fn by pointer?

        trans_fail(cx, none[span],
                   "attempt to compare values of type " +
                       ty_to_str(cx.fcx.lcx.ccx.tcx, t));
    }
}


// Used only for creating scalar comparsion glue.
tag scalar_type { nil_type; signed_int; unsigned_int; floating_point; }


fn compare_scalar_types(@block_ctxt cx, ValueRef lhs, ValueRef rhs, &ty::t t,
                        ValueRef llop) -> result {
    // FIXME: this could be a lot shorter if we could combine multiple cases
    // of alt expressions (issue #449).

    auto f = bind compare_scalar_values(cx, lhs, rhs, _, llop);

    alt (ty::struct(cx.fcx.lcx.ccx.tcx, t)) {
        case (ty::ty_nil) { ret f(nil_type); }
        case (ty::ty_bool) { ret f(unsigned_int); }
        case (ty::ty_int) { ret f(signed_int); }
        case (ty::ty_float) { ret f(floating_point); }
        case (ty::ty_uint) { ret f(unsigned_int); }
        case (ty::ty_machine(_)) {

            if (ty::type_is_fp(cx.fcx.lcx.ccx.tcx, t)) {
                // Floating point machine types
                ret f(floating_point);
            } else if (ty::type_is_signed(cx.fcx.lcx.ccx.tcx, t)) {
                // Signed, integral machine types
                ret f(signed_int);
            } else {
                // Unsigned, integral machine types
                ret f(unsigned_int);
            }
        }
        case (ty::ty_char) { ret f(unsigned_int); }
        case (ty::ty_type) {
            trans_fail(cx, none[span],
                       "attempt to compare values of type type");

            // This is a bit lame, because we return a dummy block to the
            // caller that's actually unreachable, but I don't think it
            // matters.
            ret rslt(new_sub_block_ctxt(cx, "after_fail_dummy"),
                     C_bool(false));
        }
        case (ty::ty_native(_)) {
            trans_fail(cx, none[span],
                       "attempt to compare values of type native");
            ret rslt(new_sub_block_ctxt(cx, "after_fail_dummy"),
                     C_bool(false));
        }
        case (ty::ty_ptr(_)) {
            ret f(unsigned_int);
        }
        case (_) {
            // Should never get here, because t is scalar.
            cx.fcx.lcx.ccx.sess.bug("non-scalar type passed to " +
                                    "compare_scalar_types");
        }
    }
}

// A helper function to create scalar comparison glue.
fn make_scalar_cmp_glue(&@block_ctxt cx, ValueRef lhs, ValueRef rhs, &ty::t t,
                        ValueRef llop) {
    assert ty::type_is_scalar(cx.fcx.lcx.ccx.tcx, t);

    // In most cases, we need to know whether to do signed, unsigned, or float
    // comparison.

    auto rslt = compare_scalar_types(cx, lhs, rhs, t, llop);
    auto bcx = rslt.bcx;
    auto compare_result = rslt.val;
    bcx.build.Store(compare_result, cx.fcx.llretptr);
    bcx.build.RetVoid();
}


// A helper function to do the actual comparison of scalar values.
fn compare_scalar_values(&@block_ctxt cx, ValueRef lhs, ValueRef rhs,
                         scalar_type nt, ValueRef llop) -> result {
    auto eq_cmp;
    auto lt_cmp;
    auto le_cmp;
    alt (nt) {
        case (nil_type) {
            // We don't need to do actual comparisons for nil.
            // () == () holds but () < () does not.
            eq_cmp = 1u;
            lt_cmp = 0u;
            le_cmp = 1u;
        }
        case (floating_point) {
            eq_cmp = lib::llvm::LLVMRealUEQ;
            lt_cmp = lib::llvm::LLVMRealULT;
            le_cmp = lib::llvm::LLVMRealULE;
        }
        case (signed_int) {
            eq_cmp = lib::llvm::LLVMIntEQ;
            lt_cmp = lib::llvm::LLVMIntSLT;
            le_cmp = lib::llvm::LLVMIntSLE;
        }
        case (unsigned_int) {
            eq_cmp = lib::llvm::LLVMIntEQ;
            lt_cmp = lib::llvm::LLVMIntULT;
            le_cmp = lib::llvm::LLVMIntULE;
        }
    }
    // FIXME: This wouldn't be necessary if we could bind methods off of
    // objects and therefore abstract over FCmp and ICmp (issue #435).  Then
    // we could just write, e.g., "cmp_fn = bind cx.build.FCmp(_, _, _);" in
    // the above, and "auto eq_result = cmp_fn(eq_cmp, lhs, rhs);" in the
    // below.

    fn generic_cmp(&@block_ctxt cx, scalar_type nt, uint op, ValueRef lhs,
                   ValueRef rhs) -> ValueRef {
        let ValueRef r;
        if (nt == nil_type) {
            r = C_bool(op != 0u);
        } else if (nt == floating_point) {
            r = cx.build.FCmp(op, lhs, rhs);
        } else { r = cx.build.ICmp(op, lhs, rhs); }
        ret r;
    }
    auto last_cx = new_sub_block_ctxt(cx, "last");
    auto eq_cx = new_sub_block_ctxt(cx, "eq");
    auto eq_result = generic_cmp(eq_cx, nt, eq_cmp, lhs, rhs);
    eq_cx.build.Br(last_cx.llbb);
    auto lt_cx = new_sub_block_ctxt(cx, "lt");
    auto lt_result = generic_cmp(lt_cx, nt, lt_cmp, lhs, rhs);
    lt_cx.build.Br(last_cx.llbb);
    auto le_cx = new_sub_block_ctxt(cx, "le");
    auto le_result = generic_cmp(le_cx, nt, le_cmp, lhs, rhs);
    le_cx.build.Br(last_cx.llbb);
    auto unreach_cx = new_sub_block_ctxt(cx, "unreach");
    unreach_cx.build.Unreachable();
    auto llswitch = cx.build.Switch(llop, unreach_cx.llbb, 3u);
    llvm::LLVMAddCase(llswitch, C_u8(abi::cmp_glue_op_eq), eq_cx.llbb);
    llvm::LLVMAddCase(llswitch, C_u8(abi::cmp_glue_op_lt), lt_cx.llbb);
    llvm::LLVMAddCase(llswitch, C_u8(abi::cmp_glue_op_le), le_cx.llbb);
    auto last_result =
        last_cx.build.Phi(T_i1(), ~[eq_result, lt_result, le_result],
                          ~[eq_cx.llbb, lt_cx.llbb, le_cx.llbb]);
    ret rslt(last_cx, last_result);
}

type val_pair_fn = fn(&@block_ctxt, ValueRef, ValueRef) -> result ;

type val_and_ty_fn = fn(&@block_ctxt, ValueRef, ty::t) -> result ;

type val_pair_and_ty_fn =
    fn(&@block_ctxt, ValueRef, ValueRef, ty::t) -> result ;


// Iterates through the elements of a structural type.
fn iter_structural_ty(&@block_ctxt cx, ValueRef v, &ty::t t, val_and_ty_fn f)
   -> result {
    fn adaptor_fn(val_and_ty_fn f, &@block_ctxt cx, ValueRef av, ValueRef bv,
                  ty::t t) -> result {
        ret f(cx, av, t);
    }
    ret iter_structural_ty_full(cx, v, v, t, bind adaptor_fn(f, _, _, _, _));
}

fn load_inbounds(&@block_ctxt cx, ValueRef p, &ValueRef[] idxs) -> ValueRef {
    ret cx.build.Load(cx.build.InBoundsGEP(p, idxs));
}

fn store_inbounds(&@block_ctxt cx, ValueRef v, ValueRef p, &ValueRef[] idxs) {
    cx.build.Store(v, cx.build.InBoundsGEP(p, idxs));
}

// This uses store and inboundsGEP, but it only doing so superficially; it's
// really storing an incremented pointer to another pointer.
fn incr_ptr(&@block_ctxt cx, ValueRef p, ValueRef incr, ValueRef pp) {
    cx.build.Store(cx.build.InBoundsGEP(p, ~[incr]), pp);
}

fn iter_structural_ty_full(&@block_ctxt cx, ValueRef av, ValueRef bv,
                           &ty::t t, &val_pair_and_ty_fn f) -> result {
    fn iter_boxpp(@block_ctxt cx, ValueRef box_a_cell, ValueRef box_b_cell,
                  &val_pair_and_ty_fn f) -> result {
        auto box_a_ptr = cx.build.Load(box_a_cell);
        auto box_b_ptr = cx.build.Load(box_b_cell);
        auto tnil = ty::mk_nil(cx.fcx.lcx.ccx.tcx);
        auto tbox = ty::mk_imm_box(cx.fcx.lcx.ccx.tcx, tnil);
        auto inner_cx = new_sub_block_ctxt(cx, "iter box");
        auto next_cx = new_sub_block_ctxt(cx, "next");
        auto null_test = cx.build.IsNull(box_a_ptr);
        cx.build.CondBr(null_test, next_cx.llbb, inner_cx.llbb);
        auto r = f(inner_cx, box_a_ptr, box_b_ptr, tbox);
        r.bcx.build.Br(next_cx.llbb);
        ret rslt(next_cx, C_nil());
    }

    fn iter_ivec(@block_ctxt bcx, ValueRef av, ValueRef bv, ty::t unit_ty,
                 &val_pair_and_ty_fn f) -> result {
        // FIXME: "unimplemented rebinding existing function" workaround

        fn adapter(&@block_ctxt bcx, ValueRef av, ValueRef bv, ty::t unit_ty,
                   val_pair_and_ty_fn f) -> result {
            ret f(bcx, av, bv, unit_ty);
        }
        auto llunitty = type_of_or_i8(bcx, unit_ty);
        auto rs = size_of(bcx, unit_ty);
        auto unit_sz = rs.val;
        bcx = rs.bcx;
        auto a_len_and_data = ivec::get_len_and_data(bcx, av, unit_ty);
        auto a_len = a_len_and_data._0;
        auto a_elem = a_len_and_data._1;
        bcx = a_len_and_data._2;
        auto b_len_and_data = ivec::get_len_and_data(bcx, bv, unit_ty);
        auto b_len = b_len_and_data._0;
        auto b_elem = b_len_and_data._1;
        bcx = b_len_and_data._2;
        // Calculate the last pointer address we want to handle.
        // TODO: Optimize this when the size of the unit type is statically
        // known to not use pointer casts, which tend to confuse LLVM.

        auto len = umin(bcx, a_len, b_len);
        auto b_elem_i8 = bcx.build.PointerCast(b_elem, T_ptr(T_i8()));
        auto b_end_i8 = bcx.build.GEP(b_elem_i8, ~[len]);
        auto b_end = bcx.build.PointerCast(b_end_i8, T_ptr(llunitty));

        auto dest_elem_ptr = alloca(bcx, T_ptr(llunitty));
        auto src_elem_ptr = alloca(bcx, T_ptr(llunitty));
        bcx.build.Store(a_elem, dest_elem_ptr);
        bcx.build.Store(b_elem, src_elem_ptr);

        // Now perform the iteration.
        auto loop_header_cx = new_sub_block_ctxt(bcx,
                                                 "iter_ivec_loop_header");
        bcx.build.Br(loop_header_cx.llbb);
        auto dest_elem = loop_header_cx.build.Load(dest_elem_ptr);
        auto src_elem = loop_header_cx.build.Load(src_elem_ptr);
        auto not_yet_at_end = loop_header_cx.build.ICmp(lib::llvm::LLVMIntULT,
                                                        dest_elem, b_end);
        auto loop_body_cx = new_sub_block_ctxt(bcx, "iter_ivec_loop_body");
        auto next_cx = new_sub_block_ctxt(bcx, "iter_ivec_next");
        loop_header_cx.build.CondBr(not_yet_at_end, loop_body_cx.llbb,
                                    next_cx.llbb);

        rs = f(loop_body_cx,
               load_if_immediate(loop_body_cx, dest_elem, unit_ty),
               load_if_immediate(loop_body_cx, src_elem, unit_ty), unit_ty);

        loop_body_cx = rs.bcx;

        auto increment;
        if (ty::type_has_dynamic_size(bcx.fcx.lcx.ccx.tcx, unit_ty)) {
            increment = unit_sz;
        } else {
            increment = C_int(1);
        }

        incr_ptr(loop_body_cx, dest_elem, increment, dest_elem_ptr);
        incr_ptr(loop_body_cx, src_elem, increment, src_elem_ptr);
        loop_body_cx.build.Br(loop_header_cx.llbb);

        ret rslt(next_cx, C_nil());
    }

    fn iter_variant(@block_ctxt cx, ValueRef a_tup, ValueRef b_tup,
                    &ty::variant_info variant, &ty::t[] tps,
                    &ast::def_id tid, &val_pair_and_ty_fn f) -> result {
        if (std::ivec::len[ty::t](variant.args) == 0u) {
            ret rslt(cx, C_nil());
        }
        auto fn_ty = variant.ctor_ty;
        auto ccx = cx.fcx.lcx.ccx;
        alt (ty::struct(ccx.tcx, fn_ty)) {
            case (ty::ty_fn(_, ?args, _, _, _)) {
                auto j = 0;
                for (ty::arg a in args) {
                    auto rslt = GEP_tag(cx, a_tup, tid, variant.id, tps, j);
                    auto llfldp_a = rslt.val;
                    cx = rslt.bcx;
                    rslt = GEP_tag(cx, b_tup, tid, variant.id, tps, j);
                    auto llfldp_b = rslt.val;
                    cx = rslt.bcx;
                    auto ty_subst =
                        ty::substitute_type_params(ccx.tcx, tps, a.ty);
                    auto llfld_a =
                        load_if_immediate(cx, llfldp_a, ty_subst);
                    auto llfld_b =
                        load_if_immediate(cx, llfldp_b, ty_subst);
                    rslt = f(cx, llfld_a, llfld_b, ty_subst);
                    cx = rslt.bcx;
                    j += 1;
                }
            }
        }
        ret rslt(cx, C_nil());
    }

    let result r = rslt(cx, C_nil());
    alt (ty::struct(cx.fcx.lcx.ccx.tcx, t)) {
        case (ty::ty_tup(?args)) {
            let int i = 0;
            for (ty::mt arg in args) {
                r = GEP_tup_like(r.bcx, t, av, ~[0, i]);
                auto elt_a = r.val;
                r = GEP_tup_like(r.bcx, t, bv, ~[0, i]);
                auto elt_b = r.val;
                r = f(r.bcx, load_if_immediate(r.bcx, elt_a, arg.ty),
                      load_if_immediate(r.bcx, elt_b, arg.ty), arg.ty);
                i += 1;
            }
        }
        case (ty::ty_rec(?fields)) {
            let int i = 0;
            for (ty::field fld in fields) {
                r = GEP_tup_like(r.bcx, t, av, ~[0, i]);
                auto llfld_a = r.val;
                r = GEP_tup_like(r.bcx, t, bv, ~[0, i]);
                auto llfld_b = r.val;
                r = f(r.bcx, load_if_immediate(r.bcx, llfld_a, fld.mt.ty),
                      load_if_immediate(r.bcx, llfld_b, fld.mt.ty),
                      fld.mt.ty);
                i += 1;
            }
        }
        case (ty::ty_res(_, ?inner, ?tps)) {
            auto inner1 = ty::substitute_type_params(cx.fcx.lcx.ccx.tcx, tps,
                                                     inner);
            r = GEP_tup_like(r.bcx, t, av, ~[0, 1]);
            auto llfld_a = r.val;
            r = GEP_tup_like(r.bcx, t, bv, ~[0, 1]);
            auto llfld_b = r.val;
            f(r.bcx, load_if_immediate(r.bcx, llfld_a, inner1),
              load_if_immediate(r.bcx, llfld_b, inner1), inner1);
        }
        case (ty::ty_tag(?tid, ?tps)) {
            auto variants = ty::tag_variants(cx.fcx.lcx.ccx.tcx, tid);
            auto n_variants = std::ivec::len(variants);

            // Cast the tags to types we can GEP into.
            if (n_variants == 1u) {
                ret iter_variant(cx, av, bv, variants.(0), tps, tid, f);
            }

            auto lltagty = T_opaque_tag_ptr(cx.fcx.lcx.ccx.tn);
            auto av_tag = cx.build.PointerCast(av, lltagty);
            auto bv_tag = cx.build.PointerCast(bv, lltagty);
            auto lldiscrim_a_ptr = cx.build.GEP(av_tag,
                                                ~[C_int(0), C_int(0)]);
            auto llunion_a_ptr = cx.build.GEP(av_tag, ~[C_int(0), C_int(1)]);
            auto lldiscrim_a = cx.build.Load(lldiscrim_a_ptr);
            auto lldiscrim_b_ptr = cx.build.GEP(bv_tag,
                                                ~[C_int(0), C_int(0)]);
            auto llunion_b_ptr = cx.build.GEP(bv_tag, ~[C_int(0), C_int(1)]);
            auto lldiscrim_b = cx.build.Load(lldiscrim_b_ptr);

            // NB: we must hit the discriminant first so that structural
            // comparison know not to proceed when the discriminants differ.
            auto bcx = cx;
            bcx =
                f(bcx, lldiscrim_a, lldiscrim_b,
                  ty::mk_int(cx.fcx.lcx.ccx.tcx)).bcx;
            auto unr_cx = new_sub_block_ctxt(bcx, "tag-iter-unr");
            unr_cx.build.Unreachable();
            auto llswitch =
                bcx.build.Switch(lldiscrim_a, unr_cx.llbb, n_variants);
            auto next_cx = new_sub_block_ctxt(bcx, "tag-iter-next");
            auto i = 0u;
            for (ty::variant_info variant in variants) {
                auto variant_cx =
                    new_sub_block_ctxt(bcx,
                                       "tag-iter-variant-" +
                                           uint::to_str(i, 10u));
                llvm::LLVMAddCase(llswitch, C_int(i as int), variant_cx.llbb);
                variant_cx = iter_variant
                    (variant_cx, llunion_a_ptr, llunion_b_ptr, variant,
                     tps, tid, f).bcx;
                variant_cx.build.Br(next_cx.llbb);
                i += 1u;
            }
            ret rslt(next_cx, C_nil());
        }
        case (ty::ty_fn(_, _, _, _, _)) {
            auto box_cell_a =
                cx.build.GEP(av, ~[C_int(0), C_int(abi::fn_field_box)]);
            auto box_cell_b =
                cx.build.GEP(bv, ~[C_int(0), C_int(abi::fn_field_box)]);
            ret iter_boxpp(cx, box_cell_a, box_cell_b, f);
        }
        case (ty::ty_obj(_)) {
            auto box_cell_a =
                cx.build.GEP(av, ~[C_int(0), C_int(abi::obj_field_box)]);
            auto box_cell_b =
                cx.build.GEP(bv, ~[C_int(0), C_int(abi::obj_field_box)]);
            ret iter_boxpp(cx, box_cell_a, box_cell_b, f);
        }
        case (ty::ty_ivec(?unit_tm)) {
            ret iter_ivec(cx, av, bv, unit_tm.ty, f);
        }
        case (ty::ty_istr) {
            auto unit_ty = ty::mk_mach(cx.fcx.lcx.ccx.tcx, ast::ty_u8);
            ret iter_ivec(cx, av, bv, unit_ty, f);
        }
        case (_) {
            cx.fcx.lcx.ccx.sess.unimpl("type in iter_structural_ty_full");
        }
    }
    ret r;
}


// Iterates through a pointer range, until the src* hits the src_lim*.
fn iter_sequence_raw(@block_ctxt cx, ValueRef dst,

                         // elt*
                         ValueRef src,

                         // elt*
                         ValueRef src_lim,

                         // elt*
                         ValueRef elt_sz, &val_pair_fn f) -> result {
    auto bcx = cx;
    let ValueRef dst_int = vp2i(bcx, dst);
    let ValueRef src_int = vp2i(bcx, src);
    let ValueRef src_lim_int = vp2i(bcx, src_lim);
    auto cond_cx = new_scope_block_ctxt(cx, "sequence-iter cond");
    auto body_cx = new_scope_block_ctxt(cx, "sequence-iter body");
    auto next_cx = new_sub_block_ctxt(cx, "next");
    bcx.build.Br(cond_cx.llbb);
    let ValueRef dst_curr = cond_cx.build.Phi(T_int(), ~[dst_int],
                                              ~[bcx.llbb]);
    let ValueRef src_curr = cond_cx.build.Phi(T_int(), ~[src_int],
                                              ~[bcx.llbb]);
    auto end_test =
        cond_cx.build.ICmp(lib::llvm::LLVMIntULT, src_curr, src_lim_int);
    cond_cx.build.CondBr(end_test, body_cx.llbb, next_cx.llbb);
    auto dst_curr_ptr = vi2p(body_cx, dst_curr, T_ptr(T_i8()));
    auto src_curr_ptr = vi2p(body_cx, src_curr, T_ptr(T_i8()));
    auto body_res = f(body_cx, dst_curr_ptr, src_curr_ptr);
    body_cx = body_res.bcx;
    auto dst_next = body_cx.build.Add(dst_curr, elt_sz);
    auto src_next = body_cx.build.Add(src_curr, elt_sz);
    body_cx.build.Br(cond_cx.llbb);
    cond_cx.build.AddIncomingToPhi(dst_curr, ~[dst_next], ~[body_cx.llbb]);
    cond_cx.build.AddIncomingToPhi(src_curr, ~[src_next], ~[body_cx.llbb]);
    ret rslt(next_cx, C_nil());
}

fn iter_sequence_inner(&@block_ctxt cx, ValueRef src,

                           // elt*
                           ValueRef src_lim,
                       & // elt*
                           ty::t elt_ty, &val_and_ty_fn f) -> result {
    fn adaptor_fn(val_and_ty_fn f, ty::t elt_ty, &@block_ctxt cx,
                  ValueRef dst, ValueRef src) -> result {
        auto llptrty;
        if (!ty::type_has_dynamic_size(cx.fcx.lcx.ccx.tcx, elt_ty)) {
            auto llty = type_of(cx.fcx.lcx.ccx, cx.sp, elt_ty);
            llptrty = T_ptr(llty);
        } else { llptrty = T_ptr(T_ptr(T_i8())); }
        auto p = cx.build.PointerCast(src, llptrty);
        ret f(cx, load_if_immediate(cx, p, elt_ty), elt_ty);
    }
    auto elt_sz = size_of(cx, elt_ty);
    ret iter_sequence_raw(elt_sz.bcx, src, src, src_lim, elt_sz.val,
                          bind adaptor_fn(f, elt_ty, _, _, _));
}


// Iterates through the elements of a vec or str.
fn iter_sequence(@block_ctxt cx, ValueRef v, &ty::t t, &val_and_ty_fn f) ->
   result {
    fn iter_sequence_body(@block_ctxt cx, ValueRef v, &ty::t elt_ty,
                          &val_and_ty_fn f, bool trailing_null, bool interior)
            -> result {
        auto p0;
        auto len;
        auto bcx;
        if (!interior) {
            p0 = cx.build.GEP(v, ~[C_int(0), C_int(abi::vec_elt_data)]);
            auto lp = cx.build.GEP(v, ~[C_int(0), C_int(abi::vec_elt_fill)]);
            len = cx.build.Load(lp);
            bcx = cx;
        } else {
            auto len_and_data_rslt = ivec::get_len_and_data(cx, v, elt_ty);
            len = len_and_data_rslt._0;
            p0 = len_and_data_rslt._1;
            bcx = len_and_data_rslt._2;
        }

        auto llunit_ty = type_of_or_i8(cx, elt_ty);
        if (trailing_null) {
            auto unit_sz = size_of(bcx, elt_ty);
            bcx = unit_sz.bcx;
            len = bcx.build.Sub(len, unit_sz.val);
        }
        auto p1 =
            vi2p(bcx, bcx.build.Add(vp2i(bcx, p0), len), T_ptr(llunit_ty));
        ret iter_sequence_inner(bcx, p0, p1, elt_ty, f);
    }

    alt (ty::struct(cx.fcx.lcx.ccx.tcx, t)) {
        case (ty::ty_vec(?elt)) {
            ret iter_sequence_body(cx, v, elt.ty, f, false, false);
        }
        case (ty::ty_str) {
            auto et = ty::mk_mach(cx.fcx.lcx.ccx.tcx, ast::ty_u8);
            ret iter_sequence_body(cx, v, et, f, true, false);
        }
        case (ty::ty_ivec(?elt)) {
            ret iter_sequence_body(cx, v, elt.ty, f, false, true);
        }
        case (ty::ty_istr) {
            auto et = ty::mk_mach(cx.fcx.lcx.ccx.tcx, ast::ty_u8);
            ret iter_sequence_body(cx, v, et, f, true, true);
        }
        case (_) {
            cx.fcx.lcx.ccx.sess.bug("unexpected type in " +
                                        "trans::iter_sequence: " +
                                        ty_to_str(cx.fcx.lcx.ccx.tcx, t));
        }
    }
}

fn lazily_emit_all_tydesc_glue(&@block_ctxt cx,
                               &option::t[@tydesc_info] static_ti) {
    lazily_emit_tydesc_glue(cx, abi::tydesc_field_copy_glue, static_ti);
    lazily_emit_tydesc_glue(cx, abi::tydesc_field_drop_glue, static_ti);
    lazily_emit_tydesc_glue(cx, abi::tydesc_field_free_glue, static_ti);
    lazily_emit_tydesc_glue(cx, abi::tydesc_field_cmp_glue, static_ti);
}

fn lazily_emit_all_generic_info_tydesc_glues(&@block_ctxt cx,
                                             &generic_info gi) {
    for (option::t[@tydesc_info] ti in gi.static_tis) {
        lazily_emit_all_tydesc_glue(cx, ti);
    }
}

fn lazily_emit_tydesc_glue(&@block_ctxt cx, int field,
                           &option::t[@tydesc_info] static_ti) {
    alt (static_ti) {
        case (none) { }
        case (some(?ti)) {
            if (field == abi::tydesc_field_copy_glue) {
                alt ({ ti.copy_glue }) {
                    case (some(_)) { }
                    case (none) {
                        log #fmt("+++ lazily_emit_tydesc_glue TAKE %s",
                                 ty_to_str(cx.fcx.lcx.ccx.tcx, ti.ty));
                        auto lcx = cx.fcx.lcx;
                        auto glue_fn =
                            declare_generic_glue(lcx, ti.ty,
                                                 T_glue_fn(*lcx.ccx),
                                                 "copy");
                        ti.copy_glue = some[ValueRef](glue_fn);
                        make_generic_glue(lcx, cx.sp, ti.ty, glue_fn,
                                          mgghf_single(make_copy_glue),
                                         ti.ty_params, "take");
                        log #fmt("--- lazily_emit_tydesc_glue TAKE %s",
                                 ty_to_str(cx.fcx.lcx.ccx.tcx, ti.ty));
                    }
                }
            } else if (field == abi::tydesc_field_drop_glue) {
                alt ({ ti.drop_glue }) {
                    case (some(_)) { }
                    case (none) {
                        log #fmt("+++ lazily_emit_tydesc_glue DROP %s",
                                 ty_to_str(cx.fcx.lcx.ccx.tcx, ti.ty));
                        auto lcx = cx.fcx.lcx;
                        auto glue_fn =
                            declare_generic_glue(lcx, ti.ty,
                                                 T_glue_fn(*lcx.ccx),
                                                 "drop");
                        ti.drop_glue = some[ValueRef](glue_fn);
                        make_generic_glue(lcx, cx.sp, ti.ty, glue_fn,
                                          mgghf_single(make_drop_glue),
                                          ti.ty_params, "drop");
                        log #fmt("--- lazily_emit_tydesc_glue DROP %s",
                                 ty_to_str(cx.fcx.lcx.ccx.tcx, ti.ty));
                    }
                }
            } else if (field == abi::tydesc_field_free_glue) {
                alt ({ ti.free_glue }) {
                    case (some(_)) { }
                    case (none) {
                        log #fmt("+++ lazily_emit_tydesc_glue FREE %s",
                                 ty_to_str(cx.fcx.lcx.ccx.tcx, ti.ty));
                        auto lcx = cx.fcx.lcx;
                        auto glue_fn =
                            declare_generic_glue(lcx, ti.ty,
                                                 T_glue_fn(*lcx.ccx),
                                                 "free");
                        ti.free_glue = some[ValueRef](glue_fn);
                        make_generic_glue(lcx, cx.sp, ti.ty, glue_fn,
                                          mgghf_single(make_free_glue),
                                         ti.ty_params, "free");
                        log #fmt("--- lazily_emit_tydesc_glue FREE %s",
                                 ty_to_str(cx.fcx.lcx.ccx.tcx, ti.ty));
                    }
                }
            } else if (field == abi::tydesc_field_cmp_glue) {
                alt ({ ti.cmp_glue }) {
                    case (some(_)) { }
                    case (none) {
                        log #fmt("+++ lazily_emit_tydesc_glue CMP %s",
                                 ty_to_str(cx.fcx.lcx.ccx.tcx, ti.ty));
                        auto lcx = cx.fcx.lcx;
                        auto glue_fn =
                            declare_generic_glue(lcx, ti.ty,
                                                 T_cmp_glue_fn(*lcx.ccx),
                                                 "cmp");
                        ti.cmp_glue = some[ValueRef](glue_fn);
                        make_generic_glue(lcx, cx.sp, ti.ty, glue_fn,
                                          mgghf_cmp, ti.ty_params, "cmp");
                        log #fmt("--- lazily_emit_tydesc_glue CMP %s",
                                 ty_to_str(cx.fcx.lcx.ccx.tcx, ti.ty));
                    }
                }
            }
        }
    }
}

fn call_tydesc_glue_full(&@block_ctxt cx, ValueRef v, ValueRef tydesc,
                         int field, &option::t[@tydesc_info] static_ti) {
    lazily_emit_tydesc_glue(cx, field, static_ti);

    auto static_glue_fn = none;
    alt (static_ti) {
      case (none) { /* no-op */ }
      case (some(?sti)) {
        if (field == abi::tydesc_field_copy_glue) {
            static_glue_fn = sti.copy_glue;
        } else if (field == abi::tydesc_field_drop_glue) {
            static_glue_fn = sti.drop_glue;
        } else if (field == abi::tydesc_field_free_glue) {
            static_glue_fn = sti.free_glue;
        } else if (field == abi::tydesc_field_cmp_glue) {
            static_glue_fn = sti.cmp_glue;
        }
      }
    }

    auto llrawptr = cx.build.BitCast(v, T_ptr(T_i8()));
    auto lltydescs =
        cx.build.GEP(tydesc,
                     ~[C_int(0), C_int(abi::tydesc_field_first_param)]);
    lltydescs = cx.build.Load(lltydescs);

    auto llfn;
    alt (static_glue_fn) {
      case (none) {
        auto llfnptr = cx.build.GEP(tydesc, ~[C_int(0), C_int(field)]);
        llfn = cx.build.Load(llfnptr);
      }
      case (some(?sgf)) { llfn = sgf; }
    }

    cx.build.Call(llfn,
                  ~[C_null(T_ptr(T_nil())), cx.fcx.lltaskptr,
                    C_null(T_ptr(T_nil())), lltydescs, llrawptr]);
}

fn call_tydesc_glue(&@block_ctxt cx, ValueRef v, &ty::t t, int field) ->
   result {
    let option::t[@tydesc_info] ti = none[@tydesc_info];
    auto td = get_tydesc(cx, t, false, ti);
    call_tydesc_glue_full(td.bcx, spill_if_immediate(td.bcx, v, t), td.val,
                          field, ti);
    ret rslt(td.bcx, C_nil());
}

fn maybe_call_dtor(&@block_ctxt cx, ValueRef v) -> @block_ctxt {
    auto vtbl = cx.build.GEP(v, ~[C_int(0), C_int(abi::obj_field_vtbl)]);
    vtbl = cx.build.Load(vtbl);
    auto vtbl_type = T_ptr(T_array(T_ptr(T_nil()), 1u));
    vtbl = cx.build.PointerCast(vtbl, vtbl_type);

    auto dtor_ptr = cx.build.GEP(vtbl, ~[C_int(0), C_int(0)]);
    dtor_ptr = cx.build.Load(dtor_ptr);
    dtor_ptr =
        cx.build.BitCast(dtor_ptr,
                         T_ptr(T_dtor(cx.fcx.lcx.ccx, cx.sp)));
    auto dtor_cx = new_sub_block_ctxt(cx, "dtor");
    auto after_cx = new_sub_block_ctxt(cx, "after_dtor");
    auto test =
        cx.build.ICmp(lib::llvm::LLVMIntNE, dtor_ptr,
                      C_null(val_ty(dtor_ptr)));
    cx.build.CondBr(test, dtor_cx.llbb, after_cx.llbb);
    auto me = dtor_cx.build.Load(v);
    dtor_cx.build.FastCall(dtor_ptr,
                           ~[C_null(T_ptr(T_nil())), cx.fcx.lltaskptr, me]);
    dtor_cx.build.Br(after_cx.llbb);
    ret after_cx;
}

fn call_cmp_glue(&@block_ctxt cx, ValueRef lhs, ValueRef rhs, &ty::t t,
                 ValueRef llop) -> result {
    // We can't use call_tydesc_glue_full() and friends here because compare
    // glue has a special signature.

    auto lllhs = spill_if_immediate(cx, lhs, t);
    auto llrhs = spill_if_immediate(cx, rhs, t);
    auto llrawlhsptr = cx.build.BitCast(lllhs, T_ptr(T_i8()));
    auto llrawrhsptr = cx.build.BitCast(llrhs, T_ptr(T_i8()));
    auto ti = none[@tydesc_info];
    auto r = get_tydesc(cx, t, false, ti);
    lazily_emit_tydesc_glue(cx, abi::tydesc_field_cmp_glue, ti);
    auto lltydescs =
        r.bcx.build.GEP(r.val,
                        ~[C_int(0), C_int(abi::tydesc_field_first_param)]);
    lltydescs = r.bcx.build.Load(lltydescs);

    auto llfn;
    alt (ti) {
      case (none) {
        auto llfnptr = r.bcx.build.GEP(r.val, ~[C_int(0),
            C_int(abi::tydesc_field_cmp_glue)]);
        llfn = r.bcx.build.Load(llfnptr);
      }
      case (some(?sti)) { llfn = option::get(sti.cmp_glue); }
    }

    auto llcmpresultptr = alloca(r.bcx, T_i1());
    let ValueRef[] llargs =
        ~[llcmpresultptr, r.bcx.fcx.lltaskptr, C_null(T_ptr(T_nil())),
          lltydescs, llrawlhsptr, llrawrhsptr, llop];
    r.bcx.build.Call(llfn, llargs);
    ret rslt(r.bcx, r.bcx.build.Load(llcmpresultptr));
}

// Compares two values. Performs the simple scalar comparison if the types are
// scalar and calls to comparison glue otherwise.
fn compare(&@block_ctxt cx, ValueRef lhs, ValueRef rhs, &ty::t t,
           ValueRef llop) -> result {
    if (ty::type_is_scalar(cx.fcx.lcx.ccx.tcx, t)) {
        ret compare_scalar_types(cx, lhs, rhs, t, llop);
    }
    ret call_cmp_glue(cx, lhs, rhs, t, llop);
}

fn copy_ty(&@block_ctxt cx, ValueRef v, ty::t t) -> result {
    if (ty::type_has_pointers(cx.fcx.lcx.ccx.tcx, t) ||
            ty::type_owns_heap_mem(cx.fcx.lcx.ccx.tcx, t)) {
        ret call_tydesc_glue(cx, v, t, abi::tydesc_field_copy_glue);
    }
    ret rslt(cx, C_nil());
}

fn drop_slot(&@block_ctxt cx, ValueRef slot, &ty::t t) -> result {
    auto llptr = load_if_immediate(cx, slot, t);
    auto re = drop_ty(cx, llptr, t);
    auto llty = val_ty(slot);
    auto llelemty = lib::llvm::llvm::LLVMGetElementType(llty);
    re.bcx.build.Store(C_null(llelemty), slot);
    ret re;
}

fn drop_ty(&@block_ctxt cx, ValueRef v, ty::t t) -> result {
    if (ty::type_has_pointers(cx.fcx.lcx.ccx.tcx, t)) {
        ret call_tydesc_glue(cx, v, t, abi::tydesc_field_drop_glue);
    }
    ret rslt(cx, C_nil());
}

fn free_ty(&@block_ctxt cx, ValueRef v, ty::t t) -> result {
    if (ty::type_has_pointers(cx.fcx.lcx.ccx.tcx, t)) {
        ret call_tydesc_glue(cx, v, t, abi::tydesc_field_free_glue);
    }
    ret rslt(cx, C_nil());
}

fn call_memmove(&@block_ctxt cx, ValueRef dst, ValueRef src,
                ValueRef n_bytes) -> result {
    // FIXME: switch to the 64-bit variant when on such a platform.
    // TODO: Provide LLVM with better alignment information when the alignment
    // is statically known (it must be nothing more than a constant int, or
    // LLVM complains -- not even a constant element of a tydesc works).

    auto i = cx.fcx.lcx.ccx.intrinsics;
    assert (i.contains_key("llvm.memmove.p0i8.p0i8.i32"));
    auto memmove = i.get("llvm.memmove.p0i8.p0i8.i32");
    auto src_ptr = cx.build.PointerCast(src, T_ptr(T_i8()));
    auto dst_ptr = cx.build.PointerCast(dst, T_ptr(T_i8()));
    auto size = cx.build.IntCast(n_bytes, T_i32());
    auto align = C_int(0);
    auto volatile = C_bool(false);
    ret rslt(cx,
            cx.build.Call(memmove,
                          ~[dst_ptr, src_ptr, size, align, volatile]));
}

fn call_bzero(&@block_ctxt cx, ValueRef dst, ValueRef n_bytes,
              ValueRef align_bytes) -> result {
    // FIXME: switch to the 64-bit variant when on such a platform.

    auto i = cx.fcx.lcx.ccx.intrinsics;
    assert (i.contains_key("llvm.memset.p0i8.i32"));
    auto memset = i.get("llvm.memset.p0i8.i32");
    auto dst_ptr = cx.build.PointerCast(dst, T_ptr(T_i8()));
    auto size = cx.build.IntCast(n_bytes, T_i32());
    auto align =
        if (lib::llvm::llvm::LLVMIsConstant(align_bytes) == True) {
            cx.build.IntCast(align_bytes, T_i32())
        } else { cx.build.IntCast(C_int(0), T_i32()) };
    auto volatile = C_bool(false);
    ret rslt(cx,
            cx.build.Call(memset,
                          ~[dst_ptr, C_u8(0u), size, align, volatile]));
}

fn memmove_ty(&@block_ctxt cx, ValueRef dst, ValueRef src, &ty::t t) ->
   result {
    if (ty::type_has_dynamic_size(cx.fcx.lcx.ccx.tcx, t)) {
        auto llsz = size_of(cx, t);
        ret call_memmove(llsz.bcx, dst, src, llsz.val);
    } else {
        ret rslt(cx, cx.build.Store(cx.build.Load(src), dst));
    }
}

// Duplicates any heap-owned memory owned by a value of the given type.
fn duplicate_heap_parts_if_necessary(&@block_ctxt cx, ValueRef vptr,
                                     ty::t typ) -> result {
    alt (ty::struct(cx.fcx.lcx.ccx.tcx, typ)) {
      case (ty::ty_ivec(?tm)) {
        ret ivec::duplicate_heap_part(cx, vptr, tm.ty);
      }
      case (ty::ty_istr) {
        ret ivec::duplicate_heap_part(cx, vptr,
            ty::mk_mach(cx.fcx.lcx.ccx.tcx, ast::ty_u8));
      }
      case (_) { ret rslt(cx, C_nil()); }
    }
}

tag copy_action { INIT; DROP_EXISTING; }

fn copy_val(&@block_ctxt cx, copy_action action, ValueRef dst, ValueRef src,
            &ty::t t) -> result {
    auto ccx = cx.fcx.lcx.ccx;
    // FIXME this is just a clunky stopgap. we should do proper checking in an
    // earlier pass.
    if (!ty::type_is_copyable(ccx.tcx, t)) {
        ccx.sess.span_fatal(cx.sp, "Copying a non-copyable type.");
    }

    if (ty::type_is_scalar(ccx.tcx, t) ||
            ty::type_is_native(ccx.tcx, t)) {
        ret rslt(cx, cx.build.Store(src, dst));
    } else if (ty::type_is_nil(ccx.tcx, t) ||
                   ty::type_is_bot(ccx.tcx, t)) {
        ret rslt(cx, C_nil());
    } else if (ty::type_is_boxed(ccx.tcx, t)) {
        auto bcx;
        if (action == DROP_EXISTING) {
            bcx = drop_ty(cx, cx.build.Load(dst), t).bcx;
        } else {
            bcx = cx;
        }
        bcx = copy_ty(bcx, src, t).bcx;
        ret rslt(bcx, bcx.build.Store(src, dst));
    } else if (ty::type_is_structural(ccx.tcx, t) ||
                   ty::type_has_dynamic_size(ccx.tcx, t)) {
        // Check for self-assignment.
        auto do_copy_cx = new_sub_block_ctxt(cx, "do_copy");
        auto next_cx = new_sub_block_ctxt(cx, "next");
        auto self_assigning = cx.build.ICmp(lib::llvm::LLVMIntNE,
            cx.build.PointerCast(dst, val_ty(src)), src);
        cx.build.CondBr(self_assigning, do_copy_cx.llbb, next_cx.llbb);

        if (action == DROP_EXISTING) {
            do_copy_cx = drop_ty(do_copy_cx, dst, t).bcx;
        }
        do_copy_cx = memmove_ty(do_copy_cx, dst, src, t).bcx;
        do_copy_cx = copy_ty(do_copy_cx, dst, t).bcx;
        do_copy_cx.build.Br(next_cx.llbb);

        ret rslt(next_cx, C_nil());
    }
    ccx.sess.bug("unexpected type in trans::copy_val: " +
                 ty_to_str(ccx.tcx, t));
}


// This works like copy_val, except that it deinitializes the source.
// Since it needs to zero out the source, src also needs to be an lval.
// FIXME: We always zero out the source. Ideally we would detect the
// case where a variable is always deinitialized by block exit and thus
// doesn't need to be dropped.
fn move_val(@block_ctxt cx, copy_action action, ValueRef dst,
            &lval_result src, &ty::t t) -> result {
    auto src_val = src.res.val;
    if (ty::type_is_scalar(cx.fcx.lcx.ccx.tcx, t) ||
        ty::type_is_native(cx.fcx.lcx.ccx.tcx, t)) {
        if (src.is_mem) { src_val = cx.build.Load(src_val); }
        cx.build.Store(src_val, dst);
        ret rslt(cx, C_nil());
    } else if (ty::type_is_nil(cx.fcx.lcx.ccx.tcx, t) ||
               ty::type_is_bot(cx.fcx.lcx.ccx.tcx, t)) {
        ret rslt(cx, C_nil());
    } else if (ty::type_is_boxed(cx.fcx.lcx.ccx.tcx, t)) {
        if (src.is_mem) { src_val = cx.build.Load(src_val); }
        if (action == DROP_EXISTING) {
            cx = drop_ty(cx, cx.build.Load(dst), t).bcx;
        }
        cx.build.Store(src_val, dst);
        if (src.is_mem) {
            ret zero_alloca(cx, src.res.val, t);
        } else { // It must be a temporary
            revoke_clean(cx, src_val);
            ret rslt(cx, C_nil());
        }
    } else if (ty::type_is_structural(cx.fcx.lcx.ccx.tcx, t) ||
               ty::type_has_dynamic_size(cx.fcx.lcx.ccx.tcx, t)) {
        if (action == DROP_EXISTING) { cx = drop_ty(cx, dst, t).bcx; }
        cx = memmove_ty(cx, dst, src_val, t).bcx;
        if (src.is_mem) {
            ret zero_alloca(cx, src_val, t);
        } else { // Temporary value
            revoke_clean(cx, src_val);
            ret rslt(cx, C_nil());
        }
    }
    cx.fcx.lcx.ccx.sess.bug("unexpected type in trans::move_val: " +
                            ty_to_str(cx.fcx.lcx.ccx.tcx, t));
}

fn move_val_if_temp(@block_ctxt cx, copy_action action, ValueRef dst,
                    &lval_result src, &ty::t t) -> result {
    // Lvals in memory are not temporaries. Copy them.
    if (src.is_mem) {
        ret copy_val(cx, action, dst,
                     load_if_immediate(cx, src.res.val, t), t);
    } else {
        ret move_val(cx, action, dst, src, t);
    }
}

fn trans_lit_istr(&@block_ctxt cx, str s) -> result {
    auto llstackpart = alloca(cx, T_ivec(T_i8()));
    auto len = str::byte_len(s);

    auto bcx;
    if (len < 3u) {     // 3 because of the \0
        cx.build.Store(C_uint(len + 1u),
                       cx.build.InBoundsGEP(llstackpart,
                                            ~[C_int(0), C_int(0)]));
        cx.build.Store(C_int(4),
                       cx.build.InBoundsGEP(llstackpart,
                                            ~[C_int(0), C_int(1)]));
        auto i = 0u;
        while (i < len) {
            cx.build.Store(C_u8(s.(i) as uint),
                           cx.build.InBoundsGEP(llstackpart,
                                                ~[C_int(0), C_int(2),
                                                  C_uint(i)]));
            i += 1u;
        }
        cx.build.Store(C_u8(0u),
                       cx.build.InBoundsGEP(llstackpart,
                                            ~[C_int(0), C_int(2),
                                              C_uint(len)]));

        bcx = cx;
    } else {
        auto r =
            trans_shared_malloc(cx, T_ptr(T_ivec_heap_part(T_i8())),
                                llsize_of(T_struct(~[T_int(),
                                                     T_array(T_i8(),
                                                             len + 1u)])));
        bcx = r.bcx;
        auto llheappart = r.val;

        bcx.build.Store(C_uint(len + 1u),
                        bcx.build.InBoundsGEP(llheappart,
                                              ~[C_int(0), C_int(0)]));
        bcx.build.Store(llvm::LLVMConstString(str::buf(s), len, False),
                        bcx.build.InBoundsGEP(llheappart,
                                              ~[C_int(0), C_int(1)]));

        auto llspilledstackpart = bcx.build.PointerCast(llstackpart,
            T_ptr(T_ivec_heap(T_i8())));
        bcx.build.Store(C_int(0),
                        bcx.build.InBoundsGEP(llspilledstackpart,
                                              ~[C_int(0), C_int(0)]));
        bcx.build.Store(C_uint(len + 1u),
                        bcx.build.InBoundsGEP(llspilledstackpart,
                                              ~[C_int(0), C_int(1)]));
        bcx.build.Store(llheappart,
                        bcx.build.InBoundsGEP(llspilledstackpart,
                                              ~[C_int(0), C_int(2)]));
    }

    ret rslt(bcx, llstackpart);
}

fn trans_crate_lit(&@crate_ctxt cx, &ast::lit lit) -> ValueRef {
    alt (lit.node) {
        case (ast::lit_int(?i)) { ret C_int(i); }
        case (ast::lit_uint(?u)) { ret C_int(u as int); }
        case (ast::lit_mach_int(?tm, ?i)) {
            // FIXME: the entire handling of mach types falls apart
            // if target int width is larger than host, at the moment;
            // re-do the mach-int types using 'big' when that works.

            auto t = T_int();
            auto s = True;
            alt (tm) {
                case (ast::ty_u8) { t = T_i8(); s = False; }
                case (ast::ty_u16) { t = T_i16(); s = False; }
                case (ast::ty_u32) { t = T_i32(); s = False; }
                case (ast::ty_u64) { t = T_i64(); s = False; }
                case (ast::ty_i8) { t = T_i8(); }
                case (ast::ty_i16) { t = T_i16(); }
                case (ast::ty_i32) { t = T_i32(); }
                case (ast::ty_i64) { t = T_i64(); }
            }
            ret C_integral(t, i as uint, s);
        }
        case (ast::lit_float(?fs)) { ret C_float(fs); }
        case (ast::lit_mach_float(?tm, ?s)) {
            auto t = T_float();
            alt (tm) {
                case (ast::ty_f32) { t = T_f32(); }
                case (ast::ty_f64) { t = T_f64(); }
            }
            ret C_floating(s, t);
        }
        case (ast::lit_char(?c)) {
            ret C_integral(T_char(), c as uint, False);
        }
        case (ast::lit_bool(?b)) { ret C_bool(b); }
        case (ast::lit_nil) { ret C_nil(); }
        case (ast::lit_str(?s, ast::sk_rc)) { ret C_str(cx, s); }
        case (ast::lit_str(?s, ast::sk_unique)) {
            cx.sess.span_unimpl(lit.span, "unique string in this context");
        }
    }
}

fn trans_lit(&@block_ctxt cx, &ast::lit lit) -> result {
    alt (lit.node) {
      ast::lit_str(?s, ast::sk_unique) { ret trans_lit_istr(cx, s); }
      _ { ret rslt(cx, trans_crate_lit(cx.fcx.lcx.ccx, lit)); }
    }
}


// Converts an annotation to a type
fn node_id_type(&@crate_ctxt cx, ast::node_id id) -> ty::t {
    ret ty::node_id_to_monotype(cx.tcx, id);
}

fn node_type(&@crate_ctxt cx, &span sp, ast::node_id id) -> TypeRef {
    ret type_of(cx, sp, node_id_type(cx, id));
}

fn trans_unary(&@block_ctxt cx, ast::unop op, &@ast::expr e,
               ast::node_id id) -> result {
    auto e_ty = ty::expr_ty(cx.fcx.lcx.ccx.tcx, e);
    alt (op) {
        case (ast::not) {
            auto sub = trans_expr(cx, e);
            auto dr = autoderef(sub.bcx, sub.val,
                                ty::expr_ty(cx.fcx.lcx.ccx.tcx, e));
            ret rslt(dr.bcx, dr.bcx.build.Not(dr.val));
        }
        case (ast::neg) {
            auto sub = trans_expr(cx, e);
            auto dr = autoderef(sub.bcx, sub.val,
                                ty::expr_ty(cx.fcx.lcx.ccx.tcx, e));
            if (ty::struct(cx.fcx.lcx.ccx.tcx, e_ty) == ty::ty_float) {
                ret rslt(dr.bcx, dr.bcx.build.FNeg(dr.val));
            } else { ret rslt(dr.bcx, sub.bcx.build.Neg(dr.val)); }
        }
        case (ast::box(_)) {
            auto lv = trans_lval(cx, e);
            auto box_ty = node_id_type(lv.res.bcx.fcx.lcx.ccx, id);
            auto sub = trans_malloc_boxed(lv.res.bcx, e_ty);
            add_clean_temp(cx, sub.val, box_ty);
            auto box = sub.val;
            auto rc = sub.bcx.build.GEP
                (box, ~[C_int(0), C_int(abi::box_rc_field_refcnt)]);
            auto body = sub.bcx.build.GEP
                (box, ~[C_int(0), C_int(abi::box_rc_field_body)]);
            sub.bcx.build.Store(C_int(1), rc);
            // Cast the body type to the type of the value. This is needed to
            // make tags work, since tags have a different LLVM type depending
            // on whether they're boxed or not.

            if (!ty::type_has_dynamic_size(cx.fcx.lcx.ccx.tcx, e_ty)) {
                auto llety =
                    T_ptr(type_of(sub.bcx.fcx.lcx.ccx, e.span, e_ty));
                body = sub.bcx.build.PointerCast(body, llety);
            }
            sub = move_val_if_temp(sub.bcx, INIT, body, lv, e_ty);
            ret rslt(sub.bcx, box);
        }
        case (ast::deref) {
            cx.fcx.lcx.ccx.sess.bug("deref expressions should have been " +
                                        "translated using trans_lval(), not "
                                        + "trans_unary()");
        }
    }
}

fn trans_compare(&@block_ctxt cx0, ast::binop op, &ty::t t0, ValueRef lhs0,
                 ValueRef rhs0) -> result {
    // Autoderef both sides.

    auto cx = cx0;
    auto lhs_r = autoderef(cx, lhs0, t0);
    auto lhs = lhs_r.val;
    cx = lhs_r.bcx;
    auto rhs_r = autoderef(cx, rhs0, t0);
    auto rhs = rhs_r.val;
    cx = rhs_r.bcx;
    // Determine the operation we need.
    // FIXME: Use or-patterns when we have them.

    auto llop;
    alt (op) {
        case (ast::eq) { llop = C_u8(abi::cmp_glue_op_eq); }
        case (ast::lt) { llop = C_u8(abi::cmp_glue_op_lt); }
        case (ast::le) { llop = C_u8(abi::cmp_glue_op_le); }
        case (ast::ne) { llop = C_u8(abi::cmp_glue_op_eq); }
        case (ast::ge) { llop = C_u8(abi::cmp_glue_op_lt); }
        case (ast::gt) { llop = C_u8(abi::cmp_glue_op_le); }
    }
    auto rs = compare(cx, lhs, rhs, rhs_r.ty, llop);

    // Invert the result if necessary.
    // FIXME: Use or-patterns when we have them.
    alt (op) {
        case (ast::eq) { ret rslt(rs.bcx, rs.val); }
        case (ast::lt) { ret rslt(rs.bcx, rs.val); }
        case (ast::le) { ret rslt(rs.bcx, rs.val); }
        case (ast::ne) { ret rslt(rs.bcx, rs.bcx.build.Not(rs.val)); }
        case (ast::ge) { ret rslt(rs.bcx, rs.bcx.build.Not(rs.val)); }
        case (ast::gt) { ret rslt(rs.bcx, rs.bcx.build.Not(rs.val)); }
    }
}

fn trans_vec_append(&@block_ctxt cx, &ty::t t, ValueRef lhs, ValueRef rhs) ->
   result {
    auto elt_ty = ty::sequence_element_type(cx.fcx.lcx.ccx.tcx, t);
    auto skip_null = C_bool(false);
    alt (ty::struct(cx.fcx.lcx.ccx.tcx, t)) {
        case (ty::ty_str) { skip_null = C_bool(true); }
        case (_) { }
    }
    auto bcx = cx;
    auto ti = none[@tydesc_info];
    auto llvec_tydesc = get_tydesc(bcx, t, false, ti);
    bcx = llvec_tydesc.bcx;
    ti = none[@tydesc_info];
    auto llelt_tydesc = get_tydesc(bcx, elt_ty, false, ti);
    lazily_emit_tydesc_glue(cx, abi::tydesc_field_copy_glue, ti);
    lazily_emit_tydesc_glue(cx, abi::tydesc_field_drop_glue, ti);
    lazily_emit_tydesc_glue(cx, abi::tydesc_field_free_glue, ti);
    bcx = llelt_tydesc.bcx;
    auto dst = bcx.build.PointerCast(lhs, T_ptr(T_opaque_vec_ptr()));
    auto src = bcx.build.PointerCast(rhs, T_opaque_vec_ptr());
    ret rslt(bcx,
            bcx.build.Call(cx.fcx.lcx.ccx.upcalls.vec_append,
                           ~[cx.fcx.lltaskptr, llvec_tydesc.val,
                             llelt_tydesc.val, dst, src, skip_null]));
}

mod ivec {

    // Returns the length of an interior vector and a pointer to its first
    // element, in that order.
    fn get_len_and_data(&@block_ctxt bcx, ValueRef orig_v, ty::t unit_ty)
            -> tup(ValueRef, ValueRef, @block_ctxt) {
        // If this interior vector has dynamic size, we can't assume anything
        // about the LLVM type of the value passed in, so we cast it to an
        // opaque vector type.
        auto v;
        if (ty::type_has_dynamic_size(bcx.fcx.lcx.ccx.tcx, unit_ty)) {
            v = bcx.build.PointerCast(orig_v, T_ptr(T_opaque_ivec()));
        } else {
            v = orig_v;
        }

        auto llunitty = type_of_or_i8(bcx, unit_ty);
        auto stack_len = load_inbounds(bcx, v, ~[C_int(0),
                                                 C_uint(abi::ivec_elt_len)]);
        auto stack_elem =
            bcx.build.InBoundsGEP(v,
                                  ~[C_int(0), C_uint(abi::ivec_elt_elems),
                                    C_int(0)]);
        auto on_heap =
            bcx.build.ICmp(lib::llvm::LLVMIntEQ, stack_len, C_int(0));
        auto on_heap_cx = new_sub_block_ctxt(bcx, "on_heap");
        auto next_cx = new_sub_block_ctxt(bcx, "next");
        bcx.build.CondBr(on_heap, on_heap_cx.llbb, next_cx.llbb);
        auto heap_stub =
            on_heap_cx.build.PointerCast(v, T_ptr(T_ivec_heap(llunitty)));
        auto heap_ptr = load_inbounds(on_heap_cx, heap_stub,
                                      ~[C_int(0),
                                        C_uint(abi::ivec_heap_stub_elt_ptr)]);

        // Check whether the heap pointer is null. If it is, the vector length
        // is truly zero.

        auto llstubty = T_ivec_heap(llunitty);
        auto llheapptrty = struct_elt(llstubty, abi::ivec_heap_stub_elt_ptr);
        auto heap_ptr_is_null =
            on_heap_cx.build.ICmp(lib::llvm::LLVMIntEQ, heap_ptr,
                                  C_null(T_ptr(llheapptrty)));
        auto zero_len_cx = new_sub_block_ctxt(bcx, "zero_len");
        auto nonzero_len_cx = new_sub_block_ctxt(bcx, "nonzero_len");
        on_heap_cx.build.CondBr(heap_ptr_is_null, zero_len_cx.llbb,
                                nonzero_len_cx.llbb);
        // Technically this context is unnecessary, but it makes this function
        // clearer.

        auto zero_len = C_int(0);
        auto zero_elem = C_null(T_ptr(llunitty));
        zero_len_cx.build.Br(next_cx.llbb);
        // If we're here, then we actually have a heapified vector.

        auto heap_len = load_inbounds(nonzero_len_cx, heap_ptr,
                                      ~[C_int(0),
                                        C_uint(abi::ivec_heap_elt_len)]);
        auto heap_elem =
            {
                auto v = ~[C_int(0), C_uint(abi::ivec_heap_elt_elems),
                           C_int(0)];
                nonzero_len_cx.build.InBoundsGEP(heap_ptr,v)
            };

        nonzero_len_cx.build.Br(next_cx.llbb);
        // Now we can figure out the length of `v` and get a pointer to its
        // first element.

        auto len =
            next_cx.build.Phi(T_int(), ~[stack_len, zero_len, heap_len],
                              ~[bcx.llbb, zero_len_cx.llbb,
                                nonzero_len_cx.llbb]);
        auto elem =
            next_cx.build.Phi(T_ptr(llunitty),
                              ~[stack_elem, zero_elem, heap_elem],
                              ~[bcx.llbb, zero_len_cx.llbb,
                                nonzero_len_cx.llbb]);
        ret tup(len, elem, next_cx);
    }

    // Returns a tuple consisting of a pointer to the newly-reserved space and
    // a block context. Updates the length appropriately.
    fn reserve_space(&@block_ctxt cx, TypeRef llunitty, ValueRef v,
                     ValueRef len_needed) -> result {
        auto stack_len_ptr =
            cx.build.InBoundsGEP(v, ~[C_int(0), C_uint(abi::ivec_elt_len)]);
        auto stack_len = cx.build.Load(stack_len_ptr);
        auto alen = load_inbounds(cx, v, ~[C_int(0),
                                           C_uint(abi::ivec_elt_alen)]);
        // There are four cases we have to consider:
        // (1) On heap, no resize necessary.
        // (2) On heap, need to resize.
        // (3) On stack, no resize necessary.
        // (4) On stack, need to spill to heap.

        auto maybe_on_heap =
            cx.build.ICmp(lib::llvm::LLVMIntEQ, stack_len, C_int(0));
        auto maybe_on_heap_cx = new_sub_block_ctxt(cx, "maybe_on_heap");
        auto on_stack_cx = new_sub_block_ctxt(cx, "on_stack");
        cx.build.CondBr(maybe_on_heap, maybe_on_heap_cx.llbb,
                        on_stack_cx.llbb);
        auto next_cx = new_sub_block_ctxt(cx, "next");
        // We're possibly on the heap, unless the vector is zero-length.

        auto stub_p = ~[C_int(0), C_uint(abi::ivec_heap_stub_elt_ptr)];
        auto stub_ptr =
            maybe_on_heap_cx.build.PointerCast(v,
                                               T_ptr(T_ivec_heap(llunitty)));
        auto heap_ptr = load_inbounds(maybe_on_heap_cx, stub_ptr, stub_p);
        auto on_heap =
            maybe_on_heap_cx.build.ICmp(lib::llvm::LLVMIntNE, heap_ptr,
                                        C_null(val_ty(heap_ptr)));
        auto on_heap_cx = new_sub_block_ctxt(cx, "on_heap");
        maybe_on_heap_cx.build.CondBr(on_heap, on_heap_cx.llbb,
                                      on_stack_cx.llbb);
        // We're definitely on the heap. Check whether we need to resize.

        auto heap_len_ptr =
            on_heap_cx.build.InBoundsGEP(heap_ptr,
                                         ~[C_int(0),
                                           C_uint(abi::ivec_heap_elt_len)]);
        auto heap_len = on_heap_cx.build.Load(heap_len_ptr);
        auto new_heap_len = on_heap_cx.build.Add(heap_len, len_needed);
        auto heap_len_unscaled =
            on_heap_cx.build.UDiv(heap_len, llsize_of(llunitty));
        auto heap_no_resize_needed =
            on_heap_cx.build.ICmp(lib::llvm::LLVMIntULE, new_heap_len, alen);
        auto heap_no_resize_cx = new_sub_block_ctxt(cx, "heap_no_resize");
        auto heap_resize_cx = new_sub_block_ctxt(cx, "heap_resize");
        on_heap_cx.build.CondBr(heap_no_resize_needed, heap_no_resize_cx.llbb,
                                heap_resize_cx.llbb);
        // Case (1): We're on the heap and don't need to resize.

        auto heap_data_no_resize =
            {
                auto v = ~[C_int(0), C_uint(abi::ivec_heap_elt_elems),
                           heap_len_unscaled];
                heap_no_resize_cx.build.InBoundsGEP(heap_ptr,v)
            };
        heap_no_resize_cx.build.Store(new_heap_len, heap_len_ptr);
        heap_no_resize_cx.build.Br(next_cx.llbb);
        // Case (2): We're on the heap and need to resize. This path is rare,
        // so we delegate to cold glue.

        {
            auto p =
                heap_resize_cx.build.PointerCast(v, T_ptr(T_opaque_ivec()));
            auto upcall = cx.fcx.lcx.ccx.upcalls.ivec_resize_shared;
            heap_resize_cx.build.Call(upcall,
                                      ~[cx.fcx.lltaskptr, p, new_heap_len]);
        }
        auto heap_ptr_resize =
            load_inbounds(heap_resize_cx, stub_ptr, stub_p);

        auto heap_data_resize =
            {
                auto v = ~[C_int(0), C_uint(abi::ivec_heap_elt_elems),
                           heap_len_unscaled];
                heap_resize_cx.build.InBoundsGEP(heap_ptr_resize, v)
            };
        heap_resize_cx.build.Br(next_cx.llbb);
        // We're on the stack. Check whether we need to spill to the heap.

        auto new_stack_len = on_stack_cx.build.Add(stack_len, len_needed);
        auto stack_no_spill_needed =
            on_stack_cx.build.ICmp(lib::llvm::LLVMIntULE, new_stack_len,
                                   alen);
        auto stack_len_unscaled =
            on_stack_cx.build.UDiv(stack_len, llsize_of(llunitty));
        auto stack_no_spill_cx = new_sub_block_ctxt(cx, "stack_no_spill");
        auto stack_spill_cx = new_sub_block_ctxt(cx, "stack_spill");
        on_stack_cx.build.CondBr(stack_no_spill_needed,
                                 stack_no_spill_cx.llbb, stack_spill_cx.llbb);
        // Case (3): We're on the stack and don't need to spill.

        auto stack_data_no_spill =
            stack_no_spill_cx.build.InBoundsGEP(v,
                                                ~[C_int(0),
                                                  C_uint(abi::ivec_elt_elems),
                                                  stack_len_unscaled]);
        stack_no_spill_cx.build.Store(new_stack_len, stack_len_ptr);
        stack_no_spill_cx.build.Br(next_cx.llbb);
        // Case (4): We're on the stack and need to spill. Like case (2), this
        // path is rare, so we delegate to cold glue.

        {
            auto p =
                stack_spill_cx.build.PointerCast(v, T_ptr(T_opaque_ivec()));
            auto upcall = cx.fcx.lcx.ccx.upcalls.ivec_spill_shared;
            stack_spill_cx.build.Call(upcall,
                                      ~[cx.fcx.lltaskptr, p, new_stack_len]);
        }
        auto spill_stub =
            stack_spill_cx.build.PointerCast(v, T_ptr(T_ivec_heap(llunitty)));

        auto heap_ptr_spill =
            load_inbounds(stack_spill_cx, spill_stub, stub_p);

        auto heap_data_spill =
            {
                auto v = ~[C_int(0), C_uint(abi::ivec_heap_elt_elems),
                          stack_len_unscaled];
                stack_spill_cx.build.InBoundsGEP(heap_ptr_spill, v)
            };
        stack_spill_cx.build.Br(next_cx.llbb);
        // Phi together the different data pointers to get the result.

        auto data_ptr =
            next_cx.build.Phi(T_ptr(llunitty),
                              ~[heap_data_no_resize, heap_data_resize,
                                stack_data_no_spill, heap_data_spill],
                              ~[heap_no_resize_cx.llbb, heap_resize_cx.llbb,
                                stack_no_spill_cx.llbb, stack_spill_cx.llbb]);
        ret rslt(next_cx, data_ptr);
    }
    fn trans_append(&@block_ctxt cx, &ty::t t, ValueRef orig_lhs,
                    ValueRef orig_rhs) -> result {
        // Cast to opaque interior vector types if necessary.
        auto lhs;
        auto rhs;
        if (ty::type_has_dynamic_size(cx.fcx.lcx.ccx.tcx, t)) {
            lhs = cx.build.PointerCast(orig_lhs, T_ptr(T_opaque_ivec()));
            rhs = cx.build.PointerCast(orig_rhs, T_ptr(T_opaque_ivec()));
        } else {
            lhs = orig_lhs;
            rhs = orig_rhs;
        }

        auto unit_ty = ty::sequence_element_type(cx.fcx.lcx.ccx.tcx, t);
        auto llunitty = type_of_or_i8(cx, unit_ty);
        alt (ty::struct(cx.fcx.lcx.ccx.tcx, t)) {
            case (ty::ty_istr) {  }
            case (ty::ty_ivec(_)) {  }
            case (_) {
                cx.fcx.lcx.ccx.tcx.sess.bug("non-istr/ivec in trans_append");
            }
        }

        auto rs = size_of(cx, unit_ty);
        auto bcx = rs.bcx;
        auto unit_sz = rs.val;

        // Gather the various type descriptors we'll need.

        // FIXME (issue #511): This is needed to prevent a leak.
        auto no_tydesc_info = none;

        rs = get_tydesc(bcx, t, false, no_tydesc_info);
        bcx = rs.bcx;
        rs = get_tydesc(bcx, unit_ty, false, no_tydesc_info);
        bcx = rs.bcx;
        lazily_emit_tydesc_glue(bcx, abi::tydesc_field_copy_glue, none);
        lazily_emit_tydesc_glue(bcx, abi::tydesc_field_drop_glue, none);
        lazily_emit_tydesc_glue(bcx, abi::tydesc_field_free_glue, none);
        auto rhs_len_and_data = get_len_and_data(bcx, rhs, unit_ty);
        auto rhs_len = rhs_len_and_data._0;
        auto rhs_data = rhs_len_and_data._1;
        bcx = rhs_len_and_data._2;
        rs = reserve_space(bcx, llunitty, lhs, rhs_len);
        auto lhs_data = rs.val;
        bcx = rs.bcx;
        // Work out the end pointer.

        auto lhs_unscaled_idx = bcx.build.UDiv(rhs_len, llsize_of(llunitty));
        auto lhs_end = bcx.build.InBoundsGEP(lhs_data, ~[lhs_unscaled_idx]);
        // Now emit the copy loop.

        auto dest_ptr = alloca(bcx, T_ptr(llunitty));
        bcx.build.Store(lhs_data, dest_ptr);
        auto src_ptr = alloca(bcx, T_ptr(llunitty));
        bcx.build.Store(rhs_data, src_ptr);
        auto copy_loop_header_cx =
            new_sub_block_ctxt(bcx, "copy_loop_header");
        bcx.build.Br(copy_loop_header_cx.llbb);
        auto copy_dest_ptr = copy_loop_header_cx.build.Load(dest_ptr);
        auto not_yet_at_end =
            copy_loop_header_cx.build.ICmp(lib::llvm::LLVMIntNE,
                                           copy_dest_ptr, lhs_end);
        auto copy_loop_body_cx = new_sub_block_ctxt(bcx, "copy_loop_body");
        auto next_cx = new_sub_block_ctxt(bcx, "next");
        copy_loop_header_cx.build.CondBr(not_yet_at_end,
                                         copy_loop_body_cx.llbb,
                                         next_cx.llbb);

        auto copy_src_ptr = copy_loop_body_cx.build.Load(src_ptr);
        auto copy_src = load_if_immediate(copy_loop_body_cx, copy_src_ptr,
                                          unit_ty);

        rs = copy_val(copy_loop_body_cx, INIT, copy_dest_ptr, copy_src,
                      unit_ty);
        auto post_copy_cx = rs.bcx;
        // Increment both pointers.
        if (ty::type_has_dynamic_size(cx.fcx.lcx.ccx.tcx, t)) {
            // We have to increment by the dynamically-computed size.
            incr_ptr(post_copy_cx, copy_dest_ptr, unit_sz, dest_ptr);
            incr_ptr(post_copy_cx, copy_src_ptr, unit_sz, src_ptr);
        } else {
            incr_ptr(post_copy_cx, copy_dest_ptr, C_int(1), dest_ptr);
            incr_ptr(post_copy_cx, copy_src_ptr, C_int(1), src_ptr);
        }

        post_copy_cx.build.Br(copy_loop_header_cx.llbb);
        ret rslt(next_cx, C_nil());
    }

    type alloc_result = rec(@block_ctxt bcx,
                            ValueRef llptr,
                            ValueRef llunitsz,
                            ValueRef llalen);

    fn alloc(&@block_ctxt cx, ty::t unit_ty) -> alloc_result {
        auto dynamic = ty::type_has_dynamic_size(cx.fcx.lcx.ccx.tcx, unit_ty);

        auto bcx;
        if (dynamic) {
            bcx = llderivedtydescs_block_ctxt(cx.fcx);
        } else {
            bcx = cx;
        }

        auto llunitsz;
        auto rslt = size_of(bcx, unit_ty);
        bcx = rslt.bcx;
        llunitsz = rslt.val;

        if (dynamic) { cx.fcx.llderivedtydescs = bcx.llbb; }

        auto llalen = bcx.build.Mul(llunitsz,
                                    C_uint(abi::ivec_default_length));

        auto llptr;
        auto llunitty = type_of_or_i8(bcx, unit_ty);
        auto bcx_result;
        if (dynamic) {
            auto llarraysz = bcx.build.Add(llsize_of(T_opaque_ivec()),
                                           llalen);
            auto llvecptr = array_alloca(bcx, T_i8(), llarraysz);

            bcx_result = cx;
            llptr = bcx_result.build.PointerCast(llvecptr,
                                                 T_ptr(T_opaque_ivec()));
        } else {
            llptr = alloca(bcx, T_ivec(llunitty));
            bcx_result = bcx;
        }

        ret rec(bcx=bcx_result,
                llptr=llptr,
                llunitsz=llunitsz,
                llalen=llalen);
    }

    fn trans_add(&@block_ctxt cx, ty::t vec_ty, ValueRef lhs, ValueRef rhs)
            -> result {
        auto bcx = cx;
        auto unit_ty = ty::sequence_element_type(bcx.fcx.lcx.ccx.tcx, vec_ty);

        auto ares = alloc(bcx, unit_ty);
        bcx = ares.bcx;
        auto llvecptr = ares.llptr;
        auto unit_sz = ares.llunitsz;
        auto llalen = ares.llalen;

        add_clean_temp(bcx, llvecptr, vec_ty);

        auto llunitty = type_of_or_i8(bcx, unit_ty);
        auto llheappartty = T_ivec_heap_part(llunitty);
        auto lhs_len_and_data = get_len_and_data(bcx, lhs, unit_ty);
        auto lhs_len = lhs_len_and_data._0;
        auto lhs_data = lhs_len_and_data._1;
        bcx = lhs_len_and_data._2;
        auto rhs_len_and_data = get_len_and_data(bcx, rhs, unit_ty);
        auto rhs_len = rhs_len_and_data._0;
        auto rhs_data = rhs_len_and_data._1;
        bcx = rhs_len_and_data._2;
        auto lllen = bcx.build.Add(lhs_len, rhs_len);
        // We have three cases to handle here:
        // (1) Length is zero ([] + []).
        // (2) Copy onto stack.
        // (3) Allocate on heap and copy there.

        auto len_is_zero =
            bcx.build.ICmp(lib::llvm::LLVMIntEQ, lllen, C_int(0));
        auto zero_len_cx = new_sub_block_ctxt(bcx, "zero_len");
        auto nonzero_len_cx = new_sub_block_ctxt(bcx, "nonzero_len");
        bcx.build.CondBr(len_is_zero, zero_len_cx.llbb, nonzero_len_cx.llbb);
        // Case (1): Length is zero.

        auto stub_z = ~[C_int(0), C_uint(abi::ivec_heap_stub_elt_zero)];
        auto stub_a = ~[C_int(0), C_uint(abi::ivec_heap_stub_elt_alen)];
        auto stub_p = ~[C_int(0), C_uint(abi::ivec_heap_stub_elt_ptr)];

        auto vec_l = ~[C_int(0), C_uint(abi::ivec_elt_len)];
        auto vec_a = ~[C_int(0), C_uint(abi::ivec_elt_alen)];

        auto stub_ptr_zero =
            zero_len_cx.build.PointerCast(llvecptr,
                                          T_ptr(T_ivec_heap(llunitty)));
        zero_len_cx.build.Store(C_int(0),
                                zero_len_cx.build.InBoundsGEP(stub_ptr_zero,
                                                              stub_z));
        zero_len_cx.build.Store(llalen,
                                zero_len_cx.build.InBoundsGEP(stub_ptr_zero,
                                                              stub_a));
        zero_len_cx.build.Store(C_null(T_ptr(llheappartty)),
                                zero_len_cx.build.InBoundsGEP(stub_ptr_zero,
                                                              stub_p));
        auto next_cx = new_sub_block_ctxt(bcx, "next");
        zero_len_cx.build.Br(next_cx.llbb);
        // Determine whether we need to spill to the heap.

        auto on_stack =
            nonzero_len_cx.build.ICmp(lib::llvm::LLVMIntULE, lllen, llalen);
        auto stack_cx = new_sub_block_ctxt(bcx, "stack");
        auto heap_cx = new_sub_block_ctxt(bcx, "heap");
        nonzero_len_cx.build.CondBr(on_stack, stack_cx.llbb, heap_cx.llbb);
        // Case (2): Copy onto stack.

        stack_cx.build.Store(lllen,
                             stack_cx.build.InBoundsGEP(llvecptr, vec_l));
        stack_cx.build.Store(llalen,
                             stack_cx.build.InBoundsGEP(llvecptr, vec_a));
        auto dest_ptr_stack =
            stack_cx.build.InBoundsGEP(llvecptr,
                                       ~[C_int(0),
                                         C_uint(abi::ivec_elt_elems),
                                         C_int(0)]);
        auto copy_cx = new_sub_block_ctxt(bcx, "copy");
        stack_cx.build.Br(copy_cx.llbb);
        // Case (3): Allocate on heap and copy there.

        auto stub_ptr_heap =
            heap_cx.build.PointerCast(llvecptr, T_ptr(T_ivec_heap(llunitty)));
        heap_cx.build.Store(C_int(0),
                            heap_cx.build.InBoundsGEP(stub_ptr_heap,
                                                      stub_z));
        heap_cx.build.Store(lllen,
                            heap_cx.build.InBoundsGEP(stub_ptr_heap,
                                                      stub_a));
        auto heap_sz = heap_cx.build.Add(llsize_of(llheappartty), lllen);
        auto rs = trans_shared_malloc(heap_cx, T_ptr(llheappartty), heap_sz);
        auto heap_part = rs.val;
        heap_cx = rs.bcx;
        heap_cx.build.Store(heap_part,
                            heap_cx.build.InBoundsGEP(stub_ptr_heap,
                                                      stub_p));
        {
            auto v = ~[C_int(0), C_uint(abi::ivec_heap_elt_len)];
            heap_cx.build.Store(lllen,
                                heap_cx.build.InBoundsGEP(heap_part,
                                                          v));
        }
        auto dest_ptr_heap =
            heap_cx.build.InBoundsGEP(heap_part,
                                      ~[C_int(0),
                                        C_uint(abi::ivec_heap_elt_elems),
                                        C_int(0)]);
        heap_cx.build.Br(copy_cx.llbb);
        // Emit the copy loop.

        auto first_dest_ptr =
            copy_cx.build.Phi(T_ptr(llunitty),
                              ~[dest_ptr_stack, dest_ptr_heap],
                              ~[stack_cx.llbb, heap_cx.llbb]);

        auto lhs_end_ptr; auto rhs_end_ptr;
        if (ty::type_has_dynamic_size(cx.fcx.lcx.ccx.tcx, unit_ty)) {
            lhs_end_ptr = copy_cx.build.InBoundsGEP(lhs_data, ~[lhs_len]);
            rhs_end_ptr = copy_cx.build.InBoundsGEP(rhs_data, ~[rhs_len]);
        } else {
            auto lhs_len_unscaled = copy_cx.build.UDiv(lhs_len, unit_sz);
            lhs_end_ptr = copy_cx.build.InBoundsGEP(lhs_data,
                                                    ~[lhs_len_unscaled]);
            auto rhs_len_unscaled = copy_cx.build.UDiv(rhs_len, unit_sz);
            rhs_end_ptr = copy_cx.build.InBoundsGEP(rhs_data,
                                                    ~[rhs_len_unscaled]);
        }

        auto dest_ptr_ptr = alloca(copy_cx, T_ptr(llunitty));
        copy_cx.build.Store(first_dest_ptr, dest_ptr_ptr);
        auto lhs_ptr_ptr = alloca(copy_cx, T_ptr(llunitty));
        copy_cx.build.Store(lhs_data, lhs_ptr_ptr);
        auto rhs_ptr_ptr = alloca(copy_cx, T_ptr(llunitty));
        copy_cx.build.Store(rhs_data, rhs_ptr_ptr);
        auto lhs_copy_cx = new_sub_block_ctxt(bcx, "lhs_copy");
        copy_cx.build.Br(lhs_copy_cx.llbb);
        // Copy in elements from the LHS.

        auto lhs_ptr = lhs_copy_cx.build.Load(lhs_ptr_ptr);
        auto not_at_end_lhs =
            lhs_copy_cx.build.ICmp(lib::llvm::LLVMIntNE, lhs_ptr,
                                   lhs_end_ptr);
        auto lhs_do_copy_cx = new_sub_block_ctxt(bcx, "lhs_do_copy");
        auto rhs_copy_cx = new_sub_block_ctxt(bcx, "rhs_copy");
        lhs_copy_cx.build.CondBr(not_at_end_lhs, lhs_do_copy_cx.llbb,
                                 rhs_copy_cx.llbb);
        auto dest_ptr_lhs_copy = lhs_do_copy_cx.build.Load(dest_ptr_ptr);
        auto lhs_val = load_if_immediate(lhs_do_copy_cx, lhs_ptr, unit_ty);
        rs = copy_val(lhs_do_copy_cx, INIT, dest_ptr_lhs_copy, lhs_val,
                      unit_ty);
        lhs_do_copy_cx = rs.bcx;

        // Increment both pointers.
        if (ty::type_has_dynamic_size(cx.fcx.lcx.ccx.tcx, unit_ty)) {
            // We have to increment by the dynamically-computed size.
            incr_ptr(lhs_do_copy_cx, dest_ptr_lhs_copy, unit_sz,
                     dest_ptr_ptr);
            incr_ptr(lhs_do_copy_cx, lhs_ptr, unit_sz, lhs_ptr_ptr);
        } else {
            incr_ptr(lhs_do_copy_cx, dest_ptr_lhs_copy, C_int(1),
                     dest_ptr_ptr);
            incr_ptr(lhs_do_copy_cx, lhs_ptr, C_int(1), lhs_ptr_ptr);
        }

        lhs_do_copy_cx.build.Br(lhs_copy_cx.llbb);
        // Copy in elements from the RHS.

        auto rhs_ptr = rhs_copy_cx.build.Load(rhs_ptr_ptr);
        auto not_at_end_rhs =
            rhs_copy_cx.build.ICmp(lib::llvm::LLVMIntNE, rhs_ptr,
                                   rhs_end_ptr);
        auto rhs_do_copy_cx = new_sub_block_ctxt(bcx, "rhs_do_copy");
        rhs_copy_cx.build.CondBr(not_at_end_rhs, rhs_do_copy_cx.llbb,
                                 next_cx.llbb);
        auto dest_ptr_rhs_copy = rhs_do_copy_cx.build.Load(dest_ptr_ptr);
        auto rhs_val = load_if_immediate(rhs_do_copy_cx, rhs_ptr, unit_ty);
        rs =
            copy_val(rhs_do_copy_cx, INIT, dest_ptr_rhs_copy, rhs_val,
                     unit_ty);
        rhs_do_copy_cx = rs.bcx;

        // Increment both pointers.
        if (ty::type_has_dynamic_size(cx.fcx.lcx.ccx.tcx, unit_ty)) {
            // We have to increment by the dynamically-computed size.
            incr_ptr(rhs_do_copy_cx, dest_ptr_rhs_copy, unit_sz,
                     dest_ptr_ptr);
            incr_ptr(rhs_do_copy_cx, rhs_ptr, unit_sz, rhs_ptr_ptr);
        } else {
            incr_ptr(rhs_do_copy_cx, dest_ptr_rhs_copy, C_int(1),
                     dest_ptr_ptr);
            incr_ptr(rhs_do_copy_cx, rhs_ptr, C_int(1), rhs_ptr_ptr);
        }

        rhs_do_copy_cx.build.Br(rhs_copy_cx.llbb);
        // Finally done!

        ret rslt(next_cx, llvecptr);
    }

    // NB: This does *not* adjust reference counts. The caller must have done
    // this via copy_ty() beforehand.
    fn duplicate_heap_part(&@block_ctxt cx, ValueRef orig_vptr,
                           ty::t unit_ty) -> result {
        // Cast to an opaque interior vector if we can't trust the pointer
        // type.
        auto vptr;
        if (ty::type_has_dynamic_size(cx.fcx.lcx.ccx.tcx, unit_ty)) {
            vptr = cx.build.PointerCast(orig_vptr, T_ptr(T_opaque_ivec()));
        } else {
            vptr = orig_vptr;
        }

        auto llunitty = type_of_or_i8(cx, unit_ty);
        auto llheappartty = T_ivec_heap_part(llunitty);

        // Check to see if the vector is heapified.
        auto stack_len_ptr = cx.build.InBoundsGEP(vptr, ~[C_int(0),
            C_uint(abi::ivec_elt_len)]);
        auto stack_len = cx.build.Load(stack_len_ptr);
        auto stack_len_is_zero = cx.build.ICmp(lib::llvm::LLVMIntEQ,
                                               stack_len, C_int(0));
        auto maybe_on_heap_cx = new_sub_block_ctxt(cx, "maybe_on_heap");
        auto next_cx = new_sub_block_ctxt(cx, "next");
        cx.build.CondBr(stack_len_is_zero, maybe_on_heap_cx.llbb,
                        next_cx.llbb);

        auto stub_ptr = maybe_on_heap_cx.build.PointerCast(vptr,
            T_ptr(T_ivec_heap(llunitty)));
        auto heap_ptr_ptr = maybe_on_heap_cx.build.InBoundsGEP(stub_ptr,
            ~[C_int(0), C_uint(abi::ivec_heap_stub_elt_ptr)]);
        auto heap_ptr = maybe_on_heap_cx.build.Load(heap_ptr_ptr);
        auto heap_ptr_is_nonnull = maybe_on_heap_cx.build.ICmp(
            lib::llvm::LLVMIntNE, heap_ptr, C_null(T_ptr(llheappartty)));
        auto on_heap_cx = new_sub_block_ctxt(cx, "on_heap");
        maybe_on_heap_cx.build.CondBr(heap_ptr_is_nonnull, on_heap_cx.llbb,
                                      next_cx.llbb);

        // Ok, the vector is on the heap. Copy the heap part.
        auto alen_ptr = on_heap_cx.build.InBoundsGEP(stub_ptr,
            ~[C_int(0), C_uint(abi::ivec_heap_stub_elt_alen)]);
        auto alen = on_heap_cx.build.Load(alen_ptr);

        auto heap_part_sz = on_heap_cx.build.Add(alen,
            llsize_of(T_opaque_ivec_heap_part()));
        auto rs = trans_shared_malloc(on_heap_cx, T_ptr(llheappartty),
                                      heap_part_sz);
        on_heap_cx = rs.bcx;
        auto new_heap_ptr = rs.val;

        rs = call_memmove(on_heap_cx, new_heap_ptr, heap_ptr, heap_part_sz);
        on_heap_cx = rs.bcx;

        on_heap_cx.build.Store(new_heap_ptr, heap_ptr_ptr);
        on_heap_cx.build.Br(next_cx.llbb);

        ret rslt(next_cx, C_nil());
    }
}

fn trans_vec_add(&@block_ctxt cx, &ty::t t, ValueRef lhs, ValueRef rhs) ->
   result {
    auto r = alloc_ty(cx, t);
    auto tmp = r.val;
    r = copy_val(r.bcx, INIT, tmp, lhs, t);
    auto bcx = trans_vec_append(r.bcx, t, tmp, rhs).bcx;
    tmp = load_if_immediate(bcx, tmp, t);
    add_clean_temp(cx, tmp, t);
    ret rslt(bcx, tmp);
}

fn trans_eager_binop(&@block_ctxt cx, ast::binop op, &ty::t intype,
                     ValueRef lhs, ValueRef rhs) -> result {
    auto is_float = false;
    alt (ty::struct(cx.fcx.lcx.ccx.tcx, intype)) {
        case (ty::ty_float) { is_float = true; }
        case (_) { is_float = false; }
    }
    alt (op) {
        case (ast::add) {
            if (ty::type_is_sequence(cx.fcx.lcx.ccx.tcx, intype)) {
                if (ty::sequence_is_interior(cx.fcx.lcx.ccx.tcx, intype)) {
                    ret ivec::trans_add(cx, intype, lhs, rhs);
                }
                ret trans_vec_add(cx, intype, lhs, rhs);
            }
            if (is_float) {
                ret rslt(cx, cx.build.FAdd(lhs, rhs));
            } else { ret rslt(cx, cx.build.Add(lhs, rhs)); }
        }
        case (ast::sub) {
            if (is_float) {
                ret rslt(cx, cx.build.FSub(lhs, rhs));
            } else { ret rslt(cx, cx.build.Sub(lhs, rhs)); }
        }
        case (ast::mul) {
            if (is_float) {
                ret rslt(cx, cx.build.FMul(lhs, rhs));
            } else { ret rslt(cx, cx.build.Mul(lhs, rhs)); }
        }
        case (ast::div) {
            if (is_float) { ret rslt(cx, cx.build.FDiv(lhs, rhs)); }
            if (ty::type_is_signed(cx.fcx.lcx.ccx.tcx, intype)) {
                ret rslt(cx, cx.build.SDiv(lhs, rhs));
            } else { ret rslt(cx, cx.build.UDiv(lhs, rhs)); }
        }
        case (ast::rem) {
            if (is_float) { ret rslt(cx, cx.build.FRem(lhs, rhs)); }
            if (ty::type_is_signed(cx.fcx.lcx.ccx.tcx, intype)) {
                ret rslt(cx, cx.build.SRem(lhs, rhs));
            } else { ret rslt(cx, cx.build.URem(lhs, rhs)); }
        }
        case (ast::bitor) { ret rslt(cx, cx.build.Or(lhs, rhs)); }
        case (ast::bitand) { ret rslt(cx, cx.build.And(lhs, rhs)); }
        case (ast::bitxor) { ret rslt(cx, cx.build.Xor(lhs, rhs)); }
        case (ast::lsl) { ret rslt(cx, cx.build.Shl(lhs, rhs)); }
        case (ast::lsr) { ret rslt(cx, cx.build.LShr(lhs, rhs)); }
        case (ast::asr) { ret rslt(cx, cx.build.AShr(lhs, rhs)); }
        case (_) { ret trans_compare(cx, op, intype, lhs, rhs); }
    }
}

fn autoderef(&@block_ctxt cx, ValueRef v, &ty::t t) -> result_t {
    let ValueRef v1 = v;
    let ty::t t1 = t;
    auto ccx = cx.fcx.lcx.ccx;
    while (true) {
        alt (ty::struct(ccx.tcx, t1)) {
            case (ty::ty_box(?mt)) {
                auto body =
                    cx.build.GEP(v1,
                                 ~[C_int(0), C_int(abi::box_rc_field_body)]);
                t1 = mt.ty;
                // Since we're changing levels of box indirection, we may have
                // to cast this pointer, since statically-sized tag types have
                // different types depending on whether they're behind a box
                // or not.
                if (!ty::type_has_dynamic_size(ccx.tcx, mt.ty)) {
                    auto llty = type_of(ccx, cx.sp, mt.ty);
                    v1 = cx.build.PointerCast(body, T_ptr(llty));
                } else { v1 = body; }
            }
            case (ty::ty_res(?did, ?inner, ?tps)) {
                t1 = ty::substitute_type_params(ccx.tcx, tps, inner);
                v1 = cx.build.GEP(v1, ~[C_int(0), C_int(1)]);
            }
            case (ty::ty_tag(?did, ?tps)) {
                auto variants = ty::tag_variants(ccx.tcx, did);
                if (std::ivec::len(variants) != 1u ||
                    std::ivec::len(variants.(0).args) != 1u) {
                    break;
                }
                t1 = ty::substitute_type_params
                    (ccx.tcx, tps, variants.(0).args.(0));
                if (!ty::type_has_dynamic_size(ccx.tcx, t1)) {
                    v1 = cx.build.PointerCast
                        (v1, T_ptr(type_of(ccx, cx.sp, t1)));
                }
            }
            case (_) { break; }
        }
        v1 = load_if_immediate(cx, v1, t1);
    }
    ret rec(bcx=cx, val=v1, ty=t1);
}

fn trans_binary(&@block_ctxt cx, ast::binop op, &@ast::expr a, &@ast::expr b)
   -> result {

    // First couple cases are lazy:
    alt (op) {
        case (ast::and) {
            // Lazy-eval and
            auto lhs_expr = trans_expr(cx, a);
            auto lhs_res =
                autoderef(lhs_expr.bcx, lhs_expr.val,
                          ty::expr_ty(cx.fcx.lcx.ccx.tcx, a));
            auto rhs_cx = new_scope_block_ctxt(cx, "rhs");
            auto rhs_expr = trans_expr(rhs_cx, b);
            auto rhs_res =
                autoderef(rhs_expr.bcx, rhs_expr.val,
                          ty::expr_ty(cx.fcx.lcx.ccx.tcx, b));
            auto lhs_false_cx = new_scope_block_ctxt(cx, "lhs false");
            auto lhs_false_res = rslt(lhs_false_cx, C_bool(false));
            // The following line ensures that any cleanups for rhs
            // are done within the block for rhs. This is necessary
            // because and/or are lazy. So the rhs may never execute,
            // and the cleanups can't be pushed into later code.

            auto rhs_bcx = trans_block_cleanups(rhs_res.bcx, rhs_cx);
            lhs_res.bcx.build.CondBr(lhs_res.val, rhs_cx.llbb,
                                     lhs_false_cx.llbb);
            ret join_results(cx, T_bool(),
                             ~[lhs_false_res, rec(bcx=rhs_bcx,
                                                  val=rhs_res.val)]);
        }
        case (ast::or) {
            // Lazy-eval or
            auto lhs_expr = trans_expr(cx, a);
            auto lhs_res = autoderef(lhs_expr.bcx, lhs_expr.val,
                                     ty::expr_ty(cx.fcx.lcx.ccx.tcx, a));
            auto rhs_cx = new_scope_block_ctxt(cx, "rhs");
            auto rhs_expr = trans_expr(rhs_cx, b);
            auto rhs_res = autoderef(rhs_expr.bcx, rhs_expr.val,
                                     ty::expr_ty(cx.fcx.lcx.ccx.tcx, b));
            auto lhs_true_cx = new_scope_block_ctxt(cx, "lhs true");
            auto lhs_true_res = rslt(lhs_true_cx, C_bool(true));
            // see the and case for an explanation

            auto rhs_bcx = trans_block_cleanups(rhs_res.bcx, rhs_cx);
            lhs_res.bcx.build.CondBr(lhs_res.val, lhs_true_cx.llbb,
                                     rhs_cx.llbb);
            ret join_results(cx, T_bool(),
                             ~[lhs_true_res, rec(bcx=rhs_bcx,
                                                 val=rhs_res.val)]);
        }
        case (_) {
            // Remaining cases are eager:

            auto lhs_expr = trans_expr(cx, a);
            auto lhty = ty::expr_ty(cx.fcx.lcx.ccx.tcx, a);
            auto lhs = autoderef(lhs_expr.bcx, lhs_expr.val, lhty);
            auto rhs_expr = trans_expr(lhs.bcx, b);
            auto rhty = ty::expr_ty(cx.fcx.lcx.ccx.tcx, b);
            auto rhs = autoderef(rhs_expr.bcx, rhs_expr.val, rhty);
            ret trans_eager_binop(rhs.bcx, op, lhs.ty,
                                  lhs.val, rhs.val);
        }
    }
}

fn join_results(&@block_ctxt parent_cx, TypeRef t, &result[] ins) -> result {
    let result[] live = ~[];
    let ValueRef[] vals = ~[];
    let BasicBlockRef[] bbs = ~[];
    for (result r in ins) {
        if (!is_terminated(r.bcx)) {
            live += ~[r];
            vals += ~[r.val];
            bbs += ~[r.bcx.llbb];
        }
    }
    alt (std::ivec::len[result](live)) {
        case (0u) {
            // No incoming edges are live, so we're in dead-code-land.
            // Arbitrarily pick the first dead edge, since the caller
            // is just going to propagate it outward.

            assert (std::ivec::len[result](ins) >= 1u);
            ret ins.(0);
        }
        case (_) {/* fall through */ }
    }
    // We have >1 incoming edges. Make a join block and br+phi them into it.

    auto join_cx = new_sub_block_ctxt(parent_cx, "join");
    for (result r in live) { r.bcx.build.Br(join_cx.llbb); }
    auto phi = join_cx.build.Phi(t, vals, bbs);
    ret rslt(join_cx, phi);
}

fn join_branches(&@block_ctxt parent_cx, &result[] ins) -> @block_ctxt {
    auto out = new_sub_block_ctxt(parent_cx, "join");
    for (result r in ins) {
        if (!is_terminated(r.bcx)) { r.bcx.build.Br(out.llbb); }
    }
    ret out;
}

tag out_method { return; save_in(ValueRef); }

fn trans_if(&@block_ctxt cx, &@ast::expr cond, &ast::blk thn,
            &option::t[@ast::expr] els, ast::node_id id, &out_method output)
    -> result {
    auto cond_res = trans_expr(cx, cond);
    auto then_cx = new_scope_block_ctxt(cx, "then");
    auto then_res = trans_block(then_cx, thn, output);
    auto else_cx = new_scope_block_ctxt(cx, "else");
    auto else_res =  alt (els) {
        case (some(?elexpr)) {
            alt (elexpr.node) {
                case (ast::expr_if(_, _, _)) {
                    // Synthesize a block here to act as the else block
                    // containing an if expression. Needed in order for the
                    // else scope to behave like a normal block scope. A tad
                    // ugly.
                    auto elseif_blk = ast::block_from_expr(elexpr);
                    trans_block(else_cx, elseif_blk, output)
                }
                case (ast::expr_block(?blk)) {
                    // Calling trans_block directly instead of trans_expr
                    // because trans_expr will create another scope block
                    // context for the block, but we've already got the
                    // 'else' context

                    trans_block(else_cx, blk, output)
                }
            }
        }
        case (_) { rslt(else_cx, C_nil()) }
    };
    cond_res.bcx.build.CondBr(cond_res.val, then_cx.llbb, else_cx.llbb);
    ret rslt(join_branches(cx, ~[then_res, else_res]), C_nil());
}

fn trans_for(&@block_ctxt cx, &@ast::local local, &@ast::expr seq,
             &ast::blk body) -> result {
    // FIXME: We bind to an alias here to avoid a segfault... this is
    // obviously a bug.
    fn inner(&@block_ctxt cx, @ast::local local, ValueRef curr, ty::t t,
             &ast::blk body, @block_ctxt outer_next_cx) -> result {
        auto next_cx = new_sub_block_ctxt(cx, "next");
        auto scope_cx =
            new_loop_scope_block_ctxt(cx, option::some[@block_ctxt](next_cx),
                                      outer_next_cx, "for loop scope");
        cx.build.Br(scope_cx.llbb);
        auto local_res = alloc_local(scope_cx, local);
        auto bcx = copy_val(local_res.bcx, INIT, local_res.val, curr, t).bcx;
        add_clean(scope_cx, local_res.val, t);
        bcx = trans_block(bcx, body, return).bcx;
        if (!bcx.build.is_terminated()) {
            bcx.build.Br(next_cx.llbb);
            // otherwise, this code is unreachable
        }
        ret rslt(next_cx, C_nil());
    }
    auto next_cx = new_sub_block_ctxt(cx, "next");
    auto seq_ty = ty::expr_ty(cx.fcx.lcx.ccx.tcx, seq);
    auto seq_res = trans_expr(cx, seq);
    auto it =
        iter_sequence(seq_res.bcx, seq_res.val, seq_ty,
                      bind inner(_, local, _, _, body, next_cx));
    it.bcx.build.Br(next_cx.llbb);
    ret rslt(next_cx, it.val);
}


// Iterator translation

// Finds the ValueRef associated with a variable in a function
// context. It checks locals, upvars, and args.
fn find_variable(&@fn_ctxt fcx, ast::node_id nid) -> ValueRef {
    ret
        alt (fcx.lllocals.find(nid)) {
            case (none) {
                alt (fcx.llupvars.find(nid)) {
                    case (none) {
                        alt (fcx.llargs.find(nid)) {
                            case (some(?llval)) { llval }
                            case (_) {
                                fcx.lcx.ccx.sess.bug("unbound var \
                                      in build_environment " + int::str(nid))
                            }
                        }
                    }
                    case (some(?llval)) { llval }
                }
            }
            case (some(?llval)) { llval }
        }
}

// Given a block context and a list of upvars, construct a closure that
// contains pointers to all of the upvars and all of the tydescs in
// scope. Return the ValueRef and TypeRef corresponding to the closure.
fn build_environment(&@block_ctxt cx, &freevar_set upvars) ->
    rec(ValueRef ptr, TypeRef ptrty) {
    auto has_iterbody = !option::is_none(cx.fcx.lliterbody);
    auto llbindingsptr;

    if (upvars.size() > 0u || has_iterbody) {
        // Gather up the upvars.
        let ValueRef[] llbindings = ~[];
        let TypeRef[] llbindingtys = ~[];
        if (has_iterbody) {
            llbindings += ~[option::get(cx.fcx.lliterbody)];
            llbindingtys += ~[val_ty(llbindings.(0))];
        }
        for each (ast::node_id nid in upvars.keys()) {
            auto llbinding = find_variable(cx.fcx, nid);
            llbindings += ~[llbinding];
            llbindingtys += ~[val_ty(llbinding)];
        }

        // Create an array of bindings and copy in aliases to the upvars.
        llbindingsptr = alloca(cx, T_struct(llbindingtys));
        auto upvar_count = std::ivec::len(llbindings);
        auto i = 0u;
        while (i < upvar_count) {
            auto llbindingptr =
                cx.build.GEP(llbindingsptr, ~[C_int(0), C_int(i as int)]);
            cx.build.Store(llbindings.(i), llbindingptr);
            i += 1u;
        }
    } else {
        // Null bindings.
        llbindingsptr = C_null(T_ptr(T_i8()));
    }

    // Create an environment and populate it with the bindings.
    auto tydesc_count = std::ivec::len[ValueRef](cx.fcx.lltydescs);
    auto llenvptrty =
        T_closure_ptr(*cx.fcx.lcx.ccx, T_ptr(T_nil()),
                      val_ty(llbindingsptr), tydesc_count);
    auto llenvptr = alloca(cx, llvm::LLVMGetElementType(llenvptrty));
    auto llbindingsptrptr =
        cx.build.GEP(llenvptr,
                     ~[C_int(0), C_int(abi::box_rc_field_body), C_int(2)]);
    cx.build.Store(llbindingsptr, llbindingsptrptr);

    // Copy in our type descriptors, in case the iterator body needs to refer
    // to them.
    auto lltydescsptr =
        cx.build.GEP(llenvptr,
                     ~[C_int(0), C_int(abi::box_rc_field_body),
                       C_int(abi::closure_elt_ty_params)]);
    auto i = 0u;
    while (i < tydesc_count) {
        auto lltydescptr =
            cx.build.GEP(lltydescsptr, ~[C_int(0), C_int(i as int)]);
        cx.build.Store(cx.fcx.lltydescs.(i), lltydescptr);
        i += 1u;
    }

    ret rec(ptr=llenvptr, ptrty=llenvptrty);
}

// Given an enclosing block context, a new function context, a closure type,
// and a list of upvars, generate code to load and populate the environment
// with the upvars and type descriptors.
fn load_environment(&@block_ctxt cx, &@fn_ctxt fcx,
                    TypeRef llenvptrty, &freevar_set upvars) {
    auto copy_args_bcx = new_raw_block_ctxt(fcx, fcx.llcopyargs);

    // Populate the upvars from the environment.
    auto llremoteenvptr =
        copy_args_bcx.build.PointerCast(fcx.llenv, llenvptrty);
    auto llremotebindingsptrptr =
        copy_args_bcx.build.GEP(llremoteenvptr,
                                ~[C_int(0), C_int(abi::box_rc_field_body),
                                  C_int(abi::closure_elt_bindings)]);
    auto llremotebindingsptr =
        copy_args_bcx.build.Load(llremotebindingsptrptr);

    auto i = 0u;
    if (!option::is_none(cx.fcx.lliterbody)) {
        i += 1u;
        auto lliterbodyptr =
            copy_args_bcx.build.GEP(llremotebindingsptr,
                                    ~[C_int(0), C_int(0)]);
        auto lliterbody = copy_args_bcx.build.Load(lliterbodyptr);
        fcx.lliterbody = some(lliterbody);
    }
    for each (ast::node_id upvar_id in upvars.keys()) {
        auto llupvarptrptr =
            copy_args_bcx.build.GEP(llremotebindingsptr,
                                    ~[C_int(0), C_int(i as int)]);
        auto llupvarptr = copy_args_bcx.build.Load(llupvarptrptr);
        fcx.llupvars.insert(upvar_id, llupvarptr);
        i += 1u;
    }

    // Populate the type parameters from the environment.
    auto llremotetydescsptr =
        copy_args_bcx.build.GEP(llremoteenvptr,
                                ~[C_int(0), C_int(abi::box_rc_field_body),
                                  C_int(abi::closure_elt_ty_params)]);
    auto tydesc_count = std::ivec::len(cx.fcx.lltydescs);
    i = 0u;
    while (i < tydesc_count) {
        auto llremotetydescptr =
            copy_args_bcx.build.GEP(llremotetydescsptr,
                                    ~[C_int(0), C_int(i as int)]);
        auto llremotetydesc = copy_args_bcx.build.Load(llremotetydescptr);
        fcx.lltydescs += ~[llremotetydesc];
        i += 1u;
    }

}

fn trans_for_each(&@block_ctxt cx, &@ast::local local, &@ast::expr seq,
                  &ast::blk body) -> result {
    /*
     * The translation is a little .. complex here. Code like:
     *
     *    let ty1 p = ...;
     *
     *    let ty1 q = ...;
     *
     *    foreach (ty v in foo(a,b)) { body(p,q,v) }
     *
     *
     * Turns into a something like so (C/Rust mishmash):
     *
     *    type env = { *ty1 p, *ty2 q, ... };
     *
     *    let env e = { &p, &q, ... };
     *
     *    fn foreach123_body(env* e, ty v) { body(*(e->p),*(e->q),v) }
     *
     *    foo([foreach123_body, env*], a, b);
     *
     */

    // Step 1: Generate code to build an environment containing pointers
    // to all of the upvars
    auto lcx = cx.fcx.lcx;

    // FIXME: possibly support alias-mode here?
    auto decl_ty = node_id_type(lcx.ccx, local.node.id);
    auto decl_id = local.node.id;
    auto upvars = get_freevars(lcx.ccx.tcx, body.node.id);

    auto llenv = build_environment(cx, upvars);

    // Step 2: Declare foreach body function.
    let str s =
        mangle_internal_name_by_path_and_seq(lcx.ccx, lcx.path, "foreach");

    // The 'env' arg entering the body function is a fake env member (as in
    // the env-part of the normal rust calling convention) that actually
    // points to a stack allocated env in this frame. We bundle that env
    // pointer along with the foreach-body-fn pointer into a 'normal' fn pair
    // and pass it in as a first class fn-arg to the iterator.
    auto iter_body_llty =
        type_of_fn_full(lcx.ccx, cx.sp, ast::proto_fn, false,
                        ~[rec(mode=ty::mo_alias(false), ty=decl_ty)],
                        ty::mk_nil(lcx.ccx.tcx), 0u);
    let ValueRef lliterbody =
        decl_internal_fastcall_fn(lcx.ccx.llmod, s, iter_body_llty);
    auto fcx = new_fn_ctxt(lcx, cx.sp, lliterbody);

    // Generate code to load the environment out of the
    // environment pointer.
    load_environment(cx, fcx, llenv.ptrty, upvars);

    // Add an upvar for the loop variable alias.
    fcx.llupvars.insert(decl_id, llvm::LLVMGetParam(fcx.llfn, 3u));
    auto bcx = new_top_block_ctxt(fcx);
    auto lltop = bcx.llbb;
    auto r = trans_block(bcx, body, return);
    finish_fn(fcx, lltop);

    if (!r.bcx.build.is_terminated()) {
        // if terminated is true, no need for the ret-fail
        r.bcx.build.RetVoid();
    }

    // Step 3: Call iter passing [lliterbody, llenv], plus other args.
    alt (seq.node) {
        case (ast::expr_call(?f, ?args)) {
            auto pair = create_real_fn_pair(cx, iter_body_llty,
                                            lliterbody, llenv.ptr);
            r = trans_call(cx, f, some[ValueRef](cx.build.Load(pair)),
                           args, seq.id);
            ret rslt(r.bcx, C_nil());
        }
    }
}

fn trans_while(&@block_ctxt cx, &@ast::expr cond, &ast::blk body) ->
   result {
    auto cond_cx = new_scope_block_ctxt(cx, "while cond");
    auto next_cx = new_sub_block_ctxt(cx, "next");
    auto body_cx =
        new_loop_scope_block_ctxt(cx, option::none[@block_ctxt], next_cx,
                                  "while loop body");
    auto body_res = trans_block(body_cx, body, return);
    auto cond_res = trans_expr(cond_cx, cond);
    body_res.bcx.build.Br(cond_cx.llbb);
    auto cond_bcx = trans_block_cleanups(cond_res.bcx, cond_cx);
    cond_bcx.build.CondBr(cond_res.val, body_cx.llbb, next_cx.llbb);
    cx.build.Br(cond_cx.llbb);
    ret rslt(next_cx, C_nil());
}

fn trans_do_while(&@block_ctxt cx, &ast::blk body, &@ast::expr cond) ->
   result {
    auto next_cx = new_sub_block_ctxt(cx, "next");
    auto body_cx =
        new_loop_scope_block_ctxt(cx, option::none[@block_ctxt], next_cx,
                                  "do-while loop body");
    auto body_res = trans_block(body_cx, body, return);
    auto cond_res = trans_expr(body_res.bcx, cond);
    cond_res.bcx.build.CondBr(cond_res.val, body_cx.llbb, next_cx.llbb);
    cx.build.Br(body_cx.llbb);
    ret rslt(next_cx, body_res.val);
}

type generic_info =
    rec(ty::t item_type,
        (option::t[@tydesc_info])[] static_tis,
        ValueRef[] tydescs);

type lval_result =
    rec(result res,
        bool is_mem,
        option::t[generic_info] generic,
        option::t[ValueRef] llobj,
        option::t[ty::t] method_ty);

fn lval_mem(&@block_ctxt cx, ValueRef val) -> lval_result {
    ret rec(res=rslt(cx, val),
            is_mem=true,
            generic=none[generic_info],
            llobj=none[ValueRef],
            method_ty=none[ty::t]);
}

fn lval_val(&@block_ctxt cx, ValueRef val) -> lval_result {
    ret rec(res=rslt(cx, val),
            is_mem=false,
            generic=none[generic_info],
            llobj=none[ValueRef],
            method_ty=none[ty::t]);
}

fn trans_external_path(&@block_ctxt cx, &ast::def_id did,
                       &ty::ty_param_count_and_ty tpt) -> ValueRef {
    auto lcx = cx.fcx.lcx;
    auto name = csearch::get_symbol(lcx.ccx.sess.get_cstore(), did);
    ret get_extern_const(lcx.ccx.externs, lcx.ccx.llmod, name,
                         type_of_ty_param_count_and_ty(lcx, cx.sp, tpt));
}

fn lval_generic_fn(&@block_ctxt cx, &ty::ty_param_count_and_ty tpt,
                   &ast::def_id fn_id, ast::node_id id) -> lval_result {
    auto lv;
    if (fn_id._0 == ast::local_crate) {
        // Internal reference.
        assert (cx.fcx.lcx.ccx.fn_pairs.contains_key(fn_id._1));
        lv = lval_val(cx, cx.fcx.lcx.ccx.fn_pairs.get(fn_id._1));
    } else {
        // External reference.
        lv = lval_val(cx, trans_external_path(cx, fn_id, tpt));
    }
    auto tys = ty::node_id_to_type_params(cx.fcx.lcx.ccx.tcx, id);
    if (std::ivec::len[ty::t](tys) != 0u) {
        auto bcx = lv.res.bcx;
        let ValueRef[] tydescs = ~[];
        let (option::t[@tydesc_info])[] tis = ~[];
        for (ty::t t in tys) {
            // TODO: Doesn't always escape.

            auto ti = none[@tydesc_info];
            auto td = get_tydesc(bcx, t, true, ti);
            tis += ~[ti];
            bcx = td.bcx;
            tydescs += ~[td.val];
        }
        auto gen = rec(item_type=tpt._1, static_tis=tis, tydescs=tydescs);
        lv = rec(res=rslt(bcx, lv.res.val), generic=some[generic_info](gen)
                 with lv);
    }
    ret lv;
}

fn lookup_discriminant(&@local_ctxt lcx, &ast::def_id tid, &ast::def_id vid)
   -> ValueRef {
    alt (lcx.ccx.discrims.find(vid._1)) {
        case (none) {
            // It's an external discriminant that we haven't seen yet.

            assert (vid._0 != ast::local_crate);
            auto sym = csearch::get_symbol(lcx.ccx.sess.get_cstore(), vid);
            auto gvar =
                llvm::LLVMAddGlobal(lcx.ccx.llmod, T_int(), str::buf(sym));
            llvm::LLVMSetLinkage(gvar,
                                 lib::llvm::LLVMExternalLinkage as
                                     llvm::Linkage);
            llvm::LLVMSetGlobalConstant(gvar, True);
            lcx.ccx.discrims.insert(vid._1, gvar);
            ret gvar;
        }
        case (some(?llval)) { ret llval; }
    }
}

fn trans_path(&@block_ctxt cx, &ast::path p, ast::node_id id) -> lval_result {
    auto ccx = cx.fcx.lcx.ccx;
    alt (cx.fcx.lcx.ccx.tcx.def_map.find(id)) {
        case (some(ast::def_arg(?did))) {
            alt (cx.fcx.llargs.find(did._1)) {
                case (none) {
                    assert (cx.fcx.llupvars.contains_key(did._1));
                    ret lval_mem(cx, cx.fcx.llupvars.get(did._1));
                }
                case (some(?llval)) { ret lval_mem(cx, llval); }
            }
        }
        case (some(ast::def_local(?did))) {
            alt (cx.fcx.lllocals.find(did._1)) {
                case (none) {
                    assert (cx.fcx.llupvars.contains_key(did._1));
                    ret lval_mem(cx, cx.fcx.llupvars.get(did._1));
                }
                case (some(?llval)) { ret lval_mem(cx, llval); }
            }
        }
        case (some(ast::def_binding(?did))) {
            alt (cx.fcx.lllocals.find(did._1)) {
                case (none) {
                    assert (cx.fcx.llupvars.contains_key(did._1));
                    ret lval_mem(cx, cx.fcx.llupvars.get(did._1));
                }
                case (some(?llval)) { ret lval_mem(cx, llval); }
            }
        }
        case (some(ast::def_obj_field(?did))) {
            assert (cx.fcx.llobjfields.contains_key(did._1));
            ret lval_mem(cx, cx.fcx.llobjfields.get(did._1));
        }
        case (some(ast::def_fn(?did, _))) {
            auto tyt = ty::lookup_item_type(ccx.tcx, did);
            ret lval_generic_fn(cx, tyt, did, id);
        }
        case (some(ast::def_variant(?tid, ?vid))) {
            auto v_tyt = ty::lookup_item_type(ccx.tcx, vid);
            alt (ty::struct(ccx.tcx, v_tyt._1)) {
                case (ty::ty_fn(_, _, _, _, _)) {
                    // N-ary variant.

                    ret lval_generic_fn(cx, v_tyt, vid, id);
                }
                case (_) {
                    // Nullary variant.
                    auto tag_ty = node_id_type(ccx, id);
                    auto alloc_result = alloc_ty(cx, tag_ty);
                    auto lltagblob = alloc_result.val;
                    auto lltagty = type_of_tag(ccx, p.span, tid, tag_ty);
                    auto bcx = alloc_result.bcx;
                    auto lltagptr = bcx.build.PointerCast
                        (lltagblob, T_ptr(lltagty));
                    if (std::ivec::len(ty::tag_variants(ccx.tcx, tid))
                            != 1u) {
                        auto lldiscrim_gv =
                            lookup_discriminant(bcx.fcx.lcx, tid, vid);
                        auto lldiscrim = bcx.build.Load(lldiscrim_gv);
                        auto lldiscrimptr = bcx.build.GEP
                            (lltagptr, ~[C_int(0), C_int(0)]);
                        bcx.build.Store(lldiscrim, lldiscrimptr);
                    }
                    ret lval_val(bcx, lltagptr);
                }
            }
        }
        case (some(ast::def_const(?did))) {
          if (did._0 == ast::local_crate) {
              assert (ccx.consts.contains_key(did._1));
              ret lval_mem(cx, ccx.consts.get(did._1));
          } else {
              auto tp = ty::node_id_to_monotype(ccx.tcx, id);
              ret lval_val(cx, load_if_immediate
                           (cx, trans_external_path
                            (cx, did, tup(0u, tp)), tp));
          }
        }
        case (some(ast::def_native_fn(?did))) {
            auto tyt = ty::lookup_item_type(ccx.tcx, did);
            ret lval_generic_fn(cx, tyt, did, id);
        }
        case (_) {
            ccx.sess.span_unimpl(cx.sp, "def variant in trans");
        }
    }
}

fn trans_field(&@block_ctxt cx, &span sp, ValueRef v, &ty::t t0,
               &ast::ident field, ast::node_id id) -> lval_result {
    auto r = autoderef(cx, v, t0);
    auto t = r.ty;
    alt (ty::struct(cx.fcx.lcx.ccx.tcx, t)) {
        case (ty::ty_tup(_)) {
            let uint ix = ty::field_num(cx.fcx.lcx.ccx.sess, sp, field);
            auto v = GEP_tup_like(r.bcx, t, r.val, ~[0, ix as int]);
            ret lval_mem(v.bcx, v.val);
        }
        case (ty::ty_rec(?fields)) {
            let uint ix =
                ty::field_idx(cx.fcx.lcx.ccx.sess, sp, field, fields);
            auto v = GEP_tup_like(r.bcx, t, r.val, ~[0, ix as int]);
            ret lval_mem(v.bcx, v.val);
        }
        case (ty::ty_obj(?methods)) {
            let uint ix =
                ty::method_idx(cx.fcx.lcx.ccx.sess, sp, field, methods);
            auto vtbl =
                r.bcx.build.GEP(r.val,
                                ~[C_int(0), C_int(abi::obj_field_vtbl)]);
            vtbl = r.bcx.build.Load(vtbl);

            auto vtbl_type = T_ptr(T_array(T_ptr(T_nil()), ix + 2u));
            vtbl = cx.build.PointerCast(vtbl, vtbl_type);

            // +1 because slot #0 contains the destructor
            auto v = r.bcx.build.GEP(vtbl,
                                     ~[C_int(0), C_int(ix + 1u as int)]);
            let ty::t fn_ty =
                ty::method_ty_to_fn_ty(cx.fcx.lcx.ccx.tcx, methods.(ix));
            auto tcx = cx.fcx.lcx.ccx.tcx;
            auto ll_fn_ty = type_of_fn_full(cx.fcx.lcx.ccx, sp,
                                            ty::ty_fn_proto(tcx, fn_ty),
                                            true,
                                            ty::ty_fn_args(tcx, fn_ty),
                                            ty::ty_fn_ret(tcx, fn_ty),
                                            0u);
            v = r.bcx.build.PointerCast(v, T_ptr(T_ptr(ll_fn_ty)));
            auto lvo = lval_mem(r.bcx, v);
            ret rec(llobj=some[ValueRef](r.val), method_ty=some[ty::t](fn_ty)
                    with lvo);
        }
        case (_) {
            cx.fcx.lcx.ccx.sess.unimpl("field variant in trans_field");
        }
    }
}

fn trans_index(&@block_ctxt cx, &span sp, &@ast::expr base, &@ast::expr idx,
               ast::node_id id) -> lval_result {
    // Is this an interior vector?

    auto base_ty = ty::expr_ty(cx.fcx.lcx.ccx.tcx, base);
    auto exp = trans_expr(cx, base);
    auto lv = autoderef(exp.bcx, exp.val, base_ty);
    auto base_ty_no_boxes = lv.ty;
    auto is_interior =
        ty::sequence_is_interior(cx.fcx.lcx.ccx.tcx, base_ty_no_boxes);
    auto ix = trans_expr(lv.bcx, idx);
    auto v = lv.val;
    auto bcx = ix.bcx;
    // Cast to an LLVM integer. Rust is less strict than LLVM in this regard.

    auto ix_val;
    auto ix_size = llsize_of_real(cx.fcx.lcx.ccx, val_ty(ix.val));
    auto int_size = llsize_of_real(cx.fcx.lcx.ccx, T_int());
    if (ix_size < int_size) {
        ix_val = bcx.build.ZExt(ix.val, T_int());
    } else if (ix_size > int_size) {
        ix_val = bcx.build.Trunc(ix.val, T_int());
    } else { ix_val = ix.val; }
    auto unit_ty = node_id_type(cx.fcx.lcx.ccx, id);
    auto unit_sz = size_of(bcx, unit_ty);
    bcx = unit_sz.bcx;
    maybe_name_value(cx.fcx.lcx.ccx, unit_sz.val, "unit_sz");
    auto scaled_ix = bcx.build.Mul(ix_val, unit_sz.val);
    maybe_name_value(cx.fcx.lcx.ccx, scaled_ix, "scaled_ix");
    auto interior_len_and_data;
    if (is_interior) {
        auto rslt = ivec::get_len_and_data(bcx, v, unit_ty);
        interior_len_and_data = some(tup(rslt._0, rslt._1));
        bcx = rslt._2;
    } else { interior_len_and_data = none; }
    auto lim;
    alt (interior_len_and_data) {
        case (some(?lad)) { lim = lad._0; }
        case (none) {
            lim = bcx.build.GEP(v, ~[C_int(0), C_int(abi::vec_elt_fill)]);
            lim = bcx.build.Load(lim);
        }
    }
    auto bounds_check = bcx.build.ICmp(lib::llvm::LLVMIntULT, scaled_ix, lim);
    auto fail_cx = new_sub_block_ctxt(bcx, "fail");
    auto next_cx = new_sub_block_ctxt(bcx, "next");
    bcx.build.CondBr(bounds_check, next_cx.llbb, fail_cx.llbb);
    // fail: bad bounds check.

    trans_fail(fail_cx, some[span](sp), "bounds check");
    auto body;
    alt (interior_len_and_data) {
        case (some(?lad)) { body = lad._1; }
        case (none) {
            body =
                next_cx.build.GEP(v,
                                  ~[C_int(0), C_int(abi::vec_elt_data),
                                    C_int(0)]);
        }
    }
    auto elt;
    if (ty::type_has_dynamic_size(cx.fcx.lcx.ccx.tcx, unit_ty)) {
        body = next_cx.build.PointerCast(body, T_ptr(T_i8()));
        elt = next_cx.build.GEP(body, ~[scaled_ix]);
    } else {
        elt = next_cx.build.GEP(body, ~[ix_val]);
        // We're crossing a box boundary here, so we may need to pointer cast.

        auto llunitty = type_of(next_cx.fcx.lcx.ccx, sp, unit_ty);
        elt = next_cx.build.PointerCast(elt, T_ptr(llunitty));
    }
    ret lval_mem(next_cx, elt);
}


// The additional bool returned indicates whether it's mem (that is
// represented as an alloca or heap, hence needs a 'load' to be used as an
// immediate).
fn trans_lval_gen(&@block_ctxt cx, &@ast::expr e) -> lval_result {
    alt (e.node) {
        case (ast::expr_path(?p)) { ret trans_path(cx, p, e.id); }
        case (ast::expr_field(?base, ?ident)) {
            auto r = trans_expr(cx, base);
            auto t = ty::expr_ty(cx.fcx.lcx.ccx.tcx, base);
            ret trans_field(r.bcx, e.span, r.val, t, ident, e.id);
        }
        case (ast::expr_index(?base, ?idx)) {
            ret trans_index(cx, e.span, base, idx, e.id);
        }
        case (ast::expr_unary(ast::deref, ?base)) {
            auto ccx = cx.fcx.lcx.ccx;
            auto sub = trans_expr(cx, base);
            auto t = ty::expr_ty(ccx.tcx, base);
            auto val = alt (ty::struct(ccx.tcx, t)) {
                case (ty::ty_box(_)) {
                    sub.bcx.build.InBoundsGEP
                    (sub.val, ~[C_int(0), C_int(abi::box_rc_field_body)])
                }
                case (ty::ty_res(_, _, _)) {
                    sub.bcx.build.InBoundsGEP(sub.val, ~[C_int(0), C_int(1)])
                }
                case (ty::ty_tag(_, _)) {
                    auto ety = ty::expr_ty(ccx.tcx, e);
                    auto ellty;
                    if (ty::type_has_dynamic_size(ccx.tcx, ety)) {
                        ellty = T_typaram_ptr(ccx.tn);
                    } else {
                        ellty = T_ptr(type_of(ccx, e.span, ety));
                    };
                    sub.bcx.build.PointerCast(sub.val, ellty)
                }
                case (ty::ty_ptr(_)) { sub.val }
            };
            ret lval_mem(sub.bcx, val);
        }
        case (ast::expr_self_method(?ident)) {
            alt ({ cx.fcx.llself }) {
                case (some(?pair)) {
                    auto r = pair.v;
                    auto t = pair.t;
                    ret trans_field(cx, e.span, r, t, ident, e.id);
                }
                case (_) {
                    // Shouldn't happen.

                    cx.fcx.lcx.ccx.sess.bug("trans_lval called on " +
                                                "expr_self_method in " +
                                                "a context without llself");
                }
            }
        }
        case (_) {
            ret rec(res=trans_expr(cx, e),
                    is_mem=false,
                    generic=none,
                    llobj=none,
                    method_ty=none);
        }
    }
}

fn trans_lval(&@block_ctxt cx, &@ast::expr e) -> lval_result {
    auto lv = trans_lval_gen(cx, e);
    alt (lv.generic) {
        case (some(?gi)) {
            auto t = ty::expr_ty(cx.fcx.lcx.ccx.tcx, e);
            auto n_args =
                std::ivec::len(ty::ty_fn_args(cx.fcx.lcx.ccx.tcx, t));
            auto args = std::ivec::init_elt(none[@ast::expr], n_args);
            auto bound = trans_bind_1(lv.res.bcx, e, lv, args, e.id);
            ret lval_val(bound.bcx, bound.val);
        }
        case (none) {
            ret lv;
        }
    }
}

fn int_cast(&@block_ctxt bcx, TypeRef lldsttype, TypeRef llsrctype,
            ValueRef llsrc, bool signed) -> ValueRef {
    auto srcsz = llvm::LLVMGetIntTypeWidth(llsrctype);
    auto dstsz = llvm::LLVMGetIntTypeWidth(lldsttype);
    ret if dstsz == srcsz { bcx.build.BitCast(llsrc, lldsttype) }
        else if srcsz > dstsz { bcx.build.TruncOrBitCast(llsrc, lldsttype) }
        else if signed { bcx.build.SExtOrBitCast(llsrc, lldsttype) }
        else { bcx.build.ZExtOrBitCast(llsrc, lldsttype) };
}

fn float_cast(&@block_ctxt bcx, TypeRef lldsttype, TypeRef llsrctype,
              ValueRef llsrc) -> ValueRef {
    auto srcsz = lib::llvm::float_width(llsrctype);
    auto dstsz = lib::llvm::float_width(lldsttype);
    ret if dstsz > srcsz { bcx.build.FPExt(llsrc, lldsttype) }
        else if srcsz > dstsz { bcx.build.FPTrunc(llsrc, lldsttype) }
        else { llsrc };
}

fn trans_cast(&@block_ctxt cx, &@ast::expr e, ast::node_id id) -> result {
    auto ccx = cx.fcx.lcx.ccx;
    auto e_res = trans_expr(cx, e);
    auto ll_t_in = val_ty(e_res.val);
    auto t_in = ty::expr_ty(ccx.tcx, e);
    auto t_out = node_id_type(ccx, id);
    auto ll_t_out = type_of(ccx, e.span, t_out);

    tag kind { native_; integral; float; other; }
    fn t_kind(&ty::ctxt tcx, ty::t t) -> kind {
        ret if ty::type_is_fp(tcx, t) { float }
            else if ty::type_is_native(tcx, t) { native_ }
            else if ty::type_is_integral(tcx, t) { integral }
            else { other };
    }
    auto k_in = t_kind(ccx.tcx, t_in);
    auto k_out = t_kind(ccx.tcx, t_out);
    auto s_in = k_in == integral && ty::type_is_signed(ccx.tcx, t_in);

    auto newval = alt rec(in=k_in, out=k_out) {
      {in: integral, out: integral} {
        int_cast(e_res.bcx, ll_t_out, ll_t_in, e_res.val, s_in)
      }
      {in: float, out: float} {
        float_cast(e_res.bcx, ll_t_out, ll_t_in, e_res.val)
      }
      {in: integral, out: float} {
        if s_in { e_res.bcx.build.SIToFP(e_res.val, ll_t_out) }
        else { e_res.bcx.build.UIToFP(e_res.val, ll_t_out) }
      }
      {in: float, out: integral} {
        if ty::type_is_signed(ccx.tcx, t_out) {
            e_res.bcx.build.FPToSI(e_res.val, ll_t_out)
        } else { e_res.bcx.build.FPToUI(e_res.val, ll_t_out) }
      }
      {in: integral, out: native_} {
        e_res.bcx.build.IntToPtr(e_res.val, ll_t_out)
      }
      {in: native_, out: integral} {
        e_res.bcx.build.PtrToInt(e_res.val, ll_t_out)
      }
      {in: native_, out: native_} {
        e_res.bcx.build.PointerCast(e_res.val, ll_t_out)
      }
      _ {
        ccx.sess.bug("Translating unsupported cast.")
      }
    };
    ret rslt(e_res.bcx, newval);
}

fn trans_bind_thunk(&@local_ctxt cx, &span sp, &ty::t incoming_fty,
                    &ty::t outgoing_fty, &(option::t[@ast::expr])[] args,
                    &ty::t closure_ty, &ty::t[] bound_tys,
                    uint ty_param_count) -> ValueRef {

    // Here we're not necessarily constructing a thunk in the sense of
    // "function with no arguments".  The result of compiling 'bind f(foo,
    // bar, baz)' would be a thunk that, when called, applies f to those
    // arguments and returns the result.  But we're stretching the meaning of
    // the word "thunk" here to also mean the result of compiling, say, 'bind
    // f(foo, _, baz)', or any other bind expression that binds f and leaves
    // some (or all) of the arguments unbound.

    // Here, 'incoming_fty' is the type of the entire bind expression, while
    // 'outgoing_fty' is the type of the function that is having some of its
    // arguments bound.  If f is a function that takes three arguments of type
    // int and returns int, and we're translating, say, 'bind f(3, _, 5)',
    // then outgoing_fty is the type of f, which is (int, int, int) -> int,
    // and incoming_fty is the type of 'bind f(3, _, 5)', which is int -> int.

    // Once translated, the entire bind expression will be the call f(foo,
    // bar, baz) wrapped in a (so-called) thunk that takes 'bar' as its
    // argument and that has bindings of 'foo' to 3 and 'baz' to 5 and a
    // pointer to 'f' all saved in its environment.  So, our job is to
    // construct and return that thunk.

    // Give the thunk a name, type, and value.
    let str s =
        mangle_internal_name_by_path_and_seq(cx.ccx, cx.path, "thunk");
    let TypeRef llthunk_ty =
        get_pair_fn_ty(type_of(cx.ccx, sp, incoming_fty));
    let ValueRef llthunk =
        decl_internal_fastcall_fn(cx.ccx.llmod, s, llthunk_ty);

    // Create a new function context and block context for the thunk, and hold
    // onto a pointer to the first block in the function for later use.
    auto fcx = new_fn_ctxt(cx, sp, llthunk);
    auto bcx = new_top_block_ctxt(fcx);
    auto lltop = bcx.llbb;
    // Since we might need to construct derived tydescs that depend on
    // our bound tydescs, we need to load tydescs out of the environment
    // before derived tydescs are constructed. To do this, we load them
    // in the copy_args block.
    auto copy_args_bcx = new_raw_block_ctxt(fcx, fcx.llcopyargs);

    // The 'llenv' that will arrive in the thunk we're creating is an
    // environment that will contain the values of its arguments and a pointer
    // to the original function.  So, let's create one of those:

    // The llenv pointer needs to be the correct size.  That size is
    // 'closure_ty', which was determined by trans_bind.
    auto llclosure_ptr_ty =
        type_of(cx.ccx, sp, ty::mk_imm_box(cx.ccx.tcx, closure_ty));
    auto llclosure = copy_args_bcx.build.PointerCast(fcx.llenv,
                                                     llclosure_ptr_ty);

    // "target", in this context, means the function that's having some of its
    // arguments bound and that will be called inside the thunk we're
    // creating.  (In our running example, target is the function f.)  Pick
    // out the pointer to the target function from the environment.
    auto lltarget =
        GEP_tup_like(bcx, closure_ty, llclosure,
                     ~[0, abi::box_rc_field_body, abi::closure_elt_target]);
    bcx = lltarget.bcx;

    // And then, pick out the target function's own environment.  That's what
    // we'll use as the environment the thunk gets.
    auto lltargetclosure =
        bcx.build.GEP(lltarget.val, ~[C_int(0), C_int(abi::fn_field_box)]);
    lltargetclosure = bcx.build.Load(lltargetclosure);

    // Get f's return type, which will also be the return type of the entire
    // bind expression.
    auto outgoing_ret_ty = ty::ty_fn_ret(cx.ccx.tcx, outgoing_fty);

    // Get the types of the arguments to f.
    auto outgoing_args = ty::ty_fn_args(cx.ccx.tcx, outgoing_fty);

    // The 'llretptr' that will arrive in the thunk we're creating also needs
    // to be the correct size.  Cast it to the size of f's return type, if
    // necessary.
    auto llretptr = fcx.llretptr;
    if (ty::type_has_dynamic_size(cx.ccx.tcx, outgoing_ret_ty)) {
        llretptr = bcx.build.PointerCast(llretptr, T_typaram_ptr(cx.ccx.tn));
    }

    // Set up the three implicit arguments to the thunk.
    let ValueRef[] llargs = ~[llretptr, fcx.lltaskptr, lltargetclosure];

    // Copy in the type parameters.
    let uint i = 0u;
    while (i < ty_param_count) {
        auto lltyparam_ptr =
            GEP_tup_like(copy_args_bcx, closure_ty, llclosure,
                         ~[0, abi::box_rc_field_body,
                           abi::closure_elt_ty_params, i as int]);
        copy_args_bcx = lltyparam_ptr.bcx;
        auto td = copy_args_bcx.build.Load(lltyparam_ptr.val);
        llargs += ~[td];
        fcx.lltydescs += ~[td];
        i += 1u;
    }

    let uint a = 3u; // retptr, task ptr, env come first

    let int b = 0;
    let uint outgoing_arg_index = 0u;
    let TypeRef[] llout_arg_tys =
        type_of_explicit_args(cx.ccx, sp, outgoing_args);
    for (option::t[@ast::expr] arg in args) {
        auto out_arg = outgoing_args.(outgoing_arg_index);
        auto llout_arg_ty = llout_arg_tys.(outgoing_arg_index);
        alt (arg) {
            // Arg provided at binding time; thunk copies it from
            // closure.
            case (some(?e)) {
                auto e_ty = ty::expr_ty(cx.ccx.tcx, e);
                auto bound_arg =
                    GEP_tup_like(bcx, closure_ty, llclosure,
                                 ~[0, abi::box_rc_field_body,
                                   abi::closure_elt_bindings, b]);
                bcx = bound_arg.bcx;
                auto val = bound_arg.val;
                if (out_arg.mode == ty::mo_val) {
                    if (type_is_immediate(cx.ccx, e_ty)) {
                        val = bcx.build.Load(val);
                        bcx = copy_ty(bcx, val, e_ty).bcx;
                    } else {
                        bcx = copy_ty(bcx, val, e_ty).bcx;
                        val = bcx.build.Load(val);
                    }
                }
                // If the type is parameterized, then we need to cast the
                // type we actually have to the parameterized out type.
                if (ty::type_contains_params(cx.ccx.tcx, out_arg.ty)) {
                    // FIXME: (#642) This works for boxes and alias params
                    // but does not work for bare functions.
                    val = bcx.build.PointerCast(val, llout_arg_ty);
                }
                llargs += ~[val];
                b += 1;
            }
            case (
                 // Arg will be provided when the thunk is invoked.
                 none) {
                let ValueRef passed_arg = llvm::LLVMGetParam(llthunk, a);
                if (ty::type_contains_params(cx.ccx.tcx, out_arg.ty)) {
                    assert (out_arg.mode != ty::mo_val);
                    passed_arg =
                        bcx.build.PointerCast(passed_arg, llout_arg_ty);
                }
                llargs += ~[passed_arg];
                a += 1u;
            }
        }
        outgoing_arg_index += 1u;
    }
    // FIXME: turn this call + ret into a tail call.

    auto lltargetfn =
        bcx.build.GEP(lltarget.val, ~[C_int(0), C_int(abi::fn_field_code)]);

    // Cast the outgoing function to the appropriate type (see the comments in
    // trans_bind below for why this is necessary).
    auto lltargetty =
        type_of_fn(bcx.fcx.lcx.ccx, sp,
                   ty::ty_fn_proto(bcx.fcx.lcx.ccx.tcx, outgoing_fty),
                   outgoing_args, outgoing_ret_ty, ty_param_count);
    lltargetfn = bcx.build.PointerCast(lltargetfn, T_ptr(T_ptr(lltargetty)));
    lltargetfn = bcx.build.Load(lltargetfn);
    bcx.build.FastCall(lltargetfn, llargs);
    bcx.build.RetVoid();
    finish_fn(fcx, lltop);
    ret llthunk;
}

fn trans_bind(&@block_ctxt cx, &@ast::expr f,
              &(option::t[@ast::expr])[] args, ast::node_id id) -> result {
    auto f_res = trans_lval_gen(cx, f);
    ret trans_bind_1(cx, f, f_res, args, id);
}

fn trans_bind_1(&@block_ctxt cx, &@ast::expr f, &lval_result f_res,
                &(option::t[@ast::expr])[] args, ast::node_id id) -> result {
    if (f_res.is_mem) {
        cx.fcx.lcx.ccx.sess.unimpl("re-binding existing function");
    } else {
        let (@ast::expr)[] bound = ~[];
        for (option::t[@ast::expr] argopt in args) {
            alt (argopt) {
                case (none) { }
                case (some(?e)) { bound += ~[e]; }
            }
        }

        // Figure out which tydescs we need to pass, if any.
        let ty::t outgoing_fty;
        let ValueRef[] lltydescs;
        alt (f_res.generic) {
            case (none) {
                outgoing_fty = ty::expr_ty(cx.fcx.lcx.ccx.tcx, f);
                lltydescs = ~[];
            }
            case (some(?ginfo)) {
                lazily_emit_all_generic_info_tydesc_glues(cx, ginfo);
                outgoing_fty = ginfo.item_type;
                lltydescs = ginfo.tydescs;
            }
        }
        auto ty_param_count = std::ivec::len[ValueRef](lltydescs);
        if (std::ivec::len[@ast::expr](bound) == 0u && ty_param_count == 0u) {

            // Trivial 'binding': just return the static pair-ptr.
            ret f_res.res;
        } else {
            auto bcx = f_res.res.bcx;
            auto pair_t = node_type(cx.fcx.lcx.ccx, cx.sp, id);
            auto pair_v = alloca(bcx, pair_t);

            // Translate the bound expressions.
            let ty::t[] bound_tys = ~[];
            let lval_result[] bound_vals = ~[];
            for (@ast::expr e in bound) {
                auto lv = trans_lval(bcx, e);
                bcx = lv.res.bcx;
                bound_vals += ~[lv];
                bound_tys += ~[ty::expr_ty(cx.fcx.lcx.ccx.tcx, e)];
            }

            // Synthesize a closure type.

            // First, synthesize a tuple type containing the types of all the
            // bound expressions.
            // bindings_ty = ~[bound_ty1, bound_ty2, ...]

            let ty::t bindings_ty = ty::mk_imm_tup(cx.fcx.lcx.ccx.tcx,
                                                   bound_tys);

            // NB: keep this in sync with T_closure_ptr; we're making
            // a ty::t structure that has the same "shape" as the LLVM type
            // it constructs.

            // Make a vector that contains ty_param_count copies of tydesc_ty.
            // (We'll need room for that many tydescs in the closure.)
            let ty::t tydesc_ty = ty::mk_type(cx.fcx.lcx.ccx.tcx);
            let ty::t[] captured_tys =
                std::ivec::init_elt[ty::t](tydesc_ty, ty_param_count);

            // Get all the types we've got (some of which we synthesized
            // ourselves) into a vector.  The whole things ends up looking
            // like:

            // closure_tys = [tydesc_ty, outgoing_fty, [bound_ty1, bound_ty2,
            // ...], [tydesc_ty, tydesc_ty, ...]]
            let ty::t[] closure_tys =
                ~[tydesc_ty, outgoing_fty, bindings_ty,
                  ty::mk_imm_tup(cx.fcx.lcx.ccx.tcx, captured_tys)];

            // Finally, synthesize a type for that whole vector.
            let ty::t closure_ty =
                ty::mk_imm_tup(cx.fcx.lcx.ccx.tcx, closure_tys);

            // Allocate a box that can hold something closure-sized, including
            // space for a refcount.
            auto r = trans_malloc_boxed(bcx, closure_ty);
            auto box = r.val;
            bcx = r.bcx;

            // Grab onto the refcount and body parts of the box we allocated.
            auto rc =
                bcx.build.GEP(box,
                              ~[C_int(0), C_int(abi::box_rc_field_refcnt)]);
            auto closure =
                bcx.build.GEP(box, ~[C_int(0),
                                     C_int(abi::box_rc_field_body)]);
            bcx.build.Store(C_int(1), rc);

            // Store bindings tydesc.
            auto bound_tydesc =
                bcx.build.GEP(closure,
                              ~[C_int(0), C_int(abi::closure_elt_tydesc)]);
            auto ti = none[@tydesc_info];
            auto bindings_tydesc = get_tydesc(bcx, bindings_ty, true, ti);
            lazily_emit_tydesc_glue(bcx, abi::tydesc_field_drop_glue, ti);
            lazily_emit_tydesc_glue(bcx, abi::tydesc_field_free_glue, ti);
            bcx = bindings_tydesc.bcx;
            bcx.build.Store(bindings_tydesc.val, bound_tydesc);

            // Determine the LLVM type for the outgoing function type. This
            // may be different from the type returned by trans_malloc_boxed()
            // since we have more information than that function does;
            // specifically, we know how many type descriptors the outgoing
            // function has, which type_of() doesn't, as only we know which
            // item the function refers to.
            auto llfnty =
                type_of_fn(bcx.fcx.lcx.ccx, cx.sp,
                           ty::ty_fn_proto(bcx.fcx.lcx.ccx.tcx, outgoing_fty),
                           ty::ty_fn_args(bcx.fcx.lcx.ccx.tcx, outgoing_fty),
                           ty::ty_fn_ret(bcx.fcx.lcx.ccx.tcx, outgoing_fty),
                           ty_param_count);
            auto llclosurety = T_ptr(T_fn_pair(*bcx.fcx.lcx.ccx, llfnty));

            // Store thunk-target.
            auto bound_target =
                bcx.build.GEP(closure,
                              ~[C_int(0), C_int(abi::closure_elt_target)]);
            auto src = bcx.build.Load(f_res.res.val);
            bound_target = bcx.build.PointerCast(bound_target, llclosurety);
            bcx.build.Store(src, bound_target);

            // Copy expr values into boxed bindings.
            auto i = 0u;
            auto bindings =
                bcx.build.GEP(closure,
                              ~[C_int(0), C_int(abi::closure_elt_bindings)]);
            for (lval_result lv in bound_vals) {
                auto bound =
                    bcx.build.GEP(bindings, ~[C_int(0), C_int(i as int)]);
                bcx = move_val_if_temp(bcx, INIT, bound, lv,
                                       bound_tys.(i)).bcx;
                i += 1u;
            }

            // If necessary, copy tydescs describing type parameters into the
            // appropriate slot in the closure.
            alt (f_res.generic) {
                case (none) {/* nothing to do */ }
                case (some(?ginfo)) {
                    lazily_emit_all_generic_info_tydesc_glues(cx, ginfo);
                    auto ty_params_slot =
                        bcx.build.GEP(closure,
                                      ~[C_int(0),
                                        C_int(abi::closure_elt_ty_params)]);
                    auto i = 0;
                    for (ValueRef td in ginfo.tydescs) {
                        auto ty_param_slot =
                            bcx.build.GEP(ty_params_slot,
                                          ~[C_int(0), C_int(i)]);
                        bcx.build.Store(td, ty_param_slot);
                        i += 1;
                    }
                    outgoing_fty = ginfo.item_type;
                }
            }

            // Make thunk and store thunk-ptr in outer pair's code slot.
            auto pair_code =
                bcx.build.GEP(pair_v, ~[C_int(0), C_int(abi::fn_field_code)]);
            // The type of the entire bind expression.
            let ty::t pair_ty = node_id_type(cx.fcx.lcx.ccx, id);

            let ValueRef llthunk =
                trans_bind_thunk(cx.fcx.lcx, cx.sp, pair_ty, outgoing_fty,
                                 args, closure_ty, bound_tys, ty_param_count);
            bcx.build.Store(llthunk, pair_code);

            // Store box ptr in outer pair's box slot.
            auto ccx = *bcx.fcx.lcx.ccx;
            auto pair_box =
                bcx.build.GEP(pair_v, ~[C_int(0), C_int(abi::fn_field_box)]);
            bcx.build.Store(bcx.build.PointerCast(box,
                                                  T_opaque_closure_ptr(ccx)),
                            pair_box);
            add_clean_temp(cx, pair_v, pair_ty);
            ret rslt(bcx, pair_v);
        }
    }
}

fn trans_arg_expr(&@block_ctxt cx, &ty::arg arg, TypeRef lldestty0,
                  &@ast::expr e) -> result {
    auto ccx = cx.fcx.lcx.ccx;
    auto e_ty = ty::expr_ty(ccx.tcx, e);
    auto is_bot = ty::type_is_bot(ccx.tcx, e_ty);
    auto lv = trans_lval(cx, e);
    auto bcx = lv.res.bcx;
    auto val = lv.res.val;
    if (is_bot) {
        // For values of type _|_, we generate an
        // "undef" value, as such a value should never
        // be inspected. It's important for the value
        // to have type lldestty0 (the callee's expected type).
        val = llvm::LLVMGetUndef(lldestty0);
    } else if (arg.mode == ty::mo_val) {
        if (ty::type_owns_heap_mem(ccx.tcx, e_ty)) {
            auto dst = alloc_ty(bcx, e_ty);
            val = dst.val;
            bcx = move_val_if_temp(dst.bcx, INIT, val, lv, e_ty).bcx;
        } else if (lv.is_mem) {
            val = load_if_immediate(bcx, val, e_ty);
            bcx = copy_ty(bcx, val, e_ty).bcx;
        } else {
            // Eliding take/drop for appending of external vectors currently
            // corrupts memory. I can't figure out why, and external vectors
            // are on the way out anyway, so this simply turns off the
            // optimization for that case.
            auto is_ext_vec_plus = alt (e.node) {
                case (ast::expr_binary(_, _, _)) {
                    ty::type_is_sequence(ccx.tcx, e_ty) &&
                    !ty::sequence_is_interior(ccx.tcx, e_ty)
                }
                case (_) { false }
            };
            if (is_ext_vec_plus) { bcx = copy_ty(bcx, val, e_ty).bcx; }
            else { revoke_clean(bcx, val); }
        }
    } else if (type_is_immediate(ccx, e_ty) && !lv.is_mem) {
        val = do_spill(bcx, val);
    }

    if (!is_bot && ty::type_contains_params(ccx.tcx, arg.ty)) {
        auto lldestty = lldestty0;
        if (arg.mode == ty::mo_val
            && ty::type_is_structural(ccx.tcx, e_ty)) {
            lldestty = T_ptr(lldestty);
        }
        val = bcx.build.PointerCast(val, lldestty);
    }
    if (arg.mode == ty::mo_val
        && ty::type_is_structural(ccx.tcx, e_ty)) {
        // Until here we've been treating structures by pointer;
        // we are now passing it as an arg, so need to load it.
        val = bcx.build.Load(val);
    }
    ret rslt(bcx, val);
}


// NB: must keep 4 fns in sync:
//
//  - type_of_fn_full
//  - create_llargs_for_fn_args.
//  - new_fn_ctxt
//  - trans_args
fn trans_args(&@block_ctxt cx, ValueRef llenv, &option::t[ValueRef] llobj,
              &option::t[generic_info] gen, &option::t[ValueRef] lliterbody,
              &(@ast::expr)[] es, &ty::t fn_ty)
        -> tup(@block_ctxt, ValueRef[], ValueRef) {
    let ty::arg[] args = ty::ty_fn_args(cx.fcx.lcx.ccx.tcx, fn_ty);
    let ValueRef[] llargs = ~[];
    let ValueRef[] lltydescs = ~[];
    let @block_ctxt bcx = cx;
    // Arg 0: Output pointer.

    // FIXME: test case looks like
    // f(1, fail, @42);
    if (bcx.build.is_terminated()) {
        // This means an earlier arg was divergent.
        // So this arg can't be evaluated.
        ret tup(bcx, ~[], C_nil());
    }

    auto retty = ty::ty_fn_ret(cx.fcx.lcx.ccx.tcx, fn_ty);
    auto llretslot_res = alloc_ty(bcx, retty);
    bcx = llretslot_res.bcx;
    auto llretslot = llretslot_res.val;
    alt (gen) {
        case (some(?g)) {
            lazily_emit_all_generic_info_tydesc_glues(cx, g);
            lltydescs = g.tydescs;
            args = ty::ty_fn_args(cx.fcx.lcx.ccx.tcx, g.item_type);
            retty = ty::ty_fn_ret(cx.fcx.lcx.ccx.tcx, g.item_type);
        }
        case (_) { }
    }
    if (ty::type_has_dynamic_size(cx.fcx.lcx.ccx.tcx, retty)) {
        llargs +=
            ~[bcx.build.PointerCast(llretslot,
                                    T_typaram_ptr(cx.fcx.lcx.ccx.tn))];
    } else if (ty::type_contains_params(cx.fcx.lcx.ccx.tcx, retty)) {
        // It's possible that the callee has some generic-ness somewhere in
        // its return value -- say a method signature within an obj or a fn
        // type deep in a structure -- which the caller has a concrete view
        // of. If so, cast the caller's view of the restlot to the callee's
        // view, for the sake of making a type-compatible call.

        llargs +=
            ~[cx.build.PointerCast(llretslot,
                                   T_ptr(type_of(bcx.fcx.lcx.ccx, bcx.sp,
                                                 retty)))];
    } else { llargs += ~[llretslot]; }
    // Arg 1: task pointer.

    llargs += ~[bcx.fcx.lltaskptr];
    // Arg 2: Env (closure-bindings / self-obj)

    alt (llobj) {
        case (some(?ob)) {
            // Every object is always found in memory,
            // and not-yet-loaded (as part of an lval x.y
            // doted method-call).

            llargs += ~[bcx.build.Load(ob)];
        }
        case (_) { llargs += ~[llenv]; }
    }
    // Args >3: ty_params ...

    llargs += lltydescs;
    // ... then possibly an lliterbody argument.

    alt (lliterbody) {
        case (none) { }
        case (some(?lli)) { llargs += ~[lli]; }
    }
    // ... then explicit args.

    // First we figure out the caller's view of the types of the arguments.
    // This will be needed if this is a generic call, because the callee has
    // to cast her view of the arguments to the caller's view.

    auto arg_tys = type_of_explicit_args(cx.fcx.lcx.ccx, cx.sp, args);
    auto i = 0u;
    for (@ast::expr e in es) {
        if (bcx.build.is_terminated()) {
            // This means an earlier arg was divergent.
            // So this arg can't be evaluated.
            break;
        }
        auto r = trans_arg_expr(bcx, args.(i), arg_tys.(i), e);
        bcx = r.bcx;
        llargs += ~[r.val];
        i += 1u;
    }
    ret tup(bcx, llargs, llretslot);
}

fn trans_call(&@block_ctxt cx, &@ast::expr f, &option::t[ValueRef] lliterbody,
              &(@ast::expr)[] args, ast::node_id id) -> result {
    // NB: 'f' isn't necessarily a function; it might be an entire self-call
    // expression because of the hack that allows us to process self-calls
    // with trans_call.

    auto f_res = trans_lval_gen(cx, f);
    let ty::t fn_ty;
    alt (f_res.method_ty) {
        case (some(?meth)) {
            // self-call
            fn_ty = meth;
        }
        case (_) {
            fn_ty = ty::expr_ty(cx.fcx.lcx.ccx.tcx, f);
        }
    }

    auto bcx = f_res.res.bcx;

    auto faddr = f_res.res.val;
    auto llenv = C_null(T_opaque_closure_ptr(*cx.fcx.lcx.ccx));
    alt (f_res.llobj) {
        case (some(_)) {
            // It's a vtbl entry.
            faddr = bcx.build.Load(faddr);
        }
        case (none) {
            // It's a closure. We have to autoderef.
            if (f_res.is_mem) { faddr = load_if_immediate(bcx, faddr, fn_ty);}
            auto res = autoderef(bcx, faddr, fn_ty);
            bcx = res.bcx;
            fn_ty = res.ty;

            auto pair = res.val;
            faddr =
                bcx.build.GEP(pair, ~[C_int(0), C_int(abi::fn_field_code)]);
            faddr = bcx.build.Load(faddr);
            auto llclosure =
                bcx.build.GEP(pair, ~[C_int(0), C_int(abi::fn_field_box)]);
            llenv = bcx.build.Load(llclosure);
        }
    }

    auto ret_ty = ty::node_id_to_type(cx.fcx.lcx.ccx.tcx, id);
    auto args_res =
        trans_args(bcx, llenv, f_res.llobj, f_res.generic,
                   lliterbody, args, fn_ty);
    bcx = args_res._0;
    auto llargs = args_res._1;
    auto llretslot = args_res._2;
    /*
    log "calling: " + val_str(cx.fcx.lcx.ccx.tn, faddr);

    for (ValueRef arg in llargs) {
        log "arg: " + val_str(cx.fcx.lcx.ccx.tn, arg);
    }
    */

    /* If the block is terminated,
       then one or more of the args has
       type _|_. Since that means it diverges, the code
       for the call itself is unreachable. */
    auto retval = C_nil();
    if (!bcx.build.is_terminated()) {
        bcx.build.FastCall(faddr, llargs);
        alt (lliterbody) {
            case (none) {
                if (!ty::type_is_nil(cx.fcx.lcx.ccx.tcx, ret_ty)) {
                    retval = load_if_immediate(bcx, llretslot, ret_ty);
                    // Retval doesn't correspond to anything really tangible
                    // in the frame, but it's a ref all the same, so we put a
                    // note here to drop it when we're done in this scope.
                    add_clean_temp(cx, retval, ret_ty);
                }
            }
            case (some(_)) {
                // If there was an lliterbody, it means we were calling an
                // iter, and we are *not* the party using its 'output' value,
                // we should ignore llretslot.
            }
        }
    }
    ret rslt(bcx, retval);
}

fn trans_tup(&@block_ctxt cx, &ast::elt[] elts, ast::node_id id) -> result {
    auto bcx = cx;
    auto t = node_id_type(bcx.fcx.lcx.ccx, id);
    auto tup_res = alloc_ty(bcx, t);
    auto tup_val = tup_res.val;
    bcx = tup_res.bcx;
    add_clean_temp(cx, tup_val, t);
    let int i = 0;
    for (ast::elt e in elts) {
        auto e_ty = ty::expr_ty(cx.fcx.lcx.ccx.tcx, e.expr);
        auto src = trans_lval(bcx, e.expr);
        bcx = src.res.bcx;
        auto dst_res = GEP_tup_like(bcx, t, tup_val, ~[0, i]);
        bcx = move_val_if_temp(dst_res.bcx, INIT, dst_res.val, src, e_ty).bcx;
        i += 1;
    }
    ret rslt(bcx, tup_val);
}

fn trans_vec(&@block_ctxt cx, &(@ast::expr)[] args, ast::node_id id) ->
   result {
    auto t = node_id_type(cx.fcx.lcx.ccx, id);
    auto unit_ty = t;
    alt (ty::struct(cx.fcx.lcx.ccx.tcx, t)) {
        case (ty::ty_vec(?mt)) { unit_ty = mt.ty; }
        case (_) { cx.fcx.lcx.ccx.sess.bug("non-vec type in trans_vec"); }
    }
    auto bcx = cx;
    auto unit_sz = size_of(bcx, unit_ty);
    bcx = unit_sz.bcx;
    auto data_sz =
        bcx.build.Mul(C_uint(std::ivec::len[@ast::expr](args)), unit_sz.val);
    // FIXME: pass tydesc properly.

    auto vec_val =
        bcx.build.Call(bcx.fcx.lcx.ccx.upcalls.new_vec,
                       ~[bcx.fcx.lltaskptr, data_sz,
                         C_null(T_ptr(bcx.fcx.lcx.ccx.tydesc_type))]);
    auto llty = type_of(bcx.fcx.lcx.ccx, bcx.sp, t);
    vec_val = bcx.build.PointerCast(vec_val, llty);
    add_clean_temp(bcx, vec_val, t);
    auto body = bcx.build.GEP(vec_val, ~[C_int(0), C_int(abi::vec_elt_data)]);
    auto pseudo_tup_ty =
        ty::mk_imm_tup(cx.fcx.lcx.ccx.tcx,
                       std::ivec::init_elt[ty::t](unit_ty,
                                                  std::ivec::len(args)));
    let int i = 0;
    for (@ast::expr e in args) {
        auto src = trans_lval(bcx, e);
        bcx = src.res.bcx;
        auto dst_res = GEP_tup_like(bcx, pseudo_tup_ty, body, ~[0, i]);
        bcx = dst_res.bcx;
        // Cast the destination type to the source type. This is needed to
        // make tags work, for a subtle combination of reasons:
        //
        // (1) "dst_res" above is derived from "body", which is in turn
        //     derived from "vec_val".
        // (2) "vec_val" has the LLVM type "llty".
        // (3) "llty" is the result of calling type_of() on a vector type.
        // (4) For tags, type_of() returns a different type depending on
        //     on whether the tag is behind a box or not. Vector types are
        //     considered boxes.
        // (5) "src_res" is derived from "unit_ty", which is not behind a box.

        auto dst_val;
        if (!ty::type_has_dynamic_size(cx.fcx.lcx.ccx.tcx, unit_ty)) {
            auto llunit_ty = type_of(cx.fcx.lcx.ccx, bcx.sp, unit_ty);
            dst_val = bcx.build.PointerCast(dst_res.val, T_ptr(llunit_ty));
        } else { dst_val = dst_res.val; }
        bcx = move_val_if_temp(bcx, INIT, dst_val, src, unit_ty).bcx;
        i += 1;
    }
    auto fill = bcx.build.GEP(vec_val, ~[C_int(0), C_int(abi::vec_elt_fill)]);
    bcx.build.Store(data_sz, fill);
    ret rslt(bcx, vec_val);
}


// TODO: Move me to ivec::
fn trans_ivec(@block_ctxt bcx, &(@ast::expr)[] args, ast::node_id id) ->
        result {
    auto typ = node_id_type(bcx.fcx.lcx.ccx, id);
    auto unit_ty;
    alt (ty::struct(bcx.fcx.lcx.ccx.tcx, typ)) {
        case (ty::ty_ivec(?mt)) { unit_ty = mt.ty; }
        case (_) { bcx.fcx.lcx.ccx.sess.bug("non-ivec type in trans_ivec"); }
    }
    auto llunitty = type_of_or_i8(bcx, unit_ty);

    auto ares = ivec::alloc(bcx, unit_ty);
    bcx = ares.bcx;
    auto llvecptr = ares.llptr;
    auto unit_sz = ares.llunitsz;
    auto llalen = ares.llalen;

    add_clean_temp(bcx, llvecptr, typ);

    auto lllen = bcx.build.Mul(C_uint(std::ivec::len(args)), unit_sz);
    // Allocate the vector pieces and store length and allocated length.

    auto llfirsteltptr;
    if (std::ivec::len(args) > 0u &&
            std::ivec::len(args) <= abi::ivec_default_length) {
        // Interior case.

        bcx.build.Store(lllen,
                        bcx.build.InBoundsGEP(llvecptr,
                                              ~[C_int(0),
                                                C_uint(abi::ivec_elt_len)]));
        bcx.build.Store(llalen,
                        bcx.build.InBoundsGEP(llvecptr,
                                              ~[C_int(0),
                                                C_uint(abi::ivec_elt_alen)]));
        llfirsteltptr =
            bcx.build.InBoundsGEP(llvecptr,
                                  ~[C_int(0), C_uint(abi::ivec_elt_elems),
                                    C_int(0)]);
    } else {
        // Heap case.

        auto stub_z = ~[C_int(0), C_uint(abi::ivec_heap_stub_elt_zero)];
        auto stub_a = ~[C_int(0), C_uint(abi::ivec_heap_stub_elt_alen)];
        auto stub_p = ~[C_int(0), C_uint(abi::ivec_heap_stub_elt_ptr)];
        auto llstubty = T_ivec_heap(llunitty);
        auto llstubptr = bcx.build.PointerCast(llvecptr, T_ptr(llstubty));
        bcx.build.Store(C_int(0), bcx.build.InBoundsGEP(llstubptr, stub_z));
        auto llheapty = T_ivec_heap_part(llunitty);
        if (std::ivec::len(args) == 0u) {
            // Null heap pointer indicates a zero-length vector.

            bcx.build.Store(llalen, bcx.build.InBoundsGEP(llstubptr, stub_a));
            bcx.build.Store(C_null(T_ptr(llheapty)),
                            bcx.build.InBoundsGEP(llstubptr, stub_p));
            llfirsteltptr = C_null(T_ptr(llunitty));
        } else {
            bcx.build.Store(lllen, bcx.build.InBoundsGEP(llstubptr, stub_a));

            auto llheapsz = bcx.build.Add(llsize_of(llheapty), lllen);
            auto rslt = trans_shared_malloc(bcx, T_ptr(llheapty), llheapsz);
            bcx = rslt.bcx;
            auto llheapptr = rslt.val;
            bcx.build.Store(llheapptr,
                            bcx.build.InBoundsGEP(llstubptr, stub_p));
            auto heap_l = ~[C_int(0), C_uint(abi::ivec_heap_elt_len)];
            bcx.build.Store(lllen, bcx.build.InBoundsGEP(llheapptr, heap_l));
            llfirsteltptr =
                bcx.build.InBoundsGEP(llheapptr,
                                      ~[C_int(0),
                                        C_uint(abi::ivec_heap_elt_elems),
                                        C_int(0)]);
        }
    }
    // Store the individual elements.

    auto i = 0u;
    for (@ast::expr e in args) {
        auto lv = trans_lval(bcx, e);
        bcx = lv.res.bcx;
        auto lleltptr;
        if (ty::type_has_dynamic_size(bcx.fcx.lcx.ccx.tcx, unit_ty)) {
            lleltptr =
                bcx.build.InBoundsGEP(llfirsteltptr,
                                      ~[bcx.build.Mul(C_uint(i), unit_sz)]);
        } else {
            lleltptr = bcx.build.InBoundsGEP(llfirsteltptr, ~[C_uint(i)]);
        }
        bcx = move_val_if_temp(bcx, INIT, lleltptr, lv, unit_ty).bcx;
        i += 1u;
    }
    ret rslt(bcx, llvecptr);
}

fn trans_rec(&@block_ctxt cx, &ast::field[] fields,
             &option::t[@ast::expr] base, ast::node_id id) -> result {
    auto bcx = cx;
    auto t = node_id_type(bcx.fcx.lcx.ccx, id);
    auto rec_res = alloc_ty(bcx, t);
    auto rec_val = rec_res.val;
    bcx = rec_res.bcx;
    add_clean_temp(cx, rec_val, t);
    let int i = 0;
    auto base_val = C_nil();
    alt (base) {
        case (none) { }
        case (some(?bexp)) {
            auto base_res = trans_expr(bcx, bexp);
            bcx = base_res.bcx;
            base_val = base_res.val;
        }
    }
    let ty::field[] ty_fields = ~[];
    alt (ty::struct(cx.fcx.lcx.ccx.tcx, t)) {
        case (ty::ty_rec(?flds)) { ty_fields = flds; }
    }
    for (ty::field tf in ty_fields) {
        auto e_ty = tf.mt.ty;
        auto dst_res = GEP_tup_like(bcx, t, rec_val, ~[0, i]);
        bcx = dst_res.bcx;
        auto expr_provided = false;
        for (ast::field f in fields) {
            if (str::eq(f.node.ident, tf.ident)) {
                expr_provided = true;
                auto lv = trans_lval(bcx, f.node.expr);
                bcx = move_val_if_temp(lv.res.bcx, INIT, dst_res.val, lv,
                                       e_ty).bcx;
                break;
            }
        }
        if (!expr_provided) {
            auto src_res = GEP_tup_like(bcx, t, base_val, ~[0, i]);
            src_res =
                rslt(src_res.bcx, load_if_immediate(bcx, src_res.val, e_ty));
            bcx = copy_val(src_res.bcx, INIT, dst_res.val, src_res.val,
                           e_ty).bcx;
        }
        i += 1;
    }
    ret rslt(bcx, rec_val);
}

fn trans_expr(&@block_ctxt cx, &@ast::expr e) -> result {
    ret trans_expr_out(cx, e, return);
}

fn trans_expr_out(&@block_ctxt cx, &@ast::expr e, out_method output) ->
   result {
    // FIXME Fill in cx.sp
    alt (e.node) {
        case (ast::expr_lit(?lit)) { ret trans_lit(cx, *lit); }
        case (ast::expr_unary(?op, ?x)) {
            if (op != ast::deref) { ret trans_unary(cx, op, x, e.id); }
        }
        case (ast::expr_binary(?op, ?x, ?y)) {
            ret trans_binary(cx, op, x, y);
        }
        case (ast::expr_if(?cond, ?thn, ?els)) {
            ret with_out_method(bind trans_if(cx, cond, thn, els, e.id, _),
                                cx, e.id, output);
        }
        case (ast::expr_if_check(?cond, ?thn, ?els)) {
            ret with_out_method(bind trans_if(cx, cond, thn, els, e.id, _),
                                cx, e.id, output);
        }
        case (ast::expr_ternary(_, _, _)) {
            ret trans_expr_out(cx, ast::ternary_to_if(e), output);
        }
        case (ast::expr_for(?decl, ?seq, ?body)) {
            ret trans_for(cx, decl, seq, body);
        }
        case (ast::expr_for_each(?decl, ?seq, ?body)) {
            ret trans_for_each(cx, decl, seq, body);
        }
        case (ast::expr_while(?cond, ?body)) {
            ret trans_while(cx, cond, body);
        }
        case (ast::expr_do_while(?body, ?cond)) {
            ret trans_do_while(cx, body, cond);
        }
        case (ast::expr_alt(?expr, ?arms)) {
            ret with_out_method(bind trans_alt::trans_alt(cx, expr,
                                                          arms, e.id, _),
                                cx, e.id, output);
        }
        case (ast::expr_fn(?f)) {
            auto ccx = cx.fcx.lcx.ccx;
            let TypeRef llfnty =
                alt (ty::struct(ccx.tcx, node_id_type(ccx, e.id))) {
                    case (ty::ty_fn(?proto, ?inputs, ?output, _, _)) {
                        type_of_fn_full(ccx, e.span, proto, false, inputs,
                                        output, 0u)
                    }
                };
            auto sub_cx = extend_path(cx.fcx.lcx, ccx.names.next("anon"));
            auto s = mangle_internal_name_by_path(ccx, sub_cx.path);
            auto llfn = decl_internal_fastcall_fn(ccx.llmod, s, llfnty);

            auto fn_res = trans_closure(some(cx), some(llfnty), sub_cx,
                                        e.span, f, llfn, none, ~[], e.id);
            auto fn_pair = alt (fn_res) {
                some(?fn_pair) { fn_pair }
                none { create_fn_pair(ccx, s, llfnty, llfn, false) }
            };
            ret rslt(cx, fn_pair);
        }
        case (ast::expr_block(?blk)) {
            auto sub_cx = new_scope_block_ctxt(cx, "block-expr body");
            auto next_cx = new_sub_block_ctxt(cx, "next");
            auto sub =
                with_out_method(bind trans_block(sub_cx, blk, _), cx, e.id,
                                output);
            cx.build.Br(sub_cx.llbb);
            sub.bcx.build.Br(next_cx.llbb);
            ret rslt(next_cx, sub.val);
        }
        case (ast::expr_move(?dst, ?src)) {
            auto lhs_res = trans_lval(cx, dst);
            assert (lhs_res.is_mem);
            // FIXME Fill in lhs_res.res.bcx.sp

            auto rhs_res = trans_lval(lhs_res.res.bcx, src);
            auto t = ty::expr_ty(cx.fcx.lcx.ccx.tcx, src);
            // FIXME: calculate copy init-ness in typestate.

            auto move_res =
                move_val(rhs_res.res.bcx, DROP_EXISTING, lhs_res.res.val,
                         rhs_res, t);
            ret rslt(move_res.bcx, C_nil());
        }
        case (ast::expr_assign(?dst, ?src)) {
            auto lhs_res = trans_lval(cx, dst);
            assert (lhs_res.is_mem);
            // FIXME Fill in lhs_res.res.bcx.sp
            auto rhs = trans_lval(lhs_res.res.bcx, src);
            auto t = ty::expr_ty(cx.fcx.lcx.ccx.tcx, src);
            // FIXME: calculate copy init-ness in typestate.
            auto copy_res = move_val_if_temp
                (rhs.res.bcx, DROP_EXISTING, lhs_res.res.val, rhs, t);
            ret rslt(copy_res.bcx, C_nil());
        }
        case (ast::expr_swap(?dst, ?src)) {
            auto lhs_res = trans_lval(cx, dst);
            assert (lhs_res.is_mem);
            // FIXME Fill in lhs_res.res.bcx.sp

            auto rhs_res = trans_lval(lhs_res.res.bcx, src);
            auto t = ty::expr_ty(cx.fcx.lcx.ccx.tcx, src);
            auto tmp_res = alloc_ty(rhs_res.res.bcx, t);
            // Swap through a temporary.

            auto move1_res =
                memmove_ty(tmp_res.bcx, tmp_res.val, lhs_res.res.val, t);
            auto move2_res =
                memmove_ty(move1_res.bcx, lhs_res.res.val, rhs_res.res.val,
                           t);
            auto move3_res =
                memmove_ty(move2_res.bcx, rhs_res.res.val, tmp_res.val, t);
            ret rslt(move3_res.bcx, C_nil());
        }
        case (ast::expr_assign_op(?op, ?dst, ?src)) {
            auto t = ty::expr_ty(cx.fcx.lcx.ccx.tcx, src);
            auto lhs_res = trans_lval(cx, dst);
            assert (lhs_res.is_mem);
            // FIXME Fill in lhs_res.res.bcx.sp

            auto rhs_res = trans_expr(lhs_res.res.bcx, src);
            if (ty::type_is_sequence(cx.fcx.lcx.ccx.tcx, t)) {
                alt (op) {
                    case (ast::add) {
                        if (ty::sequence_is_interior(cx.fcx.lcx.ccx.tcx, t)) {
                            ret ivec::trans_append(rhs_res.bcx, t,
                                                   lhs_res.res.val,
                                                   rhs_res.val);
                        }
                        ret trans_vec_append(rhs_res.bcx, t, lhs_res.res.val,
                                             rhs_res.val);
                    }
                    case (_) { }
                }
            }
            auto lhs_val = load_if_immediate(rhs_res.bcx, lhs_res.res.val, t);
            auto v =
                trans_eager_binop(rhs_res.bcx, op, t, lhs_val, rhs_res.val);
            // FIXME: calculate copy init-ness in typestate.
            // This is always a temporary, so can always be safely moved
            auto move_res = move_val(v.bcx, DROP_EXISTING, lhs_res.res.val,
                                     lval_val(v.bcx, v.val), t);
            ret rslt(move_res.bcx, C_nil());
        }
        case (ast::expr_bind(?f, ?args)) {
            ret trans_bind(cx, f, args, e.id);
        }
        case (ast::expr_call(?f, ?args)) {
            ret trans_call(cx, f, none[ValueRef], args, e.id);
        }
        case (ast::expr_cast(?val, _)) { ret trans_cast(cx, val, e.id); }
        case (ast::expr_vec(?args, _, ast::sk_rc)) {
            ret trans_vec(cx, args, e.id);
        }
        case (ast::expr_vec(?args, _, ast::sk_unique)) {
            ret trans_ivec(cx, args, e.id);
        }
        case (ast::expr_tup(?args)) { ret trans_tup(cx, args, e.id); }
        case (ast::expr_rec(?args, ?base)) {
            ret trans_rec(cx, args, base, e.id);
        }
        case (ast::expr_mac(_)) {
            ret cx.fcx.lcx.ccx.sess.bug("unexpanded macro");
        }
        case (ast::expr_fail(?expr)) {
            ret trans_fail_expr(cx, some(e.span), expr);
        }
        case (ast::expr_log(?lvl, ?a)) { ret trans_log(lvl, cx, a); }
        case (ast::expr_assert(?a)) {
            ret trans_check_expr(cx, a, "Assertion");
        }
        case (ast::expr_check(ast::checked, ?a)) {
            ret trans_check_expr(cx, a, "Predicate");
        }
        case (ast::expr_check(ast::unchecked, ?a)) {
            /* Claims are turned on and off by a global variable
               that the RTS sets. This case generates code to
               check the value of that variable, doing nothing
               if it's set to false and acting like a check
               otherwise. */
            auto c = get_extern_const(cx.fcx.lcx.ccx.externs,
                                      cx.fcx.lcx.ccx.llmod,
                                      "check_claims", T_bool());
            auto cond = cx.build.Load(c);

            auto then_cx   = new_scope_block_ctxt(cx, "claim_then");
            auto check_res = trans_check_expr(then_cx, a, "Claim");
            auto else_cx = new_scope_block_ctxt(cx, "else");
            auto els = rslt(else_cx, C_nil());

            cx.build.CondBr(cond, then_cx.llbb, else_cx.llbb);
            ret rslt(join_branches(cx, ~[check_res, els]), C_nil());
        }
        case (ast::expr_break) { ret trans_break(e.span, cx); }
        case (ast::expr_cont) { ret trans_cont(e.span, cx); }
        case (ast::expr_ret(?ex)) { ret trans_ret(cx, ex); }
        case (ast::expr_put(?ex)) { ret trans_put(cx, ex); }
        case (ast::expr_be(?ex)) { ret trans_be(cx, ex); }
        case (ast::expr_port(_)) { ret trans_port(cx, e.id); }
        case (ast::expr_chan(?ex)) { ret trans_chan(cx, ex, e.id); }
        case (ast::expr_send(?lhs, ?rhs)) {
            ret trans_send(cx, lhs, rhs, e.id);
        }
        case (ast::expr_recv(?lhs, ?rhs)) {
            ret trans_recv(cx, lhs, rhs, e.id);
        }
        case (ast::expr_spawn(?dom, ?name, ?func, ?args)) {
            ret trans_spawn(cx, dom, name, func, args, e.id);
        }
        case (ast::expr_anon_obj(?anon_obj)) {
            ret trans_anon_obj(cx, e.span, anon_obj, e.id);
        }
        case (_) {
            // The expression is an lvalue. Fall through.
            assert (ty::is_lval(e)); // make sure it really is and that we
                               // didn't forget to add a case for a new expr!
        }
    }
    // lval cases fall through to trans_lval and then
    // possibly load the result (if it's non-structural).

    auto t = ty::expr_ty(cx.fcx.lcx.ccx.tcx, e);
    auto sub = trans_lval(cx, e);
    auto v = sub.res.val;
    if (sub.is_mem) { v = load_if_immediate(sub.res.bcx, v, t); }
    ret rslt(sub.res.bcx, v);
}

fn with_out_method(fn(&out_method) -> result  work, @block_ctxt cx,
                   ast::node_id id, &out_method outer_output) -> result {
    auto ccx = cx.fcx.lcx.ccx;
    if (outer_output != return) {
        ret work(outer_output);
    } else {
        auto tp = node_id_type(ccx, id);
        if (ty::type_is_nil(ccx.tcx, tp)) { ret work(return); }
        auto res_alloca = alloc_ty(cx, tp);
        cx = zero_alloca(res_alloca.bcx, res_alloca.val, tp).bcx;
        fn drop_hoisted_ty(&@block_ctxt cx, ValueRef target, ty::t t) ->
           result {
            auto reg_val = load_if_immediate(cx, target, t);
            ret drop_ty(cx, reg_val, t);
        }
        auto done = work(save_in(res_alloca.val));
        auto loaded = load_if_immediate(done.bcx, res_alloca.val, tp);
        add_clean_temp(cx, loaded, tp);
        ret rslt(done.bcx, loaded);;
    }
}


// We pass structural values around the compiler "by pointer" and
// non-structural values (scalars, boxes, pointers) "by value". We call the
// latter group "immediates" and, in some circumstances when we know we have a
// pointer (or need one), perform load/store operations based on the
// immediate-ness of the type.
fn type_is_immediate(&@crate_ctxt ccx, &ty::t t) -> bool {
    ret ty::type_is_scalar(ccx.tcx, t) || ty::type_is_boxed(ccx.tcx, t) ||
            ty::type_is_native(ccx.tcx, t);
}

fn do_spill(&@block_ctxt cx, ValueRef v) -> ValueRef {
    // We have a value but we have to spill it to pass by alias.

    auto llptr = alloca(cx, val_ty(v));
    cx.build.Store(v, llptr);
    ret llptr;
}

fn spill_if_immediate(&@block_ctxt cx, ValueRef v, &ty::t t) -> ValueRef {
    if (type_is_immediate(cx.fcx.lcx.ccx, t)) { ret do_spill(cx, v); }
    ret v;
}

fn load_if_immediate(&@block_ctxt cx, ValueRef v, &ty::t t) -> ValueRef {
    if (type_is_immediate(cx.fcx.lcx.ccx, t)) { ret cx.build.Load(v); }
    ret v;
}

fn trans_log(int lvl, &@block_ctxt cx, &@ast::expr e) -> result {
    auto lcx = cx.fcx.lcx;
    auto modname = str::connect_ivec(lcx.module_path, "::");
    auto global;
    if (lcx.ccx.module_data.contains_key(modname)) {
        global = lcx.ccx.module_data.get(modname);
    } else {
        auto s =
            link::mangle_internal_name_by_path_and_seq(lcx.ccx,
                                                       lcx.module_path,
                                                       "loglevel");
        global = llvm::LLVMAddGlobal(lcx.ccx.llmod, T_int(), str::buf(s));
        llvm::LLVMSetGlobalConstant(global, False);
        llvm::LLVMSetInitializer(global, C_null(T_int()));
        llvm::LLVMSetLinkage(global,
                             lib::llvm::LLVMInternalLinkage as llvm::Linkage);
        lcx.ccx.module_data.insert(modname, global);
    }
    auto log_cx = new_scope_block_ctxt(cx, "log");
    auto after_cx = new_sub_block_ctxt(cx, "after");
    auto load = cx.build.Load(global);
    auto test = cx.build.ICmp(lib::llvm::LLVMIntSGE, load, C_int(lvl));
    cx.build.CondBr(test, log_cx.llbb, after_cx.llbb);
    auto sub = trans_expr(log_cx, e);
    auto e_ty = ty::expr_ty(cx.fcx.lcx.ccx.tcx, e);
    auto log_bcx = sub.bcx;
    if (ty::type_is_fp(cx.fcx.lcx.ccx.tcx, e_ty)) {
        let TypeRef tr;
        let bool is32bit = false;
        alt (ty::struct(cx.fcx.lcx.ccx.tcx, e_ty)) {
            case (ty::ty_machine(ast::ty_f32)) {
                tr = T_f32();
                is32bit = true;
            }
            case (ty::ty_machine(ast::ty_f64)) { tr = T_f64(); }
            case (_) { tr = T_float(); }
        }
        if (is32bit) {
            log_bcx.build.Call(log_bcx.fcx.lcx.ccx.upcalls.log_float,
                               ~[log_bcx.fcx.lltaskptr, C_int(lvl), sub.val]);
        } else {
            // FIXME: Eliminate this level of indirection.

            auto tmp = alloca(log_bcx, tr);
            sub.bcx.build.Store(sub.val, tmp);
            log_bcx.build.Call(log_bcx.fcx.lcx.ccx.upcalls.log_double,
                               ~[log_bcx.fcx.lltaskptr, C_int(lvl), tmp]);
        }
    } else if (ty::type_is_integral(cx.fcx.lcx.ccx.tcx, e_ty) ||
                   ty::type_is_bool(cx.fcx.lcx.ccx.tcx, e_ty)) {
        // FIXME: Handle signedness properly.

        auto llintval =
            int_cast(log_bcx, T_int(), val_ty(sub.val), sub.val, false);
        log_bcx.build.Call(log_bcx.fcx.lcx.ccx.upcalls.log_int,
                           ~[log_bcx.fcx.lltaskptr, C_int(lvl), llintval]);
    } else {
        alt (ty::struct(cx.fcx.lcx.ccx.tcx, e_ty)) {
            case (ty::ty_str) {
                log_bcx.build.Call(log_bcx.fcx.lcx.ccx.upcalls.log_str,
                                   ~[log_bcx.fcx.lltaskptr, C_int(lvl),
                                     sub.val]);
            }
            case (_) {
                // FIXME: Support these types.

                cx.fcx.lcx.ccx.sess.span_fatal(e.span,
                                             "log called on unsupported type "
                                                 +
                                                 ty_to_str(cx.fcx.lcx.ccx.tcx,
                                                           e_ty));
            }
        }
    }
    log_bcx = trans_block_cleanups(log_bcx, log_cx);
    log_bcx.build.Br(after_cx.llbb);
    ret rslt(after_cx, C_nil());
}

fn trans_check_expr(&@block_ctxt cx, &@ast::expr e, &str s) -> result {
    auto cond_res = trans_expr(cx, e);
    auto expr_str = s + " " + expr_to_str(e) + " failed";
    auto fail_cx = new_sub_block_ctxt(cx, "fail");
    trans_fail(fail_cx, some[span](e.span), expr_str);
    auto next_cx = new_sub_block_ctxt(cx, "next");
    cond_res.bcx.build.CondBr(cond_res.val, next_cx.llbb, fail_cx.llbb);
    ret rslt(next_cx, C_nil());
}

fn trans_fail_expr(&@block_ctxt cx, &option::t[span] sp_opt,
                   &option::t[@ast::expr] fail_expr)
        -> result {
    auto bcx = cx;
    alt (fail_expr) {
        case (some(?expr)) {
            auto tcx = bcx.fcx.lcx.ccx.tcx;
            auto expr_res = trans_expr(bcx, expr);
            auto e_ty = ty::expr_ty(tcx, expr);
            bcx = expr_res.bcx;

            if (ty::type_is_str(tcx, e_ty)) {
                auto elt = bcx.build.GEP(expr_res.val,
                                         ~[C_int(0),
                                           C_int(abi::vec_elt_data)]);
                ret trans_fail_value(bcx, sp_opt, elt);
            } else {
                cx.fcx.lcx.ccx.sess.span_bug(expr.span,
                                             "fail called with unsupported \
                                              type " + ty_to_str(tcx, e_ty));
            }
        }
        case (_) {
            ret trans_fail(bcx, sp_opt, "explicit failure");
        }
    }
}

fn trans_fail(&@block_ctxt cx, &option::t[span] sp_opt, &str fail_str)
   -> result {
    auto V_fail_str = C_cstr(cx.fcx.lcx.ccx, fail_str);
    ret trans_fail_value(cx, sp_opt, V_fail_str);
}

fn trans_fail_value(&@block_ctxt cx, &option::t[span] sp_opt,
                    &ValueRef V_fail_str)
        -> result {
    auto V_filename;
    auto V_line;
    alt (sp_opt) {
        case (some(?sp)) {
            auto loc = cx.fcx.lcx.ccx.sess.lookup_pos(sp.lo);
            V_filename = C_cstr(cx.fcx.lcx.ccx, loc.filename);
            V_line = loc.line as int;
        }
        case (none) {
            V_filename = C_cstr(cx.fcx.lcx.ccx, "<runtime>");
            V_line = 0;
        }
    }
    auto V_str = cx.build.PointerCast(V_fail_str, T_ptr(T_i8()));
    V_filename = cx.build.PointerCast(V_filename, T_ptr(T_i8()));
    auto args = ~[cx.fcx.lltaskptr, V_str, V_filename, C_int(V_line)];
    cx.build.Call(cx.fcx.lcx.ccx.upcalls._fail, args);
    cx.build.Unreachable();
    ret rslt(cx, C_nil());
}

fn trans_put(&@block_ctxt cx, &option::t[@ast::expr] e) -> result {
    auto llcallee = C_nil();
    auto llenv = C_nil();
    alt ({ cx.fcx.lliterbody }) {
        case (some(?lli)) {
            auto slot = alloca(cx, val_ty(lli));
            cx.build.Store(lli, slot);
            llcallee =
                cx.build.GEP(slot, ~[C_int(0), C_int(abi::fn_field_code)]);
            llcallee = cx.build.Load(llcallee);
            llenv = cx.build.GEP(slot, ~[C_int(0), C_int(abi::fn_field_box)]);
            llenv = cx.build.Load(llenv);
        }
    }
    auto bcx = cx;
    auto dummy_retslot = alloca(bcx, T_nil());
    let ValueRef[] llargs = ~[dummy_retslot, cx.fcx.lltaskptr, llenv];
    alt (e) {
        case (none) { }
        case (some(?x)) {
            auto e_ty = ty::expr_ty(cx.fcx.lcx.ccx.tcx, x);
            auto arg = rec(mode=ty::mo_alias(false), ty=e_ty);
            auto arg_tys =
                type_of_explicit_args(cx.fcx.lcx.ccx, x.span, ~[arg]);
            auto r = trans_arg_expr(bcx, arg, arg_tys.(0), x);
            bcx = r.bcx;
            llargs += ~[r.val];
        }
    }
    ret rslt(bcx, bcx.build.FastCall(llcallee, llargs));
}

fn trans_break_cont(&span sp, &@block_ctxt cx, bool to_end) -> result {
    auto bcx = cx;
    // Locate closest loop block, outputting cleanup as we go.

    auto cleanup_cx = cx;
    while (true) {
        bcx = trans_block_cleanups(bcx, cleanup_cx);
        alt ({ cleanup_cx.kind }) {
            case (LOOP_SCOPE_BLOCK(?_cont, ?_break)) {
                if (to_end) {
                    bcx.build.Br(_break.llbb);
                } else {
                    alt (_cont) {
                        case (option::some(?_cont)) {
                            bcx.build.Br(_cont.llbb);
                        }
                        case (_) { bcx.build.Br(cleanup_cx.llbb); }
                    }
                }
                ret rslt(new_sub_block_ctxt(bcx, "break_cont.unreachable"),
                        C_nil());
            }
            case (_) {
                alt ({ cleanup_cx.parent }) {
                    case (parent_some(?cx)) { cleanup_cx = cx; }
                    case (parent_none) {
                        cx.fcx.lcx.ccx.sess.span_fatal(sp,
                                                     if (to_end) {
                                                         "Break"
                                                     } else { "Cont" } +
                                                         " outside a loop");
                    }
                }
            }
        }
    }
    // If we get here without returning, it's a bug

    cx.fcx.lcx.ccx.sess.bug("in trans::trans_break_cont()");
}

fn trans_break(&span sp, &@block_ctxt cx) -> result {
    ret trans_break_cont(sp, cx, true);
}

fn trans_cont(&span sp, &@block_ctxt cx) -> result {
    ret trans_break_cont(sp, cx, false);
}

fn trans_ret(&@block_ctxt cx, &option::t[@ast::expr] e) -> result {
    auto bcx = cx;
    alt (e) {
        case (some(?x)) {
            auto t = ty::expr_ty(cx.fcx.lcx.ccx.tcx, x);
            auto lv = trans_lval(cx, x);
            bcx = lv.res.bcx;
            bcx = move_val_if_temp(bcx, INIT, cx.fcx.llretptr, lv, t).bcx;
        }
        case (_) {
            auto t = llvm::LLVMGetElementType(val_ty(cx.fcx.llretptr));
            bcx.build.Store(C_null(t), cx.fcx.llretptr);
        }
    }
    // run all cleanups and back out.

    let bool more_cleanups = true;
    auto cleanup_cx = cx;
    while (more_cleanups) {
        bcx = trans_block_cleanups(bcx, cleanup_cx);
        alt ({ cleanup_cx.parent }) {
            case (parent_some(?b)) { cleanup_cx = b; }
            case (parent_none) { more_cleanups = false; }
        }
    }
    bcx.build.RetVoid();
    ret rslt(new_sub_block_ctxt(bcx, "ret.unreachable"), C_nil());
}

fn trans_be(&@block_ctxt cx, &@ast::expr e) -> result {
    // FIXME: This should be a typestate precondition

    assert (ast::is_call_expr(e));
    // FIXME: Turn this into a real tail call once
    // calling convention issues are settled

    ret trans_ret(cx, some(e));
}

/*

  Suppose we create an anonymous object my_b from a regular object a:

        obj a() {
            fn foo() -> int {
                ret 2;
            }
            fn bar() -> int {
                ret self.foo();
            }
        }

       auto my_a = a();
       auto my_b = obj { fn baz() -> int { ret self.foo() } with my_a };

  Here we're extending the my_a object with an additional method baz, creating
  an object my_b. Since it's an object, my_b is a pair of a vtable pointer and
  a body pointer:

  my_b: [vtbl* | body*]

  my_b's vtable has entries for foo, bar, and baz, whereas my_a's vtable has
  only foo and bar. my_b's 3-entry vtable consists of two forwarding functions
  and one real method.

  my_b's body just contains the pair a: [ a_vtable | a_body ], wrapped up with
  any additional fields that my_b added. None were added, so my_b is just the
  wrapped inner object.

*/

// trans_anon_obj: create and return a pointer to an object.  This code
// differs from trans_obj in that, rather than creating an object constructor
// function and putting it in the generated code as an object item, we are
// instead "inlining" the construction of the object and returning the object
// itself.
fn trans_anon_obj(@block_ctxt bcx, &span sp, &ast::anon_obj anon_obj,
                  ast::node_id id) -> result {


    auto ccx = bcx.fcx.lcx.ccx;

    // Fields.
    // FIXME (part of issue #538): Where do we fill in the field *values* from
    // the outer object?
    let ast::anon_obj_field[] additional_fields = ~[];
    let result[] additional_field_vals = ~[];
    let ty::t[] additional_field_tys = ~[];
    alt (anon_obj.fields) {
        case (none) { }
        case (some(?fields)) {
            additional_fields = fields;
            for (ast::anon_obj_field f in fields) {
                additional_field_tys += ~[node_id_type(ccx, f.id)];
                additional_field_vals += ~[trans_expr(bcx, f.expr)];
            }
        }
    }

    // Get the type of the eventual entire anonymous object, possibly with
    // extensions.  NB: This type includes both inner and outer methods.
    auto outer_obj_ty = ty::node_id_to_type(ccx.tcx, id);

    // Create a vtable for the anonymous object.

    // create_vtbl() wants an ast::_obj and all we have is an ast::anon_obj,
    // so we need to roll our own.
    let ast::_obj wrapper_obj = rec(
        fields = std::ivec::map(ast::obj_field_from_anon_obj_field,
                                additional_fields),
        methods = anon_obj.methods,
        dtor = none[@ast::method]);

    let ty::t with_obj_ty;
    auto vtbl;
    alt (anon_obj.with_obj) {
        case (none) {
            // If there's no with_obj -- that is, if we're just adding new
            // fields rather than extending an existing object -- then we just
            // pass the outer object to create_vtbl().  Our vtable won't need
            // to have any forwarding slots.

            // We need a dummy with_obj_ty for setting up the object body
            // later.
            with_obj_ty = ty::mk_type(ccx.tcx);

            // This seems a little strange, because it'll come into
            // create_vtbl() with no "additional methods".  What's happening
            // is that, since *all* of the methods are "additional", we can
            // get away with acting like none of them are.
            vtbl = create_vtbl(bcx.fcx.lcx, sp, outer_obj_ty,
                               wrapper_obj, ~[], none,
                               additional_field_tys);
        }
        case (some(?e)) {
            // TODO: What makes more sense to get the type of an expr --
            // calling ty::expr_ty(ccx.tcx, e) on it or calling
            // ty::node_id_to_type(ccx.tcx, id) on its id?
            with_obj_ty = ty::expr_ty(ccx.tcx, e);
            //with_obj_ty = ty::node_id_to_type(ccx.tcx, e.id);

            // If there's a with_obj, we pass its type along to create_vtbl().
            // Part of what create_vtbl() will do is take the set difference
            // of methods defined on the original and methods being added.
            // For every method defined on the original that does *not* have
            // one with a matching name and type being added, we'll need to
            // create a forwarding slot.  And, of course, we need to create a
            // normal vtable entry for every method being added.
            vtbl = create_vtbl(bcx.fcx.lcx, sp, outer_obj_ty,
                               wrapper_obj, ~[], some(with_obj_ty),
                               additional_field_tys);
        }
    }

    // Allocate the object that we're going to return.
    auto pair = alloca(bcx, ccx.rust_object_type);

    // Take care of cleanups.
    auto t = node_id_type(ccx, id);
    add_clean_temp(bcx, pair, t);

    // Grab onto the first and second elements of the pair.
    // abi::obj_field_vtbl and abi::obj_field_box simply specify words 0 and 1
    // of 'pair'.
    auto pair_vtbl =
        bcx.build.GEP(pair, ~[C_int(0), C_int(abi::obj_field_vtbl)]);
    auto pair_box =
        bcx.build.GEP(pair, ~[C_int(0), C_int(abi::obj_field_box)]);

    vtbl = bcx.build.PointerCast(vtbl, T_ptr(T_empty_struct()));
    bcx.build.Store(vtbl, pair_vtbl);

    // Next we have to take care of the other half of the pair we're
    // returning: a boxed (reference-counted) tuple containing a tydesc,
    // typarams, fields, and a pointer to our with_obj.
    let TypeRef llbox_ty = T_ptr(T_empty_struct());

    if (std::ivec::len[ast::anon_obj_field](additional_fields) == 0u &&
        anon_obj.with_obj == none) {
        // If the object we're translating has no fields and no with_obj,
        // there's not much to do.
        bcx.build.Store(C_null(llbox_ty), pair_box);
    } else {

        // Synthesize a tuple type for fields: [field, ...]
        let ty::t fields_ty = ty::mk_imm_tup(ccx.tcx, additional_field_tys);

        // Type for tydescs.
        let ty::t tydesc_ty = ty::mk_type(ccx.tcx);

        // Placeholder for non-existent typarams, since anon objs don't have
        // them.
        let ty::t typarams_ty = ty::mk_imm_tup(ccx.tcx, ~[]);

        // Tuple type for body:
        // [tydesc, [typaram, ...], [field, ...], with_obj]
        let ty::t body_ty =
            ty::mk_imm_tup(ccx.tcx, ~[tydesc_ty, typarams_ty,
                                      fields_ty, with_obj_ty]);

        // Hand this type we've synthesized off to trans_malloc_boxed, which
        // allocates a box, including space for a refcount.
        auto box = trans_malloc_boxed(bcx, body_ty);
        bcx = box.bcx;

        // mk_imm_box throws a refcount into the type we're synthesizing,
        // so that it looks like:
        // [rc, [tydesc, [typaram, ...], [field, ...], with_obj]]
        let ty::t boxed_body_ty = ty::mk_imm_box(ccx.tcx, body_ty);

        // Grab onto the refcount and body parts of the box we allocated.
        auto rc =
            GEP_tup_like(bcx, boxed_body_ty, box.val,
                         ~[0, abi::box_rc_field_refcnt]);
        bcx = rc.bcx;
        auto body =
            GEP_tup_like(bcx, boxed_body_ty, box.val,
                         ~[0, abi::box_rc_field_body]);
        bcx = body.bcx;
        bcx.build.Store(C_int(1), rc.val);

        // Put together a tydesc for the body, so that the object can later be
        // freed by calling through its tydesc.

        // Every object (not just those with type parameters) needs to have a
        // tydesc to describe its body, since all objects have unknown type to
        // the user of the object.  So the tydesc is needed to keep track of
        // the types of the object's fields, so that the fields can be freed
        // later.
        auto body_tydesc =
            GEP_tup_like(bcx, body_ty, body.val,
                         ~[0, abi::obj_body_elt_tydesc]);
        bcx = body_tydesc.bcx;
        auto ti = none[@tydesc_info];
        auto body_td = get_tydesc(bcx, body_ty, true, ti);
        lazily_emit_tydesc_glue(bcx, abi::tydesc_field_drop_glue, ti);
        lazily_emit_tydesc_glue(bcx, abi::tydesc_field_free_glue, ti);
        bcx = body_td.bcx;
        bcx.build.Store(body_td.val, body_tydesc.val);

        // Copy the object's fields into the space we allocated for the object
        // body.  (This is something like saving the lexical environment of a
        // function in its closure: the fields were passed to the object
        // constructor and are now available to the object's methods.
        auto body_fields =
            GEP_tup_like(bcx, body_ty, body.val,
                         ~[0, abi::obj_body_elt_fields]);
        bcx = body_fields.bcx;
        let int i = 0;
        for (ast::anon_obj_field f in additional_fields) {
            // FIXME (part of issue #538): make this work eventually, when we
            // have additional field exprs in the AST.
            load_if_immediate(
                bcx,
                additional_field_vals.(i).val,
                additional_field_tys.(i));

            auto field =
                GEP_tup_like(bcx, fields_ty, body_fields.val, ~[0, i]);
            bcx = field.bcx;
            bcx = copy_val(bcx, INIT, field.val,
                           additional_field_vals.(i).val,
                           additional_field_tys.(i)).bcx;
            i += 1;
        }

        // If there's a with_obj, copy a pointer to it into the object's body.
        alt (anon_obj.with_obj) {
            case (none) { }
            case (some(?e)) {
                // If with_obj (the object being extended) exists, translate
                // it.  Translating with_obj returns a ValueRef (pointer to a
                // 2-word value) wrapped in a result.
                let result with_obj_val = trans_expr(bcx, e);

                auto body_with_obj =
                    GEP_tup_like(bcx, body_ty, body.val,
                                 ~[0, abi::obj_body_elt_with_obj]);
                bcx = body_with_obj.bcx;
                bcx = copy_val(bcx, INIT, body_with_obj.val,
                               with_obj_val.val, with_obj_ty).bcx;
            }
        }

        // Store box ptr in outer pair.
        auto p = bcx.build.PointerCast(box.val, llbox_ty);
        bcx.build.Store(p, pair_box);
    }

    // return the object we built.
    ret rslt(bcx, pair);
}

fn init_local(&@block_ctxt cx, &@ast::local local) -> result {
    // Make a note to drop this slot on the way out.

    assert (cx.fcx.lllocals.contains_key(local.node.id));
    auto llptr = cx.fcx.lllocals.get(local.node.id);
    auto ty = node_id_type(cx.fcx.lcx.ccx, local.node.id);
    auto bcx = cx;
    add_clean(cx, llptr, ty);
    alt (local.node.init) {
        case (some(?init)) {
            alt (init.op) {
                case (ast::init_assign) {
                    // Use the type of the RHS because if it's _|_, the LHS
                    // type might be something else, but we don't want to copy
                    // the value.

                    ty =
                        node_id_type(cx.fcx.lcx.ccx, init.expr.id);
                    auto sub = trans_lval(bcx, init.expr);
                    bcx = move_val_if_temp(sub.res.bcx, INIT, llptr,
                                           sub, ty).bcx;
                }
                case (ast::init_move) {
                    auto sub = trans_lval(bcx, init.expr);
                    bcx = move_val(sub.res.bcx, INIT, llptr, sub, ty).bcx;
                }
            }
        }
        case (_) { bcx = zero_alloca(bcx, llptr, ty).bcx; }
    }
    ret rslt(bcx, llptr);
}

fn zero_alloca(&@block_ctxt cx, ValueRef llptr, ty::t t) -> result {
    auto bcx = cx;
    if (ty::type_has_dynamic_size(cx.fcx.lcx.ccx.tcx, t)) {
        auto llsz = size_of(bcx, t);
        auto llalign = align_of(llsz.bcx, t);
        bcx = call_bzero(llalign.bcx, llptr, llsz.val, llalign.val).bcx;
    } else {
        auto llty = type_of(bcx.fcx.lcx.ccx, cx.sp, t);
        bcx.build.Store(C_null(llty), llptr);
    }
    ret rslt(bcx, llptr);
}

fn trans_stmt(&@block_ctxt cx, &ast::stmt s) -> result {
    // FIXME Fill in cx.sp

    auto bcx = cx;
    alt (s.node) {
        case (ast::stmt_expr(?e, _)) { bcx = trans_expr(cx, e).bcx; }
        case (ast::stmt_decl(?d, _)) {
            alt (d.node) {
                case (ast::decl_local(?locals)) {
                  for (@ast::local local in locals) {
                      bcx = init_local(bcx, local).bcx;
                  }
                }
                case (ast::decl_item(?i)) { trans_item(cx.fcx.lcx, *i); }
            }
        }
        case (_) { cx.fcx.lcx.ccx.sess.unimpl("stmt variant"); }
    }
    ret rslt(bcx, C_nil());
}

fn new_builder(BasicBlockRef llbb) -> builder {
    let BuilderRef llbuild = llvm::LLVMCreateBuilder();
    llvm::LLVMPositionBuilderAtEnd(llbuild, llbb);
    ret builder(llbuild, @mutable false);
}


// You probably don't want to use this one. See the
// next three functions instead.
fn new_block_ctxt(&@fn_ctxt cx, &block_parent parent, block_kind kind,
                  &str name) -> @block_ctxt {
    let cleanup[] cleanups = ~[];
    auto s = str::buf("");
    if (cx.lcx.ccx.sess.get_opts().save_temps ||
            cx.lcx.ccx.sess.get_opts().debuginfo) {
        s = str::buf(cx.lcx.ccx.names.next(name));
    }
    let BasicBlockRef llbb = llvm::LLVMAppendBasicBlock(cx.llfn, s);
    ret @rec(llbb=llbb,
             build=new_builder(llbb),
             parent=parent,
             kind=kind,
             mutable cleanups=cleanups,
             sp=cx.sp,
             fcx=cx);
}


// Use this when you're at the top block of a function or the like.
fn new_top_block_ctxt(&@fn_ctxt fcx) -> @block_ctxt {
    ret new_block_ctxt(fcx, parent_none, SCOPE_BLOCK, "function top level");
}


// Use this when you're at a curly-brace or similar lexical scope.
fn new_scope_block_ctxt(&@block_ctxt bcx, &str n) -> @block_ctxt {
    ret new_block_ctxt(bcx.fcx, parent_some(bcx), SCOPE_BLOCK, n);
}

fn new_loop_scope_block_ctxt(&@block_ctxt bcx, &option::t[@block_ctxt] _cont,
                             &@block_ctxt _break, &str n) -> @block_ctxt {
    ret new_block_ctxt(bcx.fcx, parent_some(bcx),
                       LOOP_SCOPE_BLOCK(_cont, _break), n);
}


// Use this when you're making a general CFG BB within a scope.
fn new_sub_block_ctxt(&@block_ctxt bcx, &str n) -> @block_ctxt {
    ret new_block_ctxt(bcx.fcx, parent_some(bcx), NON_SCOPE_BLOCK, n);
}

fn new_raw_block_ctxt(&@fn_ctxt fcx, BasicBlockRef llbb) -> @block_ctxt {
    let cleanup[] cleanups = ~[];
    ret @rec(llbb=llbb,
             build=new_builder(llbb),
             parent=parent_none,
             kind=NON_SCOPE_BLOCK,
             mutable cleanups=cleanups,
             sp=fcx.sp,
             fcx=fcx);
}


// trans_block_cleanups: Go through all the cleanups attached to this
// block_ctxt and execute them.
//
// When translating a block that introdces new variables during its scope, we
// need to make sure those variables go out of scope when the block ends.  We
// do that by running a 'cleanup' function for each variable.
// trans_block_cleanups runs all the cleanup functions for the block.
fn trans_block_cleanups(&@block_ctxt cx, &@block_ctxt cleanup_cx)
        -> @block_ctxt {
    auto bcx = cx;
    if (cleanup_cx.kind == NON_SCOPE_BLOCK) {
        assert (std::ivec::len[cleanup](cleanup_cx.cleanups) == 0u);
    }
    auto i = std::ivec::len[cleanup](cleanup_cx.cleanups);
    while (i > 0u) {
        i -= 1u;
        auto c = cleanup_cx.cleanups.(i);
        alt (c) {
            case (clean(?cfn)) { bcx = cfn(bcx).bcx; }
            case (clean_temp(_, ?cfn)) { bcx = cfn(bcx).bcx; }
        }
    }
    ret bcx;
}

iter block_locals(&ast::blk b) -> @ast::local {
    // FIXME: putting from inside an iter block doesn't work, so we can't
    // use the index here.
    for (@ast::stmt s in b.node.stmts) {
        alt (s.node) {
            case (ast::stmt_decl(?d, _)) {
                alt (d.node) {
                    case (ast::decl_local(?locals)) {
                      for (@ast::local local in locals) {
                          put local;
                      }
                    }
                    case (_) {/* fall through */ }
                }
            }
            case (_) {/* fall through */ }
        }
    }
}

fn llstaticallocas_block_ctxt(&@fn_ctxt fcx) -> @block_ctxt {
    let cleanup[] cleanups = ~[];
    ret @rec(llbb=fcx.llstaticallocas,
             build=new_builder(fcx.llstaticallocas),
             parent=parent_none,
             kind=SCOPE_BLOCK,
             mutable cleanups=cleanups,
             sp=fcx.sp,
             fcx=fcx);
}

fn llderivedtydescs_block_ctxt(&@fn_ctxt fcx) -> @block_ctxt {
    let cleanup[] cleanups = ~[];
    ret @rec(llbb=fcx.llderivedtydescs,
             build=new_builder(fcx.llderivedtydescs),
             parent=parent_none,
             kind=SCOPE_BLOCK,
             mutable cleanups=cleanups,
             sp=fcx.sp,
             fcx=fcx);
}

fn lldynamicallocas_block_ctxt(&@fn_ctxt fcx) -> @block_ctxt {
    let cleanup[] cleanups = ~[];
    ret @rec(llbb=fcx.lldynamicallocas,
             build=new_builder(fcx.lldynamicallocas),
             parent=parent_none,
             kind=SCOPE_BLOCK,
             mutable cleanups=cleanups,
             sp=fcx.sp,
             fcx=fcx);
}



fn alloc_ty(&@block_ctxt cx, &ty::t t) -> result {
    auto val = C_int(0);
    if (ty::type_has_dynamic_size(cx.fcx.lcx.ccx.tcx, t)) {
        // NB: we have to run this particular 'size_of' in a
        // block_ctxt built on the llderivedtydescs block for the fn,
        // so that the size dominates the array_alloca that
        // comes next.

        auto n = size_of(llderivedtydescs_block_ctxt(cx.fcx), t);
        cx.fcx.llderivedtydescs = n.bcx.llbb;
        val = array_alloca(cx, T_i8(), n.val);
    } else { val = alloca(cx, type_of(cx.fcx.lcx.ccx, cx.sp, t)); }
    // NB: since we've pushed all size calculations in this
    // function up to the alloca block, we actually return the
    // block passed into us unmodified; it doesn't really
    // have to be passed-and-returned here, but it fits
    // past caller conventions and may well make sense again,
    // so we leave it as-is.

    ret rslt(cx, val);
}

fn alloc_local(&@block_ctxt cx, &@ast::local local) -> result {
    auto t = node_id_type(cx.fcx.lcx.ccx, local.node.id);
    auto r = alloc_ty(cx, t);
    if (cx.fcx.lcx.ccx.sess.get_opts().debuginfo) {
        llvm::LLVMSetValueName(r.val, str::buf(local.node.ident));
    }
    r.bcx.fcx.lllocals.insert(local.node.id, r.val);
    ret r;
}

fn trans_block(&@block_ctxt cx, &ast::blk b, &out_method output) -> result {
    auto bcx = cx;
    for each (@ast::local local in block_locals(b)) {
        // FIXME Update bcx.sp
        bcx = alloc_local(bcx, local).bcx;
    }
    auto r = rslt(bcx, C_nil());
    for (@ast::stmt s in b.node.stmts) {
        r = trans_stmt(bcx, *s);
        bcx = r.bcx;

        // If we hit a terminator, control won't go any further so
        // we're in dead-code land. Stop here.
        if (is_terminated(bcx)) { ret r; }
    }
    fn accept_out_method(&@ast::expr expr) -> bool {
        ret alt (expr.node) {
                case (ast::expr_if(_, _, _)) { true }
                case (ast::expr_alt(_, _)) { true }
                case (ast::expr_block(_)) { true }
                case (_) { false }
            };
    }
    alt (b.node.expr) {
        case (some(?e)) {
            auto ccx = cx.fcx.lcx.ccx;
            auto r_ty = ty::expr_ty(ccx.tcx, e);
            auto pass = output != return && accept_out_method(e);
            if (pass) {
                r = trans_expr_out(bcx, e, output);
                bcx = r.bcx;
                if (is_terminated(bcx) || ty::type_is_bot(ccx.tcx, r_ty)) {
                    ret r;
                }
            } else {
                auto lv = trans_lval(bcx, e);
                r = lv.res;
                bcx = r.bcx;
                if (is_terminated(bcx) || ty::type_is_bot(ccx.tcx, r_ty)) {
                    ret r;
                }
                alt (output) {
                    case (save_in(?target)) {
                        // The output method is to save the value at target,
                        // and we didn't pass it to the recursive trans_expr
                        // call.
                        bcx = move_val_if_temp(bcx, INIT, target,
                                               lv, r_ty).bcx;
                        r = rslt(bcx, C_nil());
                    }
                    case (return) { }
                }
            }
        }
        case (none) { r = rslt(bcx, C_nil()); }
    }
    bcx = trans_block_cleanups(bcx, find_scope_cx(bcx));
    ret rslt(bcx, r.val);
}

fn new_local_ctxt(&@crate_ctxt ccx) -> @local_ctxt {
    let str[] pth = ~[];
    ret @rec(path=pth,
             module_path=~[ccx.link_meta.name],
             obj_typarams=~[],
             obj_fields=~[],
             ccx=ccx);
}


// Creates the standard quartet of basic blocks: static allocas, copy args,
// derived tydescs, and dynamic allocas.
fn mk_standard_basic_blocks(ValueRef llfn) ->
   tup(BasicBlockRef, BasicBlockRef, BasicBlockRef, BasicBlockRef) {
    ret tup(llvm::LLVMAppendBasicBlock(llfn, str::buf("static_allocas")),
            llvm::LLVMAppendBasicBlock(llfn, str::buf("copy_args")),
            llvm::LLVMAppendBasicBlock(llfn, str::buf("derived_tydescs")),
            llvm::LLVMAppendBasicBlock(llfn, str::buf("dynamic_allocas")));
}


// NB: must keep 4 fns in sync:
//
//  - type_of_fn_full
//  - create_llargs_for_fn_args.
//  - new_fn_ctxt
//  - trans_args
fn new_fn_ctxt(@local_ctxt cx, &span sp, ValueRef llfndecl) -> @fn_ctxt {
    let ValueRef llretptr = llvm::LLVMGetParam(llfndecl, 0u);
    let ValueRef lltaskptr = llvm::LLVMGetParam(llfndecl, 1u);
    let ValueRef llenv = llvm::LLVMGetParam(llfndecl, 2u);
    let hashmap[ast::node_id, ValueRef] llargs = new_int_hash[ValueRef]();
    let hashmap[ast::node_id, ValueRef] llobjfields =
        new_int_hash[ValueRef]();
    let hashmap[ast::node_id, ValueRef] lllocals = new_int_hash[ValueRef]();
    let hashmap[ast::node_id, ValueRef] llupvars = new_int_hash[ValueRef]();
    auto derived_tydescs =
        map::mk_hashmap[ty::t, derived_tydesc_info](ty::hash_ty, ty::eq_ty);
    auto llbbs = mk_standard_basic_blocks(llfndecl);
    ret @rec(llfn=llfndecl,
             lltaskptr=lltaskptr,
             llenv=llenv,
             llretptr=llretptr,
             mutable llstaticallocas=llbbs._0,
             mutable llcopyargs=llbbs._1,
             mutable llderivedtydescs_first=llbbs._2,
             mutable llderivedtydescs=llbbs._2,
             mutable lldynamicallocas=llbbs._3,
             mutable llself=none[val_self_pair],
             mutable lliterbody=none[ValueRef],
             llargs=llargs,
             llobjfields=llobjfields,
             lllocals=lllocals,
             llupvars=llupvars,
             mutable lltydescs=~[],
             derived_tydescs=derived_tydescs,
             sp=sp,
             lcx=cx);
}


// NB: must keep 4 fns in sync:
//
//  - type_of_fn_full
//  - create_llargs_for_fn_args.
//  - new_fn_ctxt
//  - trans_args

// create_llargs_for_fn_args: Creates a mapping from incoming arguments to
// allocas created for them.
//
// When we translate a function, we need to map its incoming arguments to the
// spaces that have been created for them (by code in the llallocas field of
// the function's fn_ctxt).  create_llargs_for_fn_args populates the llargs
// field of the fn_ctxt with
fn create_llargs_for_fn_args(&@fn_ctxt cx, ast::proto proto,
                             option::t[ty::t] ty_self, ty::t ret_ty,
                             &ast::arg[] args,
                             &ast::ty_param[] ty_params) {
    // Skip the implicit arguments 0, 1, and 2.  TODO: Pull out 3u and define
    // it as a constant, since we're using it in several places in trans this
    // way.

    auto arg_n = 3u;
    alt (ty_self) {
        case (some(?tt)) {
            cx.llself = some[val_self_pair](rec(v=cx.llenv, t=tt));
        }
        case (none) {
            auto i = 0u;
            for (ast::ty_param tp in ty_params) {
                auto llarg = llvm::LLVMGetParam(cx.llfn, arg_n);
                assert (llarg as int != 0);
                cx.lltydescs += ~[llarg];
                arg_n += 1u;
                i += 1u;
            }
        }
    }
    // If the function is actually an iter, populate the lliterbody field of
    // the function context with the ValueRef that we get from
    // llvm::LLVMGetParam for the iter's body.

    if (proto == ast::proto_iter) {
        auto llarg = llvm::LLVMGetParam(cx.llfn, arg_n);
        assert (llarg as int != 0);
        cx.lliterbody = some[ValueRef](llarg);
        arg_n += 1u;
    }

    // Populate the llargs field of the function context with the ValueRefs
    // that we get from llvm::LLVMGetParam for each argument.
    for (ast::arg arg in args) {
        auto llarg = llvm::LLVMGetParam(cx.llfn, arg_n);
        assert (llarg as int != 0);
        cx.llargs.insert(arg.id, llarg);
        arg_n += 1u;
    }
}


// Recommended LLVM style, strange though this is, is to copy from args to
// allocas immediately upon entry; this permits us to GEP into structures we
// were passed and whatnot. Apparently mem2reg will mop up.
fn copy_any_self_to_alloca(@fn_ctxt fcx) {
    auto bcx = llstaticallocas_block_ctxt(fcx);
    alt ({ fcx.llself }) {
        case (some(?pair)) {
            auto a = alloca(bcx, fcx.lcx.ccx.rust_object_type);
            bcx.build.Store(pair.v, a);
            fcx.llself = some[val_self_pair](rec(v=a, t=pair.t));
        }
        case (_) { }
    }
}

fn copy_args_to_allocas(@fn_ctxt fcx, &ast::arg[] args, &ty::arg[] arg_tys) {
    auto bcx = new_raw_block_ctxt(fcx, fcx.llcopyargs);
    let uint arg_n = 0u;
    for (ast::arg aarg in args) {
        if (aarg.mode == ast::val) {
            auto arg_t = type_of_arg(bcx.fcx.lcx, fcx.sp, arg_tys.(arg_n));
            auto a = alloca(bcx, arg_t);
            auto argval;
            alt (bcx.fcx.llargs.find(aarg.id)) {
                case (some(?x)) { argval = x; }
                case (_) { bcx.fcx.lcx.ccx.sess.span_fatal(aarg.ty.span,
                         "unbound arg ID in copy_args_to_allocas"); }
            }
            bcx.build.Store(argval, a);
            // Overwrite the llargs entry for this arg with its alloca.

            bcx.fcx.llargs.insert(aarg.id, a);
        }
        arg_n += 1u;
    }
}

fn add_cleanups_for_args(&@block_ctxt bcx, &ast::arg[] args,
                         &ty::arg[] arg_tys) {
    let uint arg_n = 0u;
    for (ast::arg aarg in args) {
        if (aarg.mode == ast::val) {
            auto argval;
            alt (bcx.fcx.llargs.find(aarg.id)) {
                case (some(?x)) { argval = x; }
                case (_) { bcx.fcx.lcx.ccx.sess.span_fatal(aarg.ty.span,
                      "unbound arg ID in copy_args_to_allocas"); }
            }
            add_clean(bcx, argval, arg_tys.(arg_n).ty);
        }
        arg_n += 1u;
    }
}

fn is_terminated(&@block_ctxt cx) -> bool {
    auto inst = llvm::LLVMGetLastInstruction(cx.llbb);
    ret llvm::LLVMIsATerminatorInst(inst) as int != 0;
}

fn arg_tys_of_fn(&@crate_ctxt ccx,ast::node_id id) -> ty::arg[] {
    alt (ty::struct(ccx.tcx, ty::node_id_to_type(ccx.tcx, id))) {
        case (ty::ty_fn(_, ?arg_tys, _, _, _)) { ret arg_tys; }
    }
}

fn populate_fn_ctxt_from_llself(@fn_ctxt fcx, val_self_pair llself) {
    auto bcx = llstaticallocas_block_ctxt(fcx);
    let ty::t[] field_tys = ~[];
    for (ast::obj_field f in bcx.fcx.lcx.obj_fields) {
        field_tys += ~[node_id_type(bcx.fcx.lcx.ccx, f.id)];
    }
    // Synthesize a tuple type for the fields so that GEP_tup_like() can work
    // its magic.

    auto fields_tup_ty = ty::mk_imm_tup(fcx.lcx.ccx.tcx, field_tys);
    auto n_typarams = std::ivec::len[ast::ty_param](bcx.fcx.lcx.obj_typarams);
    let TypeRef llobj_box_ty = T_obj_ptr(*bcx.fcx.lcx.ccx, n_typarams);
    auto box_cell =
        bcx.build.GEP(llself.v, ~[C_int(0), C_int(abi::obj_field_box)]);
    auto box_ptr = bcx.build.Load(box_cell);
    box_ptr = bcx.build.PointerCast(box_ptr, llobj_box_ty);
    auto obj_typarams =
        bcx.build.GEP(box_ptr,
                      ~[C_int(0), C_int(abi::box_rc_field_body),
                       C_int(abi::obj_body_elt_typarams)]);
    // The object fields immediately follow the type parameters, so we skip
    // over them to get the pointer.

    auto et = llvm::LLVMGetElementType(val_ty(obj_typarams));
    auto obj_fields = bcx.build.Add(vp2i(bcx, obj_typarams), llsize_of(et));
    // If we can (i.e. the type is statically sized), then cast the resulting
    // fields pointer to the appropriate LLVM type. If not, just leave it as
    // i8 *.

    if (!ty::type_has_dynamic_size(fcx.lcx.ccx.tcx, fields_tup_ty)) {
        auto llfields_ty = type_of(fcx.lcx.ccx, fcx.sp, fields_tup_ty);
        obj_fields = vi2p(bcx, obj_fields, T_ptr(llfields_ty));
    } else { obj_fields = vi2p(bcx, obj_fields, T_ptr(T_i8())); }
    let int i = 0;
    for (ast::ty_param p in fcx.lcx.obj_typarams) {
        let ValueRef lltyparam =
            bcx.build.GEP(obj_typarams, ~[C_int(0), C_int(i)]);
        lltyparam = bcx.build.Load(lltyparam);
        fcx.lltydescs += ~[lltyparam];
        i += 1;
    }
    i = 0;
    for (ast::obj_field f in fcx.lcx.obj_fields) {
        auto rslt = GEP_tup_like(bcx, fields_tup_ty, obj_fields, ~[0, i]);
        bcx = llstaticallocas_block_ctxt(fcx);
        auto llfield = rslt.val;
        fcx.llobjfields.insert(f.id, llfield);
        i += 1;
    }
    fcx.llstaticallocas = bcx.llbb;
}


// Ties up the llstaticallocas -> llcopyargs -> llderivedtydescs ->
// lldynamicallocas -> lltop edges.
fn finish_fn(&@fn_ctxt fcx, BasicBlockRef lltop) {
    new_builder(fcx.llstaticallocas).Br(fcx.llcopyargs);
    new_builder(fcx.llcopyargs).Br(fcx.llderivedtydescs_first);
    new_builder(fcx.llderivedtydescs).Br(fcx.lldynamicallocas);
    new_builder(fcx.lldynamicallocas).Br(lltop);
}

// trans_closure: Builds an LLVM function out of a source function.
// If the function closes over its environment a closure will be
// returned.
fn trans_closure(&option::t[@block_ctxt] bcx_maybe,
                 &option::t[TypeRef] llfnty,
                 @local_ctxt cx,
                 &span sp, &ast::_fn f, ValueRef llfndecl,
                 option::t[ty::t] ty_self,
                 &ast::ty_param[] ty_params, ast::node_id id)
    -> option::t[ValueRef] {
    set_uwtable(llfndecl);

    // Set up arguments to the function.
    auto fcx = new_fn_ctxt(cx, sp, llfndecl);
    create_llargs_for_fn_args(fcx, f.proto, ty_self,
                              ty::ret_ty_of_fn(cx.ccx.tcx, id),
                              f.decl.inputs, ty_params);
    copy_any_self_to_alloca(fcx);
    alt ({ fcx.llself }) {
        some(?llself) { populate_fn_ctxt_from_llself(fcx, llself); }
        _ { }
    }
    auto arg_tys = arg_tys_of_fn(fcx.lcx.ccx, id);
    copy_args_to_allocas(fcx, f.decl.inputs, arg_tys);

    // Figure out if we need to build a closure and act accordingly
    auto closure = none;
    alt(f.proto) {
        ast::proto_block {
            auto bcx = option::get(bcx_maybe);
            auto upvars = get_freevars(cx.ccx.tcx, id);

            auto llenv = build_environment(bcx, upvars);

            // Generate code to load the environment out of the
            // environment pointer.
            load_environment(bcx, fcx, llenv.ptrty, upvars);
            // Build the closure.
            closure = some(create_real_fn_pair(bcx, option::get(llfnty),
                                               llfndecl, llenv.ptr));
        }
        ast::proto_closure {
            fail "copy capture not implemented yet";
        }
        _ {}
    }

    // Create the first basic block in the function and keep a handle on it to
    //  pass to finish_fn later.
    auto bcx = new_top_block_ctxt(fcx);
    add_cleanups_for_args(bcx, f.decl.inputs, arg_tys);
    auto lltop = bcx.llbb;
    auto block_ty = node_id_type(cx.ccx, f.body.node.id);

    if (cx.ccx.sess.get_opts().dps) {
        // Call into the new destination-passing-style translation engine.
        auto dest = trans_dps::dest_move(cx.ccx.tcx, fcx.llretptr, block_ty);
        bcx = trans_dps::trans_block(bcx, dest, f.body);
    } else {
        // This call to trans_block is the place where we bridge between
        // translation calls that don't have a return value (trans_crate,
        // trans_mod, trans_item, trans_obj, et cetera) and those that do
        // (trans_block, trans_expr, et cetera).
        auto rslt =
            if (!ty::type_is_nil(cx.ccx.tcx, block_ty) &&
                    !ty::type_is_bot(cx.ccx.tcx, block_ty)) {
                trans_block(bcx, f.body, save_in(fcx.llretptr))
            } else { trans_block(bcx, f.body, return) };
        bcx = rslt.bcx;
    }

    if (!is_terminated(bcx)) {
        // FIXME: until LLVM has a unit type, we are moving around
        // C_nil values rather than their void type.
       bcx.build.RetVoid();
    }

    // Insert the mandatory first few basic blocks before lltop.
    finish_fn(fcx, lltop);

    ret closure;
}

fn trans_fn_inner(@local_ctxt cx, &span sp, &ast::_fn f, ValueRef llfndecl,
                  option::t[ty::t] ty_self, &ast::ty_param[] ty_params,
                  ast::node_id id) {
    trans_closure(none, none, cx, sp, f, llfndecl, ty_self, ty_params, id);
}


// trans_fn: creates an LLVM function corresponding to a source language
// function.
fn trans_fn(@local_ctxt cx, &span sp, &ast::_fn f, ValueRef llfndecl,
            option::t[ty::t] ty_self, &ast::ty_param[] ty_params,
            ast::node_id id) {
    if !cx.ccx.sess.get_opts().stats {
        trans_fn_inner(cx, sp, f, llfndecl, ty_self, ty_params, id);
        ret;
    }

    auto start = time::get_time();
    trans_fn_inner(cx, sp, f, llfndecl, ty_self, ty_params, id);
    auto end = time::get_time();
    log_fn_time(cx.ccx, str::connect_ivec(cx.path, "::"), start, end);
}

// process_fwding_mthd: Create the forwarding function that appears in a
// vtable slot for method calls that "fall through" to an inner object.  A
// helper function for create_vtbl.
fn process_fwding_mthd(@local_ctxt cx, &span sp, @ty::method m,
                       &ast::ty_param[] ty_params,
                       ty::t with_obj_ty,
                       &ty::t[] additional_field_tys) -> ValueRef {


    // The method m is being called on the outer object, but the outer object
    // doesn't have that method; only the inner object does.  So what we have
    // to do is synthesize that method on the outer object.  It has to take
    // all the same arguments as the method on the inner object does, then
    // call m with those arguments on the inner object, and then return the
    // value returned from that call.  It's like an eta-expansion around m,
    // except we also have to pass the inner object that m should be called
    // on.  That object won't exist until run-time, but we know its type
    // statically.

    // Create a local context that's aware of the name of the method we're
    // creating.
    let @local_ctxt mcx =
        @rec(path=cx.path + ~["method", m.ident] with *cx);

    // Make up a name for the forwarding function.
    let str s = mangle_internal_name_by_path_and_seq(mcx.ccx, mcx.path,
                                                     "forwarding_fn");

    // Get the forwarding function's type and declare it.
    let TypeRef llforwarding_fn_ty =
        type_of_fn_full(
            cx.ccx, sp, m.proto,
            true, m.inputs, m.output,
            std::ivec::len[ast::ty_param](ty_params));
    let ValueRef llforwarding_fn =
        decl_internal_fastcall_fn(cx.ccx.llmod, s, llforwarding_fn_ty);

    // Create a new function context and block context for the forwarding
    // function, holding onto a pointer to the first block.
    auto fcx = new_fn_ctxt(cx, sp, llforwarding_fn);
    auto bcx = new_top_block_ctxt(fcx);
    auto lltop = bcx.llbb;

    // The outer object will arrive in the forwarding function via the llenv
    // argument.  Put it in an alloca so that we can GEP into it later.
    auto llself_obj_ptr = alloca(bcx, fcx.lcx.ccx.rust_object_type);
    bcx.build.Store(fcx.llenv, llself_obj_ptr);

    // Grab hold of the outer object so we can pass it into the inner object,
    // in case that inner object needs to make any self-calls.  (Such calls
    // will need to dispatch back through the outer object.)
    auto llself_obj = bcx.build.Load(llself_obj_ptr);

    // The 'llretptr' that will arrive in the forwarding function we're
    // creating also needs to be the correct size.  Cast it to the size of the
    // method's return type, if necessary.
    auto llretptr = fcx.llretptr;
    if (ty::type_has_dynamic_size(cx.ccx.tcx, m.output)) {
        llretptr = bcx.build.PointerCast(llretptr,
                                         T_typaram_ptr(cx.ccx.tn));
    }

    // Now, we have to get the the with_obj's vtbl out of the self_obj.  This
    // is a multi-step process:

    // First, grab the box out of the self_obj.  It contains a refcount and a
    // body.
    auto llself_obj_box =
        bcx.build.GEP(llself_obj_ptr, ~[C_int(0),
                                        C_int(abi::obj_field_box)]);
    llself_obj_box = bcx.build.Load(llself_obj_box);

    auto ccx = bcx.fcx.lcx.ccx;
    auto llbox_ty = T_opaque_obj_ptr(*ccx);
    llself_obj_box = bcx.build.PointerCast(llself_obj_box, llbox_ty);

    // Now, reach into the box and grab the body.
    auto llself_obj_body =
        bcx.build.GEP(llself_obj_box, ~[C_int(0),
                                        C_int(abi::box_rc_field_body)]);

    // Now, we need to figure out exactly what type the body is supposed to be
    // cast to.

    // NB: This next part is almost flat-out copypasta from trans_anon_obj.
    // It would be great to factor this out.

    // Synthesize a tuple type for fields: [field, ...]
    let ty::t fields_ty = ty::mk_imm_tup(cx.ccx.tcx, additional_field_tys);

    // Type for tydescs.
    let ty::t tydesc_ty = ty::mk_type(cx.ccx.tcx);

    // Placeholder for non-existent typarams, since anon objs don't have them.
    let ty::t typarams_ty = ty::mk_imm_tup(cx.ccx.tcx, ~[]);

    // Tuple type for body:
    // [tydesc, [typaram, ...], [field, ...], with_obj]
    let ty::t body_ty =
        ty::mk_imm_tup(cx.ccx.tcx, ~[tydesc_ty, typarams_ty,
                                     fields_ty, with_obj_ty]);

    // And cast to that type.
    llself_obj_body = bcx.build.PointerCast(llself_obj_body,
                                            T_ptr(type_of(cx.ccx, sp,
                                                          body_ty)));

    // Now, reach into the body and grab the with_obj.
    auto llwith_obj =
        GEP_tup_like(bcx,
                     body_ty,
                     llself_obj_body,
                     ~[0, abi::obj_body_elt_with_obj]);
    bcx = llwith_obj.bcx;

    // And, now, somewhere in with_obj is a vtable with an entry for the
    // method we want.  First, pick out the vtable, and then pluck that
    // method's entry out of the vtable so that the forwarding function can
    // call it.
    auto llwith_obj_vtbl =
        bcx.build.GEP(llwith_obj.val, ~[C_int(0),
                                        C_int(abi::obj_field_vtbl)]);
    llwith_obj_vtbl = bcx.build.Load(llwith_obj_vtbl);

    // Get the index of the method we want.
    let uint ix = 0u;
    alt (ty::struct(bcx.fcx.lcx.ccx.tcx, with_obj_ty)) {
        case (ty::ty_obj(?methods)) {
            ix = ty::method_idx(cx.ccx.sess, sp, m.ident, methods);
        }
        case (_) {
            // Shouldn't happen.
            cx.ccx.sess.bug("process_fwding_mthd(): non-object type passed "
                            + "as with_obj_ty");
        }
    }

    // Pick out the original method from the vtable.  The +1 is because slot
    // #0 contains the destructor.
    auto vtbl_type = T_ptr(T_array(T_ptr(T_nil()), ix + 2u));
    llwith_obj_vtbl = bcx.build.PointerCast(llwith_obj_vtbl, vtbl_type);

    auto llorig_mthd = bcx.build.GEP(llwith_obj_vtbl,
                                     ~[C_int(0), C_int(ix + 1u as int)]);

    // Set up the original method to be called.
    auto orig_mthd_ty = ty::method_ty_to_fn_ty(cx.ccx.tcx, *m);
    auto llorig_mthd_ty =
        type_of_fn_full(bcx.fcx.lcx.ccx, sp,
                        ty::ty_fn_proto(bcx.fcx.lcx.ccx.tcx, orig_mthd_ty),
                        true,
                        m.inputs,
                        m.output,
                        std::ivec::len[ast::ty_param](ty_params));
    llorig_mthd = bcx.build.PointerCast(llorig_mthd,
                                        T_ptr(T_ptr(llorig_mthd_ty)));
    llorig_mthd = bcx.build.Load(llorig_mthd);

    // Set up the three implicit arguments to the original method we'll need
    // to call.
    let ValueRef[] llorig_mthd_args = ~[llretptr, fcx.lltaskptr, llself_obj];

    // Copy the explicit arguments that are being passed into the forwarding
    // function (they're in fcx.llargs) to llorig_mthd_args.

    let uint a = 3u; // retptr, task ptr, env come first
    let ValueRef passed_arg = llvm::LLVMGetParam(llforwarding_fn, a);
    for (ty::arg arg in m.inputs) {
        if (arg.mode == ty::mo_val) {
            passed_arg = load_if_immediate(bcx, passed_arg, arg.ty);
        }
        llorig_mthd_args += ~[passed_arg];
        a += 1u;
    }

    // And, finally, call the original method.
    bcx.build.FastCall(llorig_mthd, llorig_mthd_args);

    bcx.build.RetVoid();
    finish_fn(fcx, lltop);

    ret llforwarding_fn;
}

// process_normal_mthd: Create the contents of a normal vtable slot.  A helper
// function for create_vtbl.
fn process_normal_mthd(@local_ctxt cx, @ast::method m,
                       ty::t self_ty, &ast::ty_param[] ty_params)
    -> ValueRef {

    auto llfnty = T_nil();
    alt (ty::struct(cx.ccx.tcx, node_id_type(cx.ccx, m.node.id))){
        case (ty::ty_fn(?proto, ?inputs, ?output, _, _)) {
            llfnty =
                type_of_fn_full(
                    cx.ccx, m.span, proto,
                    true, inputs, output,
                    std::ivec::len[ast::ty_param](ty_params));
        }
    }
    let @local_ctxt mcx =
        @rec(path=cx.path + ~["method", m.node.ident] with *cx);
    let str s = mangle_internal_name_by_path(mcx.ccx, mcx.path);
    let ValueRef llfn =
        decl_internal_fastcall_fn(cx.ccx.llmod, s, llfnty);

    // Every method on an object gets its node_id inserted into the
    // crate-wide item_ids map, together with the ValueRef that points to
    // where that method's definition will be in the executable.
    cx.ccx.item_ids.insert(m.node.id, llfn);
    cx.ccx.item_symbols.insert(m.node.id, s);
    trans_fn(mcx, m.span, m.node.meth, llfn,
             some(self_ty),
             ty_params, m.node.id);

    ret llfn;
}

// Create a vtable for an object being translated.  Returns a pointer into
// read-only memory.
fn create_vtbl(@local_ctxt cx, &span sp, ty::t self_ty,
               &ast::_obj ob, &ast::ty_param[] ty_params,
               option::t[ty::t] with_obj_ty,
               &ty::t[] additional_field_tys) -> ValueRef {

    // Used only inside create_vtbl to distinguish different kinds of slots
    // we'll have to create.
    tag vtbl_mthd {
        // Normal methods are complete AST nodes, but for forwarding methods,
        // the only information we'll have about them is their type.
        normal_mthd(@ast::method);
        fwding_mthd(@ty::method);
    }

    auto dtor = C_null(T_ptr(T_i8()));
    alt (ob.dtor) {
        case (some(?d)) {
            auto dtor_1 = trans_dtor(cx, self_ty, ty_params, d);
            dtor = llvm::LLVMConstBitCast(dtor_1, val_ty(dtor));
        }
        case (none) { }
    }

    let ValueRef[] llmethods = ~[dtor];
    let vtbl_mthd[] meths = ~[];

    alt (with_obj_ty) {
        case (none) {
            // If there's no with_obj, then we don't need any forwarding
            // slots.  Just use the object's regular methods.
            for (@ast::method m in ob.methods) { meths += ~[normal_mthd(m)]; }
        }
        case (some(?with_obj_ty)) {
            // Handle forwarding slots.

            // If this vtable is being created for an extended object, then
            // the vtable needs to contain 'forwarding slots' for methods that
            // were on the original object and are not being overloaded by the
            // extended one.  So, to find the set of methods that we need
            // forwarding slots for, we need to take the set difference of
            // with_obj_methods (methods on the original object) and
            // ob.methods (methods on the object being added).

            // If we're here, then with_obj_ty and llwith_obj_ty are the type
            // of the inner object, and "ob" is the wrapper object.  We need
            // to take apart with_obj_ty (it had better have an object type
            // with methods!) and put those original methods onto the list of
            // methods we need forwarding methods for.

            // Gather up methods on the original object in 'meths'.
            alt (ty::struct(cx.ccx.tcx, with_obj_ty)) {
                case (ty::ty_obj(?with_obj_methods)) {
                    for (ty::method m in with_obj_methods) {
                        meths += ~[fwding_mthd(@m)];
                    }
                }
                case (_) {
                    // Shouldn't happen.
                    cx.ccx.sess.bug("create_vtbl(): trying to extend a "
                                    + "non-object");
                }
            }

            // Now, filter out any methods that we don't need forwarding slots
            // for, because they're being replaced.
            fn filtering_fn(@local_ctxt cx, &vtbl_mthd m,
                            (@ast::method)[] addtl_meths)
                -> option::t[vtbl_mthd] {

                alt (m) {
                    case (fwding_mthd(?fm)) {
                        // Since fm is a fwding_mthd, and we're checking to
                        // see if it's in addtl_meths (which only contains
                        // normal_mthds), we can't just check if fm is a
                        // member of addtl_meths.  Instead, we have to go
                        // through addtl_meths and see if there's some method
                        // in it that has the same name as fm.

                        // FIXME (part of #543): We're only checking names
                        // here.  If a method is replacing another, it also
                        // needs to have the same type, but this should
                        // probably be enforced in typechecking.
                        for (@ast::method am in addtl_meths) {
                            if (str::eq(am.node.ident, fm.ident)) {
                                ret none;
                            }
                        }
                        ret some(fwding_mthd(fm));
                    }
                    case (normal_mthd(_)) {
                        // Should never happen.
                        cx.ccx.sess.bug("create_vtbl(): shouldn't be any"
                                        + " normal_mthds in meths here");
                    }
                }
            }
            auto f = bind filtering_fn(cx, _, ob.methods);
            meths = std::ivec::filter_map[vtbl_mthd, vtbl_mthd](f, meths);

            // And now add the additional ones (both replacements and entirely
            // new ones).  These'll just be normal methods.
            for (@ast::method m in ob.methods) {
                meths += ~[normal_mthd(m)];
            }
        }
    }

    // Sort all the methods.
    fn vtbl_mthd_lteq(&vtbl_mthd a, &vtbl_mthd b) -> bool {
        alt (a) {
            case (normal_mthd(?ma)) {
                alt (b) {
                    case (normal_mthd(?mb)) {
                        ret str::lteq(ma.node.ident, mb.node.ident);
                    }
                    case (fwding_mthd(?mb)) {
                        ret str::lteq(ma.node.ident, mb.ident);
                    }
                }
            }
            case (fwding_mthd(?ma)) {
                alt (b) {
                    case (normal_mthd(?mb)) {
                        ret str::lteq(ma.ident, mb.node.ident);
                    }
                    case (fwding_mthd(?mb)) {
                        ret str::lteq(ma.ident, mb.ident);
                    }
                }
            }
        }
    }
    meths = std::sort::ivector::merge_sort[vtbl_mthd]
        (bind vtbl_mthd_lteq(_, _), meths);

    // Now that we have our list of methods, we can process them in order.
    for (vtbl_mthd m in meths) {
        alt (m) {
            case (normal_mthd(?nm)) {
                llmethods += ~[process_normal_mthd(cx, nm, self_ty,
                                                   ty_params)];
            }
            // If we have to process a forwarding method, then we need to know
            // about the with_obj's type as well as the outer object's type.
            case (fwding_mthd(?fm)) {
                alt (with_obj_ty) {
                    case (none) {
                        // This shouldn't happen; if we're trying to process a
                        // forwarding method, then we should always have a
                        // with_obj_ty.
                        cx.ccx.sess.bug("create_vtbl(): trying to create "
                                        + "forwarding method without a type "
                                        + "of object to forward to");
                    }
                    case (some(?t)) {
                        llmethods += ~[process_fwding_mthd(
                                cx, sp, fm, ty_params, t,
                                additional_field_tys)];
                    }
                }
            }
        }
    }

    auto vtbl = C_struct(llmethods);
    auto vtbl_name = mangle_internal_name_by_path(cx.ccx,
                                                  cx.path + ~["vtbl"]);
    auto gvar =
        llvm::LLVMAddGlobal(cx.ccx.llmod, val_ty(vtbl), str::buf(vtbl_name));
    llvm::LLVMSetInitializer(gvar, vtbl);
    llvm::LLVMSetGlobalConstant(gvar, True);
    llvm::LLVMSetLinkage(gvar,
                         lib::llvm::LLVMInternalLinkage as llvm::Linkage);
    ret gvar;
}

fn trans_dtor(@local_ctxt cx, ty::t self_ty,
              &ast::ty_param[] ty_params, &@ast::method dtor) -> ValueRef {
    auto llfnty = T_dtor(cx.ccx, dtor.span);
    let str s = mangle_internal_name_by_path(cx.ccx, cx.path + ~["drop"]);
    let ValueRef llfn = decl_internal_fastcall_fn(cx.ccx.llmod, s, llfnty);
    cx.ccx.item_ids.insert(dtor.node.id, llfn);
    cx.ccx.item_symbols.insert(dtor.node.id, s);
    trans_fn(cx, dtor.span, dtor.node.meth, llfn,
             some(self_ty), ty_params,
             dtor.node.id);
    ret llfn;
}

// trans_obj: creates an LLVM function that is the object constructor for the
// object being translated.
fn trans_obj(@local_ctxt cx, &span sp, &ast::_obj ob, ast::node_id ctor_id,
             &ast::ty_param[] ty_params) {
    // To make a function, we have to create a function context and, inside
    // that, a number of block contexts for which code is generated.

    auto ccx = cx.ccx;
    auto llctor_decl;
    alt (ccx.item_ids.find(ctor_id)) {
        case (some(?x)) { llctor_decl = x; }
        case (_) { cx.ccx.sess.span_fatal(sp,
                     "unbound llctor_decl in trans_obj"); }
    }
    // Much like trans_fn, we must create an LLVM function, but since we're
    // starting with an ast::_obj rather than an ast::_fn, we have some setup
    // work to do.

    // The fields of our object will become the arguments to the function
    // we're creating.

    let ast::arg[] fn_args = ~[];
    for (ast::obj_field f in ob.fields) {
        fn_args +=
            ~[rec(mode=ast::alias(false), ty=f.ty, ident=f.ident, id=f.id)];
    }
    auto fcx = new_fn_ctxt(cx, sp, llctor_decl);

    // Both regular arguments and type parameters are handled here.
    create_llargs_for_fn_args(fcx, ast::proto_fn, none[ty::t],
                              ty::ret_ty_of_fn(ccx.tcx, ctor_id),
                              fn_args, ty_params);
    let ty::arg[] arg_tys = arg_tys_of_fn(ccx, ctor_id);
    copy_args_to_allocas(fcx, fn_args, arg_tys);

    //  Create the first block context in the function and keep a handle on it
    //  to pass to finish_fn later.
    auto bcx = new_top_block_ctxt(fcx);
    auto lltop = bcx.llbb;

    // Pick up the type of this object by looking at our own output type, that
    // is, the output type of the object constructor we're building.
    auto self_ty = ty::ret_ty_of_fn(ccx.tcx, ctor_id);

    // Set up the two-word pair that we're going to return from the object
    // constructor we're building.  The two elements of this pair will be a
    // vtable pointer and a body pointer.  (llretptr already points to the
    // place where this two-word pair should go; it was pre-allocated by the
    // caller of the function.)
    auto pair = bcx.fcx.llretptr;

    // Grab onto the first and second elements of the pair.
    // abi::obj_field_vtbl and abi::obj_field_box simply specify words 0 and 1
    // of 'pair'.
    auto pair_vtbl =
        bcx.build.GEP(pair, ~[C_int(0), C_int(abi::obj_field_vtbl)]);
    auto pair_box =
        bcx.build.GEP(pair, ~[C_int(0), C_int(abi::obj_field_box)]);

    // Make a vtable for this object: a static array of pointers to functions.
    // It will be located in the read-only memory of the executable we're
    // creating and will contain ValueRefs for all of this object's methods.
    // create_vtbl returns a pointer to the vtable, which we store.
    auto vtbl = create_vtbl(cx, sp, self_ty, ob, ty_params, none,
                            ~[]);
    vtbl = bcx.build.PointerCast(vtbl, T_ptr(T_empty_struct()));

    bcx.build.Store(vtbl, pair_vtbl);

    // Next we have to take care of the other half of the pair we're
    // returning: a boxed (reference-counted) tuple containing a tydesc,
    // typarams, and fields.

    // FIXME: What about with_obj?  Do we have to think about it here?
    // (Pertains to issues #538/#539/#540/#543.)

    let TypeRef llbox_ty = T_ptr(T_empty_struct());

    // FIXME: we should probably also allocate a box for empty objs that have
    // a dtor, since otherwise they are never dropped, and the dtor never
    // runs.
    if (std::ivec::len[ast::ty_param](ty_params) == 0u &&
            std::ivec::len[ty::arg](arg_tys) == 0u) {
        // If the object we're translating has no fields or type parameters,
        // there's not much to do.

        // Store null into pair, if no args or typarams.
        bcx.build.Store(C_null(llbox_ty), pair_box);
    } else {
        // Otherwise, we have to synthesize a big structural type for the
        // object body.
        let ty::t[] obj_fields = ~[];
        for (ty::arg a in arg_tys) { obj_fields += ~[a.ty]; }

        // Tuple type for fields: [field, ...]
        let ty::t fields_ty = ty::mk_imm_tup(ccx.tcx, obj_fields);

        auto tydesc_ty = ty::mk_type(ccx.tcx);
        let ty::t[] tps = ~[];
        for (ast::ty_param tp in ty_params) { tps += ~[tydesc_ty]; }

        // Tuple type for typarams: [typaram, ...]
        let ty::t typarams_ty = ty::mk_imm_tup(ccx.tcx, tps);

        // Tuple type for body: [tydesc_ty, [typaram, ...], [field, ...]]
        let ty::t body_ty =
            ty::mk_imm_tup(ccx.tcx, ~[tydesc_ty, typarams_ty, fields_ty]);

        // Hand this type we've synthesized off to trans_malloc_boxed, which
        // allocates a box, including space for a refcount.
        auto box = trans_malloc_boxed(bcx, body_ty);
        bcx = box.bcx;

        // mk_imm_box throws a refcount into the type we're synthesizing, so
        // that it looks like: [rc, [tydesc_ty, [typaram, ...], [field, ...]]]
        let ty::t boxed_body_ty = ty::mk_imm_box(ccx.tcx, body_ty);

        // Grab onto the refcount and body parts of the box we allocated.
        auto rc =
            GEP_tup_like(bcx, boxed_body_ty, box.val,
                         ~[0, abi::box_rc_field_refcnt]);
        bcx = rc.bcx;
        auto body =
            GEP_tup_like(bcx, boxed_body_ty, box.val,
                         ~[0, abi::box_rc_field_body]);
        bcx = body.bcx;
        bcx.build.Store(C_int(1), rc.val);

        // Put together a tydesc for the body, so that the object can later be
        // freed by calling through its tydesc.

        // Every object (not just those with type parameters) needs to have a
        // tydesc to describe its body, since all objects have unknown type to
        // the user of the object.  So the tydesc is needed to keep track of
        // the types of the object's fields, so that the fields can be freed
        // later.

        auto body_tydesc =
            GEP_tup_like(bcx, body_ty, body.val,
                         ~[0, abi::obj_body_elt_tydesc]);
        bcx = body_tydesc.bcx;
        auto ti = none[@tydesc_info];
        auto body_td = get_tydesc(bcx, body_ty, true, ti);
        lazily_emit_tydesc_glue(bcx, abi::tydesc_field_drop_glue, ti);
        lazily_emit_tydesc_glue(bcx, abi::tydesc_field_free_glue, ti);
        bcx = body_td.bcx;
        bcx.build.Store(body_td.val, body_tydesc.val);

        // Copy the object's type parameters and fields into the space we
        // allocated for the object body.  (This is something like saving the
        // lexical environment of a function in its closure: the "captured
        // typarams" are any type parameters that are passed to the object
        // constructor and are then available to the object's methods.
        // Likewise for the object's fields.)

        // Copy typarams into captured typarams.
        auto body_typarams =
            GEP_tup_like(bcx, body_ty, body.val,
                         ~[0, abi::obj_body_elt_typarams]);
        bcx = body_typarams.bcx;
        let int i = 0;
        for (ast::ty_param tp in ty_params) {
            auto typaram = bcx.fcx.lltydescs.(i);
            auto capture =
                GEP_tup_like(bcx, typarams_ty, body_typarams.val, ~[0, i]);
            bcx = capture.bcx;
            bcx = copy_val(bcx, INIT, capture.val, typaram, tydesc_ty).bcx;
            i += 1;
        }

        // Copy args into body fields.
        auto body_fields =
            GEP_tup_like(bcx, body_ty, body.val,
                         ~[0, abi::obj_body_elt_fields]);
        bcx = body_fields.bcx;
        i = 0;
        for (ast::obj_field f in ob.fields) {
            alt (bcx.fcx.llargs.find(f.id)) {
                case (some(?arg1)) {
                    auto arg = load_if_immediate(bcx, arg1, arg_tys.(i).ty);
                    auto field =
                        GEP_tup_like(bcx, fields_ty, body_fields.val,
                                     ~[0, i]);
                    bcx = field.bcx;
                    bcx = copy_val(bcx, INIT, field.val, arg,
                                   arg_tys.(i).ty).bcx;
                    i += 1;
                }
                case (none) {
                    bcx.fcx.lcx.ccx.sess.span_fatal(f.ty.span,
                                  "internal error in trans_obj");
                }
            }
        }

        // Store box ptr in outer pair.
        auto p = bcx.build.PointerCast(box.val, llbox_ty);
        bcx.build.Store(p, pair_box);
    }
    bcx.build.RetVoid();

    // Insert the mandatory first few basic blocks before lltop.
    finish_fn(fcx, lltop);
}

fn trans_res_ctor(@local_ctxt cx, &span sp, &ast::_fn dtor,
                  ast::node_id ctor_id, &ast::ty_param[] ty_params) {
    // Create a function for the constructor
    auto llctor_decl;
    alt (cx.ccx.item_ids.find(ctor_id)) {
        case (some(?x)) { llctor_decl = x; }
        case (_) {
            cx.ccx.sess.span_fatal(sp, "unbound ctor_id in trans_res_ctor");
        }
    }
    auto fcx = new_fn_ctxt(cx, sp, llctor_decl);
    auto ret_t = ty::ret_ty_of_fn(cx.ccx.tcx, ctor_id);
    create_llargs_for_fn_args(fcx, ast::proto_fn, none[ty::t],
                              ret_t, dtor.decl.inputs, ty_params);
    auto bcx = new_top_block_ctxt(fcx);
    auto lltop = bcx.llbb;
    auto arg_t = arg_tys_of_fn(cx.ccx, ctor_id).(0).ty;
    auto tup_t = ty::mk_imm_tup(cx.ccx.tcx, ~[ty::mk_int(cx.ccx.tcx), arg_t]);
    auto arg;
    alt (fcx.llargs.find(dtor.decl.inputs.(0).id)) {
        case (some(?x)) { arg = load_if_immediate(bcx, x, arg_t); }
        case (_) {
            cx.ccx.sess.span_fatal(sp, "unbound dtor decl in trans_res_ctor");
        }
    }

    auto llretptr = fcx.llretptr;
    if (ty::type_has_dynamic_size(cx.ccx.tcx, ret_t)) {
        auto llret_t = T_ptr(T_struct(~[T_i32(), llvm::LLVMTypeOf(arg)]));
        llretptr = bcx.build.BitCast(llretptr, llret_t);
    }

    auto dst = GEP_tup_like(bcx, tup_t, llretptr, ~[0, 1]);
    bcx = dst.bcx;
    bcx = copy_val(bcx, INIT, dst.val, arg, arg_t).bcx;
    auto flag = GEP_tup_like(bcx, tup_t, llretptr, ~[0, 0]);
    bcx = flag.bcx;
    bcx.build.Store(C_int(1), flag.val);
    bcx.build.RetVoid();
    finish_fn(fcx, lltop);
}


fn trans_tag_variant(@local_ctxt cx, ast::node_id tag_id,
                     &ast::variant variant, int index, bool is_degen,
                     &ast::ty_param[] ty_params) {
    if (std::ivec::len[ast::variant_arg](variant.node.args) == 0u) {
        ret; // nullary constructors are just constants

    }
    // Translate variant arguments to function arguments.

    let ast::arg[] fn_args = ~[];
    auto i = 0u;
    for (ast::variant_arg varg in variant.node.args) {
        fn_args +=
            ~[rec(mode=ast::alias(false),
                  ty=varg.ty,
                  ident="arg" + uint::to_str(i, 10u),
                  id=varg.id)];
    }
    assert (cx.ccx.item_ids.contains_key(variant.node.id));
    let ValueRef llfndecl;
    alt (cx.ccx.item_ids.find(variant.node.id)) {
        case (some(?x)) { llfndecl = x; }
        case (_) {
            cx.ccx.sess.span_fatal(variant.span,
                               "unbound variant id in trans_tag_variant");
        }
    }
    auto fcx = new_fn_ctxt(cx, variant.span, llfndecl);
    create_llargs_for_fn_args(fcx, ast::proto_fn, none[ty::t],
                              ty::ret_ty_of_fn(cx.ccx.tcx, variant.node.id),
                              fn_args, ty_params);
    let ty::t[] ty_param_substs = ~[];
    i = 0u;
    for (ast::ty_param tp in ty_params) {
        ty_param_substs += ~[ty::mk_param(cx.ccx.tcx, i)];
        i += 1u;
    }
    auto arg_tys = arg_tys_of_fn(cx.ccx, variant.node.id);
    copy_args_to_allocas(fcx, fn_args, arg_tys);
    auto bcx = new_top_block_ctxt(fcx);
    auto lltop = bcx.llbb;

    auto llblobptr = if (is_degen) {
        fcx.llretptr
    } else {
        // Cast the tag to a type we can GEP into.
        auto lltagptr = bcx.build.PointerCast
            (fcx.llretptr, T_opaque_tag_ptr(fcx.lcx.ccx.tn));
        auto lldiscrimptr = bcx.build.GEP(lltagptr, ~[C_int(0), C_int(0)]);
        bcx.build.Store(C_int(index), lldiscrimptr);
        bcx.build.GEP(lltagptr, ~[C_int(0), C_int(1)])
    };
    i = 0u;
    for (ast::variant_arg va in variant.node.args) {
        auto rslt =
            GEP_tag(bcx, llblobptr, ast::local_def(tag_id),
                    ast::local_def(variant.node.id), ty_param_substs,
                    i as int);
        bcx = rslt.bcx;
        auto lldestptr = rslt.val;
        // If this argument to this function is a tag, it'll have come in to
        // this function as an opaque blob due to the way that type_of()
        // works. So we have to cast to the destination's view of the type.

        auto llargptr;
        alt (fcx.llargs.find(va.id)) {
            case (some(?x)) {
                llargptr = bcx.build.PointerCast(x, val_ty(lldestptr));
            }
            case (none) {
                bcx.fcx.lcx.ccx.sess.bug("unbound argptr in \
                   trans_tag_variant");
            }
        }
        auto arg_ty = arg_tys.(i).ty;
        auto llargval;
        if (ty::type_is_structural(cx.ccx.tcx, arg_ty) ||
                ty::type_has_dynamic_size(cx.ccx.tcx, arg_ty)) {
            llargval = llargptr;
        } else { llargval = bcx.build.Load(llargptr); }
        rslt = copy_val(bcx, INIT, lldestptr, llargval, arg_ty);
        bcx = rslt.bcx;
        i += 1u;
    }
    bcx = trans_block_cleanups(bcx, find_scope_cx(bcx));
    bcx.build.RetVoid();
    finish_fn(fcx, lltop);
}


// FIXME: this should do some structural hash-consing to avoid
// duplicate constants. I think. Maybe LLVM has a magical mode
// that does so later on?
fn trans_const_expr(&@crate_ctxt cx, @ast::expr e) -> ValueRef {
    alt (e.node) {
        case (ast::expr_lit(?lit)) { ret trans_crate_lit(cx, *lit); }
        case (_) {
            cx.sess.span_unimpl(e.span, "consts that's not a plain literal");
        }
    }
}

fn trans_const(&@crate_ctxt cx, @ast::expr e, ast::node_id id) {
    auto v = trans_const_expr(cx, e);
    // The scalars come back as 1st class LLVM vals
    // which we have to stick into global constants.

    alt (cx.consts.find(id)) {
        case (some(?g)) {
            llvm::LLVMSetInitializer(g, v);
            llvm::LLVMSetGlobalConstant(g, True);
        }
        case (_) {
            cx.sess.span_fatal(e.span, "Unbound const in trans_const");
        }
    }
}

fn trans_item(@local_ctxt cx, &ast::item item) {
    alt (item.node) {
        case (ast::item_fn(?f, ?tps)) {
            auto sub_cx = extend_path(cx, item.ident);
            alt (cx.ccx.item_ids.find(item.id)) {
                case (some(?llfndecl)) {
                    trans_fn(sub_cx, item.span, f, llfndecl,
                             none, tps, item.id);
                }
                case (_) {
                    cx.ccx.sess.span_fatal(item.span,
                           "unbound function item in trans_item");
                }
            }
        }
        case (ast::item_obj(?ob, ?tps, ?ctor_id)) {
            auto sub_cx =
                @rec(obj_typarams=tps, obj_fields=ob.fields
                     with *extend_path(cx, item.ident));
            trans_obj(sub_cx, item.span, ob, ctor_id, tps);
        }
        case (ast::item_res(?dtor, ?dtor_id, ?tps, ?ctor_id)) {
            trans_res_ctor(cx, item.span, dtor, ctor_id, tps);
            // Create a function for the destructor
            alt (cx.ccx.item_ids.find(item.id)) {
                case (some(?lldtor_decl)) {
                    trans_fn(cx, item.span, dtor, lldtor_decl, none, tps,
                             dtor_id);
                }
                case (_) { cx.ccx.sess.span_fatal(item.span,
                                          "unbound dtor in trans_item"); }
            }
        }
        case (ast::item_mod(?m)) {
            auto sub_cx =
                @rec(path=cx.path + ~[item.ident],
                     module_path=cx.module_path + ~[item.ident] with *cx);
            trans_mod(sub_cx, m);
        }
        case (ast::item_tag(?variants, ?tps)) {
            auto sub_cx = extend_path(cx, item.ident);
            auto degen = std::ivec::len(variants) == 1u;
            auto i = 0;
            for (ast::variant variant in variants) {
                trans_tag_variant(sub_cx, item.id, variant, i, degen, tps);
                i += 1;
            }
        }
        case (ast::item_const(_, ?expr)) {
            trans_const(cx.ccx, expr, item.id);
        }
        case (_) {/* fall through */ }
    }
}


// Translate a module.  Doing this amounts to translating the items in the
// module; there ends up being no artifact (aside from linkage names) of
// separate modules in the compiled program.  That's because modules exist
// only as a convenience for humans working with the code, to organize names
// and control visibility.
fn trans_mod(@local_ctxt cx, &ast::_mod m) {
    for (@ast::item item in m.items) { trans_item(cx, *item); }
}

fn get_pair_fn_ty(TypeRef llpairty) -> TypeRef {
    // Bit of a kludge: pick the fn typeref out of the pair.

    ret struct_elt(llpairty, 0u);
}

fn decl_fn_and_pair(&@crate_ctxt ccx, &span sp, &str[] path, str flav,
                    &ast::ty_param[] ty_params, ast::node_id node_id) {
    decl_fn_and_pair_full(ccx, sp, path, flav, ty_params, node_id,
                          node_id_type(ccx, node_id));
}

fn decl_fn_and_pair_full(&@crate_ctxt ccx, &span sp, &str[] path, str flav,
                         &ast::ty_param[] ty_params, ast::node_id node_id,
                         ty::t node_type) {
    auto llfty;
    alt (ty::struct(ccx.tcx, node_type)) {
        case (ty::ty_fn(?proto, ?inputs, ?output, _, _)) {
            llfty =
                type_of_fn(ccx, sp, proto, inputs, output,
                           std::ivec::len[ast::ty_param](ty_params));
        }
        case (_) {
            ccx.sess.bug("decl_fn_and_pair(): fn item doesn't have fn type!");
        }
    }
    let bool is_main = is_main_name(path) && !ccx.sess.get_opts().library;
    // Declare the function itself.

    let str s =
        if (is_main) {
            "_rust_main"
        } else { mangle_internal_name_by_path(ccx, path) };
    let ValueRef llfn = decl_internal_fastcall_fn(ccx.llmod, s, llfty);
    // Declare the global constant pair that points to it.

    let str ps = mangle_exported_name(ccx, path, node_type);
    register_fn_pair(ccx, ps, llfty, llfn, node_id);
    if (is_main) {
        if (ccx.main_fn != none[ValueRef]) {
            ccx.sess.span_fatal(sp, "multiple 'main' functions");
        }
        llvm::LLVMSetLinkage(llfn,
                             lib::llvm::LLVMExternalLinkage as llvm::Linkage);
        ccx.main_fn = some(llfn);
    }
}

// Create a closure: a pair containing (1) a ValueRef, pointing to where the
// fn's definition is in the executable we're creating, and (2) a pointer to
// space for the function's environment.
fn create_fn_pair(&@crate_ctxt cx, str ps, TypeRef llfnty, ValueRef llfn,
                  bool external) -> ValueRef {
    auto gvar =
        llvm::LLVMAddGlobal(cx.llmod, T_fn_pair(*cx, llfnty), str::buf(ps));
    auto pair = C_struct(~[llfn, C_null(T_opaque_closure_ptr(*cx))]);
    llvm::LLVMSetInitializer(gvar, pair);
    llvm::LLVMSetGlobalConstant(gvar, True);
    if (!external) {
        llvm::LLVMSetLinkage(gvar,
                             lib::llvm::LLVMInternalLinkage as llvm::Linkage);
    }
    ret gvar;
}

// Create a /real/ closure: this is like create_fn_pair, but creates a
// a fn value on the stack with a specified environment (which need not be
// on the stack).
fn create_real_fn_pair(&@block_ctxt cx, TypeRef llfnty,
                       ValueRef llfn, ValueRef llenvptr) -> ValueRef {
    auto lcx = cx.fcx.lcx;

    auto pair = alloca(cx, T_fn_pair(*lcx.ccx, llfnty));
    auto code_cell =
        cx.build.GEP(pair, ~[C_int(0), C_int(abi::fn_field_code)]);
    cx.build.Store(llfn, code_cell);
    auto env_cell =
        cx.build.GEP(pair, ~[C_int(0), C_int(abi::fn_field_box)]);
    auto llenvblobptr =
        cx.build.PointerCast(llenvptr,
                             T_opaque_closure_ptr(*lcx.ccx));
    cx.build.Store(llenvblobptr, env_cell);
    ret pair;
}

fn register_fn_pair(&@crate_ctxt cx, str ps, TypeRef llfnty, ValueRef llfn,
                    ast::node_id id) {
    // FIXME: We should also hide the unexported pairs in crates.

    auto gvar =
        create_fn_pair(cx, ps, llfnty, llfn, cx.sess.get_opts().library);
    cx.item_ids.insert(id, llfn);
    cx.item_symbols.insert(id, ps);
    cx.fn_pairs.insert(id, gvar);
}


// Returns the number of type parameters that the given native function has.
fn native_fn_ty_param_count(&@crate_ctxt cx, ast::node_id id) -> uint {
    auto count;
    auto native_item = alt (cx.ast_map.find(id)) {
        case (some(ast_map::node_native_item(?i))) { i }
    };
    alt (native_item.node) {
        case (ast::native_item_ty) {
            cx.sess.bug("decl_native_fn_and_pair(): native fn isn't " +
                            "actually a fn");
        }
        case (ast::native_item_fn(_, _, ?tps)) {
            count = std::ivec::len[ast::ty_param](tps);
        }
    }
    ret count;
}

fn native_fn_wrapper_type(&@crate_ctxt cx, &span sp, uint ty_param_count,
                          ty::t x) -> TypeRef {
    alt (ty::struct(cx.tcx, x)) {
        case (ty::ty_native_fn(?abi, ?args, ?out)) {
            ret type_of_fn(cx, sp, ast::proto_fn, args, out, ty_param_count);
        }
    }
}

fn decl_native_fn_and_pair(&@crate_ctxt ccx, &span sp, &str[] path, str name,
                           ast::node_id id) {
    auto num_ty_param = native_fn_ty_param_count(ccx, id);
    // Declare the wrapper.

    auto t = node_id_type(ccx, id);
    auto wrapper_type = native_fn_wrapper_type(ccx, sp, num_ty_param, t);
    let str s = mangle_internal_name_by_path(ccx, path);
    let ValueRef wrapper_fn =
        decl_internal_fastcall_fn(ccx.llmod, s, wrapper_type);
    // Declare the global constant pair that points to it.

    let str ps = mangle_exported_name(ccx, path, node_id_type(ccx, id));
    register_fn_pair(ccx, ps, wrapper_type, wrapper_fn, id);
    // Build the wrapper.

    auto fcx = new_fn_ctxt(new_local_ctxt(ccx), sp, wrapper_fn);
    auto bcx = new_top_block_ctxt(fcx);
    auto lltop = bcx.llbb;
    // Declare the function itself.

    auto fn_type = node_id_type(ccx, id); // NB: has no type params

    auto abi = ty::ty_fn_abi(ccx.tcx, fn_type);
    // FIXME: If the returned type is not nil, then we assume it's 32 bits
    // wide. This is obviously wildly unsafe. We should have a better FFI
    // that allows types of different sizes to be returned.

    auto rty = ty::ty_fn_ret(ccx.tcx, fn_type);
    auto rty_is_nil = ty::type_is_nil(ccx.tcx, rty);

    auto pass_task;
    auto uses_retptr;
    auto cast_to_i32;
    alt (abi) {
      case (ast::native_abi_rust) {
        pass_task = true;
        uses_retptr = false;
        cast_to_i32 = true;
      }
      case (ast::native_abi_rust_intrinsic) {
        pass_task = true;
        uses_retptr = true;
        cast_to_i32 = false;
      }
      case (ast::native_abi_cdecl) {
        pass_task = false;
        uses_retptr = false;
        cast_to_i32 = true;
      }
      case (ast::native_abi_llvm) {
        pass_task = false;
        uses_retptr = false;
        cast_to_i32 = false;
      }
      case (ast::native_abi_x86stdcall) {
        pass_task = false;
        uses_retptr = false;
        cast_to_i32 = true;
      }
    }

    auto lltaskptr;
    if (cast_to_i32) {
        lltaskptr = vp2i(bcx, fcx.lltaskptr);
    } else { lltaskptr = fcx.lltaskptr; }

    let ValueRef[] call_args = ~[];
    if (pass_task) { call_args += ~[lltaskptr]; }
    if (uses_retptr) { call_args += ~[bcx.fcx.llretptr]; }

    auto arg_n = 3u;
    for each (uint i in uint::range(0u, num_ty_param)) {
        auto llarg = llvm::LLVMGetParam(fcx.llfn, arg_n);
        fcx.lltydescs += ~[llarg];
        assert (llarg as int != 0);
        if (cast_to_i32) {
            call_args += ~[vp2i(bcx, llarg)];
        } else { call_args += ~[llarg]; }
        arg_n += 1u;
    }
    fn convert_arg_to_i32(&@block_ctxt cx, ValueRef v, ty::t t, ty::mode mode)
       -> ValueRef {
        if (mode == ty::mo_val) {
            if (ty::type_is_integral(cx.fcx.lcx.ccx.tcx, t)) {
                auto lldsttype = T_int();
                auto llsrctype = type_of(cx.fcx.lcx.ccx, cx.sp, t);
                if (llvm::LLVMGetIntTypeWidth(lldsttype) >
                        llvm::LLVMGetIntTypeWidth(llsrctype)) {
                    ret cx.build.ZExtOrBitCast(v, T_int());
                }
                ret cx.build.TruncOrBitCast(v, T_int());
            }
            if (ty::type_is_fp(cx.fcx.lcx.ccx.tcx, t)) {
                ret cx.build.FPToSI(v, T_int());
            }
        }
        ret vp2i(cx, v);
    }

    fn trans_simple_native_abi(&@block_ctxt bcx, str name,
                               &mutable ValueRef[] call_args,
                               ty::t fn_type, uint first_arg_n,
                               bool uses_retptr, uint cc) ->
       tup(ValueRef, ValueRef) {
        let TypeRef[] call_arg_tys = ~[];
        for (ValueRef arg in call_args) { call_arg_tys += ~[val_ty(arg)]; }

        auto llnativefnty;
        if (uses_retptr) {
            llnativefnty = T_fn(call_arg_tys, T_void());
        } else {
            llnativefnty =
                T_fn(call_arg_tys,
                     type_of(bcx.fcx.lcx.ccx, bcx.sp,
                             ty::ty_fn_ret(bcx.fcx.lcx.ccx.tcx, fn_type)));
        }

        auto llnativefn =
            get_extern_fn(bcx.fcx.lcx.ccx.externs, bcx.fcx.lcx.ccx.llmod,
                          name, cc, llnativefnty);
        auto r = if (cc == lib::llvm::LLVMCCallConv) {
            bcx.build.Call(llnativefn, call_args)
        } else {
            bcx.build.CallWithConv(llnativefn, call_args, cc)
        };
        auto rptr = bcx.fcx.llretptr;
        ret tup(r, rptr);
    }

    auto args = ty::ty_fn_args(ccx.tcx, fn_type);
    // Build up the list of arguments.

    let (tup(ValueRef, ty::t))[] drop_args = ~[];
    auto i = arg_n;
    for (ty::arg arg in args) {
        auto llarg = llvm::LLVMGetParam(fcx.llfn, i);
        assert (llarg as int != 0);
        if (cast_to_i32) {
            auto llarg_i32 = convert_arg_to_i32(bcx, llarg, arg.ty, arg.mode);
            call_args += ~[llarg_i32];
        } else {
            call_args += ~[llarg];
        }
        if (arg.mode == ty::mo_val) { drop_args += ~[tup(llarg, arg.ty)]; }
        i += 1u;
    }
    auto r;
    auto rptr;
    alt (abi) {
        case (ast::native_abi_llvm) {
            auto result =
                trans_simple_native_abi(bcx, name, call_args, fn_type, arg_n,
                                        uses_retptr,
                                        lib::llvm::LLVMCCallConv);
            r = result._0;
            rptr = result._1;
        }
        case (ast::native_abi_rust_intrinsic) {
            auto external_name = "rust_intrinsic_" + name;
            auto result =
                trans_simple_native_abi(bcx, external_name, call_args,
                                        fn_type, arg_n, uses_retptr,
                                        lib::llvm::LLVMCCallConv);
            r = result._0;
            rptr = result._1;
        }
        case (ast::native_abi_x86stdcall) {
            auto result =
                trans_simple_native_abi(bcx, name, call_args, fn_type, arg_n,
                                        uses_retptr,
                                        lib::llvm::LLVMX86StdcallCallConv);
            r = result._0;
            rptr = result._1;
        }
        case (_) {
            r =
                trans_native_call(bcx.build, ccx.glues, lltaskptr,
                                  ccx.externs, ccx.tn, ccx.llmod, name,
                                  pass_task, call_args);
            rptr = bcx.build.BitCast(fcx.llretptr, T_ptr(T_i32()));
        }
    }
    // We don't store the return value if it's nil, to avoid stomping on a nil
    // pointer. This is the only concession made to non-i32 return values. See
    // the FIXME above.

    if (!rty_is_nil && !uses_retptr) { bcx.build.Store(r, rptr); }

    for (tup(ValueRef, ty::t) d in drop_args) {
        bcx = drop_ty(bcx, d._0, d._1).bcx;
    }
    bcx.build.RetVoid();
    finish_fn(fcx, lltop);
}

fn item_path(&@ast::item item) -> str[] { ret ~[item.ident]; }

fn collect_native_item(@crate_ctxt ccx, &@ast::native_item i, &str[] pt,
                       &vt[str[]] v) {
    alt (i.node) {
        case (ast::native_item_fn(_, _, _)) {
            if (!ccx.obj_methods.contains_key(i.id)) {
                decl_native_fn_and_pair(ccx, i.span, pt, i.ident, i.id);
            }
        }
        case (_) {}
    }
}

fn collect_item_1(@crate_ctxt ccx, &@ast::item i, &str[] pt, &vt[str[]] v) {
    visit::visit_item(i, pt + item_path(i), v);
    alt (i.node) {
        case (ast::item_const(_, _)) {
            auto typ = node_id_type(ccx, i.id);
            auto s = mangle_exported_name(ccx, pt + ~[i.ident],
                                          node_id_type(ccx, i.id));
            auto g = llvm::LLVMAddGlobal(ccx.llmod, type_of(ccx, i.span, typ),
                                         str::buf(s));
            ccx.item_symbols.insert(i.id, s);
            ccx.consts.insert(i.id, g);
        }
        case (_) { }
    }
}

fn collect_item_2(&@crate_ctxt ccx, &@ast::item i, &str[] pt, &vt[str[]] v) {
    auto new_pt = pt + item_path(i);
    visit::visit_item(i, new_pt, v);
    alt (i.node) {
        case (ast::item_fn(?f, ?tps)) {
            if (!ccx.obj_methods.contains_key(i.id)) {
                decl_fn_and_pair(ccx, i.span, new_pt, "fn", tps, i.id);
            }
        }
        case (ast::item_obj(?ob, ?tps, ?ctor_id)) {
            decl_fn_and_pair(ccx, i.span, new_pt, "obj_ctor", tps, ctor_id);
            for (@ast::method m in ob.methods) {
                ccx.obj_methods.insert(m.node.id, ());
            }
        }
        case (ast::item_res(_, ?dtor_id, ?tps, ?ctor_id)) {
            decl_fn_and_pair(ccx, i.span, new_pt, "res_ctor", tps, ctor_id);
            // Note that the destructor is associated with the item's id, not
            // the dtor_id. This is a bit counter-intuitive, but simplifies
            // ty_res, which would have to carry around two def_ids otherwise
            // -- one to identify the type, and one to find the dtor symbol.
            decl_fn_and_pair_full(ccx, i.span, new_pt, "res_dtor", tps, i.id,
                                  node_id_type(ccx, dtor_id));
        }
        case (_) { }
    }
}

fn collect_items(&@crate_ctxt ccx, @ast::crate crate) {
    auto visitor0 = visit::default_visitor();
    auto visitor1 =
        @rec(visit_native_item=bind collect_native_item(ccx, _, _, _),
             visit_item=bind collect_item_1(ccx, _, _, _) with *visitor0);
    auto visitor2 =
        @rec(visit_item=bind collect_item_2(ccx, _, _, _) with *visitor0);
    visit::visit_crate(*crate, ~[], visit::mk_vt(visitor1));
    visit::visit_crate(*crate, ~[], visit::mk_vt(visitor2));
}

fn collect_tag_ctor(@crate_ctxt ccx, &@ast::item i, &str[] pt, &vt[str[]] v) {
    auto new_pt = pt + item_path(i);
    visit::visit_item(i, new_pt, v);
    alt (i.node) {
        case (ast::item_tag(?variants, ?tps)) {
            for (ast::variant variant in variants) {
                if (std::ivec::len(variant.node.args) != 0u) {
                    decl_fn_and_pair(ccx, i.span,
                                     new_pt + ~[variant.node.name], "tag",
                                     tps, variant.node.id);
                }
            }
        }
        case (_) {/* fall through */ }
    }
}

fn collect_tag_ctors(&@crate_ctxt ccx, @ast::crate crate) {
    auto visitor =
        @rec(visit_item=bind collect_tag_ctor(ccx, _, _, _)
             with *visit::default_visitor());
    visit::visit_crate(*crate, ~[], visit::mk_vt(visitor));
}


// The constant translation pass.
fn trans_constant(@crate_ctxt ccx, &@ast::item it, &str[] pt, &vt[str[]] v) {
    auto new_pt = pt + item_path(it);
    visit::visit_item(it, new_pt, v);
    alt (it.node) {
        case (ast::item_tag(?variants, _)) {
            auto i = 0u;
            auto n_variants = std::ivec::len[ast::variant](variants);
            while (i < n_variants) {
                auto variant = variants.(i);
                auto p = new_pt + ~[it.ident, variant.node.name, "discrim"];
                auto s = mangle_exported_name(ccx, p, ty::mk_int(ccx.tcx));
                auto discrim_gvar =
                    llvm::LLVMAddGlobal(ccx.llmod, T_int(), str::buf(s));
                if (n_variants != 1u) {
                    llvm::LLVMSetInitializer(discrim_gvar, C_int(i as int));
                    llvm::LLVMSetGlobalConstant(discrim_gvar, True);
                }
                ccx.discrims.insert(variant.node.id, discrim_gvar);
                ccx.discrim_symbols.insert(variant.node.id, s);
                i += 1u;
            }
        }
        case (_) { }
    }
}

fn trans_constants(&@crate_ctxt ccx, @ast::crate crate) {
    auto visitor =
        @rec(visit_item=bind trans_constant(ccx, _, _, _)
             with *visit::default_visitor());
    visit::visit_crate(*crate, ~[], visit::mk_vt(visitor));
}

fn vp2i(&@block_ctxt cx, ValueRef v) -> ValueRef {
    ret cx.build.PtrToInt(v, T_int());
}

fn vi2p(&@block_ctxt cx, ValueRef v, TypeRef t) -> ValueRef {
    ret cx.build.IntToPtr(v, t);
}

fn p2i(ValueRef v) -> ValueRef { ret llvm::LLVMConstPtrToInt(v, T_int()); }

fn i2p(ValueRef v, TypeRef t) -> ValueRef {
    ret llvm::LLVMConstIntToPtr(v, t);
}

fn declare_intrinsics(ModuleRef llmod) -> hashmap[str, ValueRef] {
    let TypeRef[] T_memmove32_args =
        ~[T_ptr(T_i8()), T_ptr(T_i8()), T_i32(), T_i32(), T_i1()];
    let TypeRef[] T_memmove64_args =
        ~[T_ptr(T_i8()), T_ptr(T_i8()), T_i64(), T_i32(), T_i1()];
    let TypeRef[] T_memset32_args =
        ~[T_ptr(T_i8()), T_i8(), T_i32(), T_i32(), T_i1()];
    let TypeRef[] T_memset64_args =
        ~[T_ptr(T_i8()), T_i8(), T_i64(), T_i32(), T_i1()];
    let TypeRef[] T_trap_args = ~[];
    auto memmove32 =
        decl_cdecl_fn(llmod, "llvm.memmove.p0i8.p0i8.i32",
                      T_fn(T_memmove32_args, T_void()));
    auto memmove64 =
        decl_cdecl_fn(llmod, "llvm.memmove.p0i8.p0i8.i64",
                      T_fn(T_memmove64_args, T_void()));
    auto memset32 =
        decl_cdecl_fn(llmod, "llvm.memset.p0i8.i32",
                      T_fn(T_memset32_args, T_void()));
    auto memset64 =
        decl_cdecl_fn(llmod, "llvm.memset.p0i8.i64",
                      T_fn(T_memset64_args, T_void()));
    auto trap =
        decl_cdecl_fn(llmod, "llvm.trap", T_fn(T_trap_args, T_void()));
    auto intrinsics = new_str_hash[ValueRef]();
    intrinsics.insert("llvm.memmove.p0i8.p0i8.i32", memmove32);
    intrinsics.insert("llvm.memmove.p0i8.p0i8.i64", memmove64);
    intrinsics.insert("llvm.memset.p0i8.i32", memset32);
    intrinsics.insert("llvm.memset.p0i8.i64", memset64);
    intrinsics.insert("llvm.trap", trap);
    ret intrinsics;
}

fn trace_str(&@block_ctxt cx, str s) {
    cx.build.Call(cx.fcx.lcx.ccx.upcalls.trace_str,
                  ~[cx.fcx.lltaskptr, C_cstr(cx.fcx.lcx.ccx, s)]);
}

fn trace_word(&@block_ctxt cx, ValueRef v) {
    cx.build.Call(cx.fcx.lcx.ccx.upcalls.trace_word, ~[cx.fcx.lltaskptr, v]);
}

fn trace_ptr(&@block_ctxt cx, ValueRef v) {
    trace_word(cx, cx.build.PtrToInt(v, T_int()));
}

fn trap(&@block_ctxt bcx) {
    let ValueRef[] v = ~[];
    alt (bcx.fcx.lcx.ccx.intrinsics.find("llvm.trap")) {
        case (some(?x)) { bcx.build.Call(x, v); }
        case (_) { bcx.fcx.lcx.ccx.sess.bug("unbound llvm.trap in trap"); }
    }
}

fn decl_no_op_type_glue(ModuleRef llmod, TypeRef taskptr_type) -> ValueRef {
    auto ty = T_fn(~[taskptr_type, T_ptr(T_i8())], T_void());
    ret decl_fastcall_fn(llmod, abi::no_op_type_glue_name(), ty);
}

fn make_no_op_type_glue(ValueRef fun) {
    auto bb_name = str::buf("_rust_no_op_type_glue_bb");
    auto llbb = llvm::LLVMAppendBasicBlock(fun, bb_name);
    new_builder(llbb).RetVoid();
}

fn vec_fill(&@block_ctxt bcx, ValueRef v) -> ValueRef {
    ret bcx.build.Load(bcx.build.GEP(v,
                                     ~[C_int(0), C_int(abi::vec_elt_fill)]));
}

fn vec_p0(&@block_ctxt bcx, ValueRef v) -> ValueRef {
    auto p = bcx.build.GEP(v, ~[C_int(0), C_int(abi::vec_elt_data)]);
    ret bcx.build.PointerCast(p, T_ptr(T_i8()));
}

fn make_glues(ModuleRef llmod, TypeRef taskptr_type) -> @glue_fns {
    ret @rec(no_op_type_glue=decl_no_op_type_glue(llmod, taskptr_type));
}

fn make_common_glue(&session::session sess, &str output) {
    // FIXME: part of this is repetitive and is probably a good idea
    // to autogen it.

    auto task_type = T_task();
    auto taskptr_type = T_ptr(task_type);

    auto llmod =
        llvm::LLVMModuleCreateWithNameInContext(str::buf("rust_out"),
                                                llvm::LLVMGetGlobalContext());
    llvm::LLVMSetDataLayout(llmod, str::buf(x86::get_data_layout()));
    llvm::LLVMSetTarget(llmod, str::buf(x86::get_target_triple()));
    mk_target_data(x86::get_data_layout());
    declare_intrinsics(llmod);
    llvm::LLVMSetModuleInlineAsm(llmod, str::buf(x86::get_module_asm()));
    make_glues(llmod, taskptr_type);
    link::write::run_passes(sess, llmod, output);
}

fn create_module_map(&@crate_ctxt ccx) -> ValueRef {
    auto elttype = T_struct(~[T_int(), T_int()]);
    auto maptype = T_array(elttype, ccx.module_data.size() + 1u);
    auto map =
        llvm::LLVMAddGlobal(ccx.llmod, maptype, str::buf("_rust_mod_map"));
    llvm::LLVMSetLinkage(map,
                         lib::llvm::LLVMInternalLinkage as
                         llvm::Linkage);
    let ValueRef[] elts = ~[];
    for each (@tup(str, ValueRef) item in ccx.module_data.items()) {
        auto elt = C_struct(~[p2i(C_cstr(ccx, item._0)), p2i(item._1)]);
        elts += ~[elt];
    }
    auto term = C_struct(~[C_int(0), C_int(0)]);
    elts += ~[term];
    llvm::LLVMSetInitializer(map, C_array(elttype, elts));
    ret map;
}


// FIXME use hashed metadata instead of crate names once we have that
fn create_crate_map(&@crate_ctxt ccx) -> ValueRef {
    let ValueRef[] subcrates = ~[];
    auto i = 1;
    auto cstore = ccx.sess.get_cstore();
    while (cstore::have_crate_data(cstore, i)) {
        auto name = cstore::get_crate_data(cstore, i).name;
        auto cr =
            llvm::LLVMAddGlobal(ccx.llmod, T_int(),
                                str::buf("_rust_crate_map_" + name));
        subcrates += ~[p2i(cr)];
        i += 1;
    }
    subcrates += ~[C_int(0)];
    auto mapname;
    if (ccx.sess.get_opts().library) {
        mapname = ccx.link_meta.name;
    } else { mapname = "toplevel"; }
    auto sym_name = "_rust_crate_map_" + mapname;
    auto arrtype = T_array(T_int(), std::ivec::len[ValueRef](subcrates));
    auto maptype = T_struct(~[T_int(), arrtype]);
    auto map = llvm::LLVMAddGlobal(ccx.llmod, maptype, str::buf(sym_name));
    llvm::LLVMSetLinkage(map,
                         lib::llvm::LLVMExternalLinkage as llvm::Linkage);
    llvm::LLVMSetInitializer(map,
                             C_struct(~[p2i(create_module_map(ccx)),
                                        C_array(T_int(), subcrates)]));
    ret map;
}

fn write_metadata(&@crate_ctxt cx, &@ast::crate crate) {
    if (!cx.sess.get_opts().library) { ret; }
    auto llmeta = C_postr(metadata::encoder::encode_metadata(cx, crate));
    auto llconst = trans_common::C_struct(~[llmeta]);
    auto llglobal =
        llvm::LLVMAddGlobal(cx.llmod, val_ty(llconst),
                            str::buf("rust_metadata"));
    llvm::LLVMSetInitializer(llglobal, llconst);
    llvm::LLVMSetSection(llglobal, str::buf(x86::get_meta_sect_name()));
    llvm::LLVMSetLinkage(llglobal,
                         lib::llvm::LLVMInternalLinkage as llvm::Linkage);

    auto t_ptr_i8 = T_ptr(T_i8());
    llglobal = llvm::LLVMConstBitCast(llglobal, t_ptr_i8);
    auto llvm_used =
        llvm::LLVMAddGlobal(cx.llmod, T_array(t_ptr_i8, 1u),
                            str::buf("llvm.used"));
    llvm::LLVMSetLinkage(llvm_used,
                         lib::llvm::LLVMAppendingLinkage as llvm::Linkage);
    llvm::LLVMSetInitializer(llvm_used, C_array(t_ptr_i8, ~[llglobal]));
}

fn trans_crate(&session::session sess, &@ast::crate crate, &ty::ctxt tcx,
               &str output, &ast_map::map amap) -> ModuleRef {
    auto llmod =
        llvm::LLVMModuleCreateWithNameInContext(str::buf("rust_out"),
                                                llvm::LLVMGetGlobalContext());
    llvm::LLVMSetDataLayout(llmod, str::buf(x86::get_data_layout()));
    llvm::LLVMSetTarget(llmod, str::buf(x86::get_target_triple()));
    auto td = mk_target_data(x86::get_data_layout());
    auto tn = mk_type_names();
    auto intrinsics = declare_intrinsics(llmod);
    auto task_type = T_task();
    auto taskptr_type = T_ptr(task_type);
    auto tydesc_type = T_tydesc(taskptr_type);
    auto glues = make_glues(llmod, taskptr_type);
    auto hasher = ty::hash_ty;
    auto eqer = ty::eq_ty;
    auto tag_sizes = map::mk_hashmap[ty::t, uint](hasher, eqer);
    auto tydescs = map::mk_hashmap[ty::t, @tydesc_info](hasher, eqer);
    auto lltypes = map::mk_hashmap[ty::t, TypeRef](hasher, eqer);
    auto sha1s = map::mk_hashmap[ty::t, str](hasher, eqer);
    auto short_names = map::mk_hashmap[ty::t, str](hasher, eqer);
    auto sha = std::sha1::mk_sha1();
    auto ccx =
        @rec(sess=sess,
             llmod=llmod,
             td=td,
             tn=tn,
             externs=new_str_hash[ValueRef](),
             intrinsics=intrinsics,
             item_ids=new_int_hash[ValueRef](),
             ast_map=amap,
             item_symbols=new_int_hash[str](),
             mutable main_fn=none[ValueRef],
             link_meta=link::build_link_meta(sess, *crate, output, sha),
             tag_sizes=tag_sizes,
             discrims=new_int_hash[ValueRef](),
             discrim_symbols=new_int_hash[str](),
             fn_pairs=new_int_hash[ValueRef](),
             consts=new_int_hash[ValueRef](),
             obj_methods=new_int_hash[()](),
             tydescs=tydescs,
             module_data=new_str_hash[ValueRef](),
             lltypes=lltypes,
             glues=glues,
             names=namegen(0),
             sha=sha,
             type_sha1s=sha1s,
             type_short_names=short_names,
             tcx=tcx,
             stats=rec(mutable n_static_tydescs=0u,
                       mutable n_derived_tydescs=0u,
                       mutable n_glues_created=0u,
                       mutable n_null_glues=0u,
                       mutable n_real_glues=0u,
                       fn_times=@mutable ~[]),
             upcalls=upcall::declare_upcalls(tn, tydesc_type, taskptr_type,
                                             llmod),
             rust_object_type=T_rust_object(),
             tydesc_type=tydesc_type,
             task_type=task_type);
    auto cx = new_local_ctxt(ccx);
    collect_items(ccx, crate);
    collect_tag_ctors(ccx, crate);
    trans_constants(ccx, crate);
    trans_mod(cx, crate.node.module);
    create_crate_map(ccx);
    emit_tydescs(ccx);
    // Translate the metadata:

    write_metadata(cx.ccx, crate);
    if (ccx.sess.get_opts().stats) {
        log_err "--- trans stats ---";
        log_err #fmt("n_static_tydescs: %u", ccx.stats.n_static_tydescs);
        log_err #fmt("n_derived_tydescs: %u", ccx.stats.n_derived_tydescs);
        log_err #fmt("n_glues_created: %u", ccx.stats.n_glues_created);
        log_err #fmt("n_null_glues: %u", ccx.stats.n_null_glues);
        log_err #fmt("n_real_glues: %u", ccx.stats.n_real_glues);

        for (tup(str,int) timing in *ccx.stats.fn_times) {
            log_err #fmt("time: %s took %d ms", timing._0, timing._1);
        }
    }
    ret llmod;
}
//
// Local Variables:
// mode: rust
// fill-column: 78;
// indent-tabs-mode: nil
// c-basic-offset: 4
// buffer-file-coding-system: utf-8-unix
// compile-command: "make -k -C $RBUILD 2>&1 | sed -e 's/\\/x\\//x:\\//g'";
// End:
//
