import driver::session::{session, arch_x86_64};
import syntax::codemap::span;
import ctypes::{ c_uint, unsigned };
import front::attr;
import lib::llvm::{ llvm, TypeRef, ValueRef,
                    ModuleRef, CallConv, Attribute,
                    StructRetAttribute, ByValAttribute
                  };
import syntax::ast;
import back::link;
import common::*;
import build::*;
import base::*;
import type_of::*;
import std::map::hashmap;

export link_name, trans_native_mod, register_crust_fn, trans_crust_fn,
       decl_native_fn;

enum x86_64_reg_class {
    no_class,
    integer_class,
    sse_fs_class,
    sse_fv_class,
    sse_ds_class,
    sse_dv_class,
    sse_int_class,
    sseup_class,
    x87_class,
    x87up_class,
    complex_x87_class,
    memory_class
}

fn is_sse(c: x86_64_reg_class) -> bool {
    ret alt c {
        sse_fs_class | sse_fv_class |
        sse_ds_class | sse_dv_class { true }
        _ { false }
    };
}

fn is_ymm(cls: [x86_64_reg_class]) -> bool {
    let len = vec::len(cls);
    ret (len > 2u &&
         is_sse(cls[0]) &&
         cls[1] == sseup_class &&
         cls[2] == sseup_class) ||
        (len > 3u &&
         is_sse(cls[1]) &&
         cls[2] == sseup_class &&
         cls[3] == sseup_class);
}

fn classify_ty(ty: TypeRef) -> [x86_64_reg_class] {
    fn align(off: uint, ty: TypeRef) -> uint {
        let a = ty_align(ty);
        ret (off + a - 1u) / a * a;
    }

    fn struct_tys(ty: TypeRef) -> [TypeRef] {
        let n = llvm::LLVMCountStructElementTypes(ty);
        let elts = vec::init_elt(n as uint, ptr::null());
        vec::as_buf(elts) {|buf|
            llvm::LLVMGetStructElementTypes(ty, buf);
        }
        ret elts;
    }

    fn ty_align(ty: TypeRef) -> uint {
        ret alt llvm::LLVMGetTypeKind(ty) as int {
            8 /* integer */ {
                ((llvm::LLVMGetIntTypeWidth(ty) as uint) + 7u) / 8u
            }
            12 /* pointer */ { 8u }
            2 /* float */ { 4u }
            3 /* double */ { 8u }
            10 /* struct */ {
                vec::foldl(0u, struct_tys(ty)) {|a, t|
                    uint::max(a, ty_align(t))
                }
            }
            _ {
                fail "ty_size: unhandled type"
            }
        };
    }

    fn ty_size(ty: TypeRef) -> uint {
        ret alt llvm::LLVMGetTypeKind(ty) as int {
            8 /* integer */ {
                ((llvm::LLVMGetIntTypeWidth(ty) as uint) + 7u) / 8u
            }
            12 /* pointer */ { 8u }
            2 /* float */ { 4u }
            3 /* double */ { 8u }
            10 /* struct */ {
                vec::foldl(0u, struct_tys(ty)) {|s, t|
                    s + ty_size(t)
                }
            }
            _ {
                fail "ty_size: unhandled type"
            }
        };
    }

    fn all_mem(cls: [mut x86_64_reg_class]) {
        vec::iteri(cls) {|i, _c|
            cls[i] = memory_class;
        }
    }

    fn unify(cls: [mut x86_64_reg_class], i: uint,
             new: x86_64_reg_class) {
        if cls[i] == new {
            ret;
        } else if cls[i] == no_class {
            cls[i] = new;
        } else if new == no_class {
            ret;
        } else if cls[i] == memory_class || new == memory_class {
            cls[i] = memory_class;
        } else if cls[i] == integer_class || new == integer_class {
            cls[i] = integer_class;
        } else if cls[i] == x87_class ||
                  cls[i] == x87up_class ||
                  cls[i] == complex_x87_class ||
                  new == x87_class ||
                  new == x87up_class ||
                  new == complex_x87_class {
            cls[i] = memory_class;
        } else {
            cls[i] = new;
        }
    }

    fn classify_struct(tys: [TypeRef],
                       cls: [mut x86_64_reg_class], i: uint,
                       off: uint) {
        if vec::is_empty(tys) {
            classify(T_i64(), cls, i, off);
        } else {
            let field_off = off;
            for ty in tys {
                field_off = align(field_off, ty);
                classify(ty, cls, i, field_off);
                field_off += ty_size(ty);
            }
        }
    }

    fn classify(ty: TypeRef,
                cls: [mut x86_64_reg_class], i: uint,
                off: uint) {
        let t_align = ty_align(ty);
        let t_size = ty_size(ty);

        let misalign = off % t_align;
        if misalign != 0u {
            let i = off / 8u;
            let e = (off + t_size + 7u) / 8u;
            while i < e {
                unify(cls, i, memory_class);
                i += 1u;
            }
            ret;
        }

        alt llvm::LLVMGetTypeKind(ty) as int {
            8 /* integer */ |
            12 /* pointer */ {
                unify(cls, off / 8u, integer_class);
            }
            2 /* float */ {
                if off % 8u == 4u {
                    unify(cls, off / 8u, sse_fv_class);
                } else {
                    unify(cls, off / 8u, sse_fs_class);
                }
            }
            3 /* double */ {
                unify(cls, off / 8u, sse_ds_class);
            }
            10 /* struct */ {
                classify_struct(struct_tys(ty), cls, i, off);
            }
            _ {
                fail "classify: unhandled type";
            }
        }
    }

    fn fixup(ty: TypeRef, cls: [mut x86_64_reg_class]) {
        let i = 0u;
        let e = vec::len(cls);
        if vec::len(cls) > 2u &&
           llvm::LLVMGetTypeKind(ty) as int == 10 /* struct */ {
            if is_sse(cls[i]) {
                i += 1u;
                while i < e {
                    if cls[i] != sseup_class {
                        all_mem(cls);
                        ret;
                    }
                    i += 1u;
                }
            } else {
                all_mem(cls);
                ret
            }
        } else {
            while i < e {
                if cls[i] == memory_class {
                    all_mem(cls);
                    ret;
                }
                if cls[i] == x87up_class {
                    // for darwin
                    // cls[i] = sse_ds_class;
                    all_mem(cls);
                    ret;
                }
                if cls[i] == sseup_class {
                    cls[i] = sse_int_class;
                } else if is_sse(cls[i]) {
                    i += 1u;
                    while cls[i] == sseup_class { i += 1u; }
                } else if cls[i] == x87_class {
                    i += 1u;
                    while cls[i] == x87up_class { i += 1u; }
                } else {
                    i += 1u;
                }
            }
        }
    }

    let words = (ty_size(ty) + 7u) / 8u;
    let cls = vec::to_mut(vec::init_elt(words, no_class));
    if words > 4u {
        all_mem(cls);
        ret vec::from_mut(cls);
    }
    classify(ty, cls, 0u, 0u);
    fixup(ty, cls);
    ret vec::from_mut(cls);
}

fn llreg_ty(cls: [x86_64_reg_class]) -> TypeRef {
    fn llvec_len(cls: [x86_64_reg_class]) -> uint {
        let len = 1u;
        for c in cls {
            if c != sseup_class {
                break;
            }
            len += 1u;
        }
        ret len;
    }

    let tys = [];
    let i = 0u;
    let e = vec::len(cls);
    while i < e {
        alt cls[i] {
            integer_class {
                tys += [T_i64()];
            }
            sse_fv_class {
                let vec_len = llvec_len(vec::tailn(cls, i + 1u)) * 2u;
                let vec_ty = llvm::LLVMVectorType(T_f32(),
                                                  vec_len as unsigned);
                tys += [vec_ty];
                i += vec_len;
                cont;
            }
            sse_fs_class {
                tys += [T_f32()];
            }
            sse_ds_class {
                tys += [T_f64()];
            }
            _ {
                fail "llregtype: unhandled class";
            }
        }
        i += 1u;
    }
    ret T_struct(tys);
}

type x86_64_llty = {
    cast: bool,
    ty: TypeRef
};

type x86_64_tys = {
    arg_tys: [x86_64_llty],
    ret_ty: x86_64_llty,
    attrs: [option::t<Attribute>],
    sret: bool
};

fn x86_64_tys(atys: [TypeRef],
              rty: TypeRef,
              ret_def: bool) -> x86_64_tys {
    fn is_reg_ty(ty: TypeRef) -> bool {
        ret alt llvm::LLVMGetTypeKind(ty) as int {
            8 /* integer */ |
            12 /* pointer */ |
            2 /* float */ |
            3 /* double */ { true }
            _ { false }
        };
    }

    fn is_pass_byval(cls: [x86_64_reg_class]) -> bool {
        ret cls[0] == memory_class ||
            cls[0] == x87_class ||
            cls[0] == complex_x87_class;
    }

    fn is_ret_bysret(cls: [x86_64_reg_class]) -> bool {
        ret cls[0] == memory_class;
    }

    fn x86_64_ty(ty: TypeRef,
                 is_mem_cls: fn(cls: [x86_64_reg_class]) -> bool,
                 attr: Attribute) -> (x86_64_llty, option::t<Attribute>) {
        let cast = false;
        let ty_attr = option::none;
        let llty = ty;
        if !is_reg_ty(ty) {
            let cls = classify_ty(ty);
            if is_mem_cls(cls) {
                llty = T_ptr(ty);
                ty_attr = option::some(attr);
            } else {
                cast = true;
                llty = llreg_ty(cls);
            }
        }
        ret ({ cast: cast, ty: llty }, ty_attr);
    }

    let arg_tys = [];
    let attrs = [];
    for t in atys {
        let (ty, attr) = x86_64_ty(t, is_pass_byval, ByValAttribute);
        arg_tys += [ty];
        attrs += [attr];
    }
    let (ret_ty, ret_attr) = x86_64_ty(rty, is_ret_bysret,
                                            StructRetAttribute);
    let sret = option::is_some(ret_attr);
    if sret {
        arg_tys = [ret_ty] + arg_tys;
        ret_ty = { cast:  false,
                   ty: T_void()
                 };
        attrs = [ret_attr] + attrs;
    } else if !ret_def {
        ret_ty = { cast: false,
                   ty: T_void()
                 };
    }
    ret {
        arg_tys: arg_tys,
        ret_ty: ret_ty,
        attrs: attrs,
        sret: sret
    };
}

fn decl_x86_64_fn(tys: x86_64_tys,
                  decl: fn(fnty: TypeRef) -> ValueRef) -> ValueRef {
    let atys = vec::map(tys.arg_tys) {|t| t.ty };
    let rty = tys.ret_ty.ty;
    let fnty = T_fn(atys, rty);
    let llfn = decl(fnty);

    vec::iteri(tys.attrs) {|i, a|
        alt a {
            option::some(attr) {
                let llarg = llvm::LLVMGetParam(llfn, i as unsigned);
                llvm::LLVMAddAttribute(llarg, attr as unsigned);
            }
            _ {}
        }
    }
    ret llfn;
}

fn link_name(i: @ast::native_item) -> str {
    alt attr::get_meta_item_value_str_by_name(i.attrs, "link_name") {
      none { ret i.ident; }
      option::some(ln) { ret ln; }
    }
}

type c_stack_tys = {
    arg_tys: [TypeRef],
    ret_ty: TypeRef,
    ret_def: bool,
    bundle_ty: TypeRef,
    shim_fn_ty: TypeRef,
    x86_64_tys: option::t<x86_64_tys>
};

fn c_arg_and_ret_lltys(ccx: crate_ctxt,
                       id: ast::node_id) -> ([TypeRef], TypeRef, ty::t) {
    alt ty::get(ty::node_id_to_type(ccx.tcx, id)).struct {
      ty::ty_fn({inputs: arg_tys, output: ret_ty, _}) {
        let llargtys = type_of_explicit_args(ccx, arg_tys);
        let llretty = type_of::type_of(ccx, ret_ty);
        (llargtys, llretty, ret_ty)
      }
      _ { ccx.sess.bug("c_arg_and_ret_lltys called on non-function type"); }
    }
}

fn c_stack_tys(ccx: crate_ctxt,
               id: ast::node_id) -> @c_stack_tys {
    let (llargtys, llretty, ret_ty) = c_arg_and_ret_lltys(ccx, id);
    let bundle_ty = T_struct(llargtys + [T_ptr(llretty)]);
    let ret_def = !ty::type_is_bot(ret_ty) && !ty::type_is_nil(ret_ty);
    let x86_64 = if ccx.sess.targ_cfg.arch == arch_x86_64 {
        option::some(x86_64_tys(llargtys, llretty, ret_def))
    } else {
        option::none
    };
    ret @{
        arg_tys: llargtys,
        ret_ty: llretty,
        ret_def: ret_def,
        bundle_ty: bundle_ty,
        shim_fn_ty: T_fn([T_ptr(bundle_ty)], T_void()),
        x86_64_tys: x86_64
    };
}

type shim_arg_builder = fn(bcx: block, tys: @c_stack_tys,
                           llargbundle: ValueRef) -> [ValueRef];

type shim_ret_builder = fn(bcx: block, tys: @c_stack_tys,
                           llargbundle: ValueRef, llretval: ValueRef);

fn build_shim_fn_(ccx: crate_ctxt,
                  shim_name: str,
                  llbasefn: ValueRef,
                  tys: @c_stack_tys,
                  cc: lib::llvm::CallConv,
                  arg_builder: shim_arg_builder,
                  ret_builder: shim_ret_builder) -> ValueRef {

    let llshimfn = decl_internal_cdecl_fn(
        ccx.llmod, shim_name, tys.shim_fn_ty);

    // Declare the body of the shim function:
    let fcx = new_fn_ctxt(ccx, [], llshimfn, none);
    let bcx = top_scope_block(fcx, none);
    let lltop = bcx.llbb;
    let llargbundle = llvm::LLVMGetParam(llshimfn, 0 as c_uint);
    let llargvals = arg_builder(bcx, tys, llargbundle);

    // Create the call itself and store the return value:
    let llretval = CallWithConv(bcx, llbasefn,
                                llargvals, cc); // r

    ret_builder(bcx, tys, llargbundle, llretval);

    build_return(bcx);
    finish_fn(fcx, lltop);

    ret llshimfn;
}

type wrap_arg_builder = fn(bcx: block, tys: @c_stack_tys,
                           llwrapfn: ValueRef,
                           llargbundle: ValueRef);

type wrap_ret_builder = fn(bcx: block, tys: @c_stack_tys,
                           llargbundle: ValueRef);

fn build_wrap_fn_(ccx: crate_ctxt,
                  tys: @c_stack_tys,
                  llshimfn: ValueRef,
                  llwrapfn: ValueRef,
                  shim_upcall: ValueRef,
                  arg_builder: wrap_arg_builder,
                  ret_builder: wrap_ret_builder) {

    let fcx = new_fn_ctxt(ccx, [], llwrapfn, none);
    let bcx = top_scope_block(fcx, none);
    let lltop = bcx.llbb;

    // Allocate the struct and write the arguments into it.
    let llargbundle = alloca(bcx, tys.bundle_ty);
    arg_builder(bcx, tys, llwrapfn, llargbundle);

    // Create call itself.
    let llshimfnptr = PointerCast(bcx, llshimfn, T_ptr(T_i8()));
    let llrawargbundle = PointerCast(bcx, llargbundle, T_ptr(T_i8()));
    Call(bcx, shim_upcall, [llrawargbundle, llshimfnptr]);
    ret_builder(bcx, tys, llargbundle);

    tie_up_header_blocks(fcx, lltop);

    // Make sure our standard return block (that we didn't use) is terminated
    let ret_cx = raw_block(fcx, fcx.llreturn);
    Unreachable(ret_cx);
}

// For each native function F, we generate a wrapper function W and a shim
// function S that all work together.  The wrapper function W is the function
// that other rust code actually invokes.  Its job is to marshall the
// arguments into a struct.  It then uses a small bit of assembly to switch
// over to the C stack and invoke the shim function.  The shim function S then
// unpacks the arguments from the struct and invokes the actual function F
// according to its specified calling convention.
//
// Example: Given a native c-stack function F(x: X, y: Y) -> Z,
// we generate a wrapper function W that looks like:
//
//    void W(Z* dest, void *env, X x, Y y) {
//        struct { X x; Y y; Z *z; } args = { x, y, z };
//        call_on_c_stack_shim(S, &args);
//    }
//
// The shim function S then looks something like:
//
//     void S(struct { X x; Y y; Z *z; } *args) {
//         *args->z = F(args->x, args->y);
//     }
//
// However, if the return type of F is dynamically sized or of aggregate type,
// the shim function looks like:
//
//     void S(struct { X x; Y y; Z *z; } *args) {
//         F(args->z, args->x, args->y);
//     }
//
// Note: on i386, the layout of the args struct is generally the same as the
// desired layout of the arguments on the C stack.  Therefore, we could use
// upcall_alloc_c_stack() to allocate the `args` structure and switch the
// stack pointer appropriately to avoid a round of copies.  (In fact, the shim
// function itself is unnecessary). We used to do this, in fact, and will
// perhaps do so in the future.
fn trans_native_mod(ccx: crate_ctxt,
                    native_mod: ast::native_mod, abi: ast::native_abi) {
    fn build_shim_fn(ccx: crate_ctxt,
                     native_item: @ast::native_item,
                     tys: @c_stack_tys,
                     cc: lib::llvm::CallConv) -> ValueRef {

        fn build_args(bcx: block, tys: @c_stack_tys,
                      llargbundle: ValueRef) -> [ValueRef] {
            let llargvals = [];
            let i = 0u;
            let n = vec::len(tys.arg_tys);

            alt tys.x86_64_tys {
                some(x86_64) {
                    let atys = x86_64.arg_tys;
                    let attrs = x86_64.attrs;
                    if x86_64.sret {
                        let llretptr = GEPi(bcx, llargbundle, [0, n as int]);
                        let llretloc = Load(bcx, llretptr);
                        llargvals = [llretloc];
                        atys = vec::tail(atys);
                        attrs = vec::tail(attrs);
                    }
                    while i < n {
                        let llargval = if atys[i].cast {
                            let arg_ptr = GEPi(bcx, llargbundle,
                                                    [0, i as int]);
                            arg_ptr = BitCast(bcx, arg_ptr,
                                                   T_ptr(atys[i].ty));
                            Load(bcx, arg_ptr)
                        } else if option::is_some(attrs[i]) {
                            GEPi(bcx, llargbundle, [0, i as int])
                        } else {
                            load_inbounds(bcx, llargbundle, [0, i as int])
                        };
                        llargvals += [llargval];
                        i += 1u;
                    }
                }
                _ {
                    while i < n {
                        let llargval = load_inbounds(bcx, llargbundle,
                                                          [0, i as int]);
                        llargvals += [llargval];
                        i += 1u;
                    }
                }
            }
            ret llargvals;
        }

        fn build_ret(bcx: block, tys: @c_stack_tys,
                     llargbundle: ValueRef, llretval: ValueRef)  {
            alt tys.x86_64_tys {
                some(x86_64) {
                    vec::iteri(x86_64.attrs) {|i, a|
                        alt a {
                            some(attr) {
                                llvm::LLVMAddInstrAttribute(
                                    llretval, (i + 1u) as unsigned,
                                              attr as unsigned);
                            }
                            _ {}
                        }
                    }
                    if x86_64.sret || !tys.ret_def {
                        ret;
                    }
                    let n = vec::len(tys.arg_tys);
                    let llretptr = GEPi(bcx, llargbundle, [0, n as int]);
                    let llretloc = Load(bcx, llretptr);
                    if x86_64.ret_ty.cast {
                        let tmp_ptr = BitCast(bcx, llretloc,
                                                   T_ptr(x86_64.ret_ty.ty));
                        Store(bcx, llretval, tmp_ptr);
                    } else {
                        Store(bcx, llretval, llretloc);
                    };
                }
                _ {
                    if tys.ret_def {
                        let n = vec::len(tys.arg_tys);
                        // R** llretptr = &args->r;
                        let llretptr = GEPi(bcx, llargbundle, [0, n as int]);
                        // R* llretloc = *llretptr; /* (args->r) */
                        let llretloc = Load(bcx, llretptr);
                        // *args->r = r;
                        Store(bcx, llretval, llretloc);
                    }
                }
            }
        }

        let lname = link_name(native_item);
        // Declare the "prototype" for the base function F:
        let llbasefn = alt tys.x86_64_tys {
            some(x86_64) {
                decl_x86_64_fn(x86_64) {|fnty|
                    decl_fn(ccx.llmod, lname, cc, fnty)
                }
            }
            _ {
                let llbasefnty = T_fn(tys.arg_tys, tys.ret_ty);
                decl_fn(ccx.llmod, lname, cc, llbasefnty)
            }
        };
        // Name the shim function
        let shim_name = lname + "__c_stack_shim";
        ret build_shim_fn_(ccx, shim_name, llbasefn, tys, cc,
                           build_args, build_ret);
    }

    fn build_wrap_fn(ccx: crate_ctxt,
                     tys: @c_stack_tys,
                     num_tps: uint,
                     llshimfn: ValueRef,
                     llwrapfn: ValueRef) {

        fn build_args(bcx: block, tys: @c_stack_tys,
                      llwrapfn: ValueRef, llargbundle: ValueRef,
                      num_tps: uint) {
            let i = 0u, n = vec::len(tys.arg_tys);
            let implicit_args = first_tp_arg + num_tps; // ret + env
            while i < n {
                let llargval = llvm::LLVMGetParam(
                    llwrapfn,
                    (i + implicit_args) as c_uint);
                store_inbounds(bcx, llargval, llargbundle, [0, i as int]);
                i += 1u;
            }
            let llretptr = llvm::LLVMGetParam(llwrapfn, 0 as c_uint);
            store_inbounds(bcx, llretptr, llargbundle, [0, n as int]);
        }

        fn build_ret(bcx: block, _tys: @c_stack_tys,
                     _llargbundle: ValueRef) {
            RetVoid(bcx);
        }

        build_wrap_fn_(ccx, tys, llshimfn, llwrapfn,
                       ccx.upcalls.call_shim_on_c_stack,
                       bind build_args(_, _ ,_ , _, num_tps),
                       build_ret);
    }

    let cc = lib::llvm::CCallConv;
    alt abi {
      ast::native_abi_rust_intrinsic {
        for item in native_mod.items { get_item_val(ccx, item.id); }
        ret;
      }
      ast::native_abi_cdecl { cc = lib::llvm::CCallConv; }
      ast::native_abi_stdcall { cc = lib::llvm::X86StdcallCallConv; }
    }

    for native_item in native_mod.items {
      alt native_item.node {
        ast::native_item_fn(fn_decl, tps) {
          let id = native_item.id;
          let tys = c_stack_tys(ccx, id);
          let llwrapfn = get_item_val(ccx, id);
          let llshimfn = build_shim_fn(ccx, native_item, tys, cc);
          build_wrap_fn(ccx, tys, vec::len(tps), llshimfn, llwrapfn);
        }
      }
    }
}

fn trans_crust_fn(ccx: crate_ctxt, path: ast_map::path, decl: ast::fn_decl,
                  body: ast::blk, llwrapfn: ValueRef, id: ast::node_id) {

    fn build_rust_fn(ccx: crate_ctxt, path: ast_map::path,
                     decl: ast::fn_decl, body: ast::blk,
                     id: ast::node_id) -> ValueRef {
        let t = ty::node_id_to_type(ccx.tcx, id);
        let ps = link::mangle_internal_name_by_path(
            ccx, path + [ast_map::path_name("__rust_abi")]);
        let llty = type_of_fn_from_ty(ccx, t, []);
        let llfndecl = decl_internal_cdecl_fn(ccx.llmod, ps, llty);
        trans_fn(ccx, path, decl, body, llfndecl, no_self, [], none, id,
                 none);
        ret llfndecl;
    }

    fn build_shim_fn(ccx: crate_ctxt, path: ast_map::path,
                     llrustfn: ValueRef, tys: @c_stack_tys) -> ValueRef {

        fn build_args(bcx: block, tys: @c_stack_tys,
                      llargbundle: ValueRef) -> [ValueRef] {
            let llargvals = [];
            let i = 0u;
            let n = vec::len(tys.arg_tys);
            let llretptr = load_inbounds(bcx, llargbundle, [0, n as int]);
            llargvals += [llretptr];
            let llenvptr = C_null(T_opaque_box_ptr(bcx.ccx()));
            llargvals += [llenvptr];
            while i < n {
                let llargval = load_inbounds(bcx, llargbundle, [0, i as int]);
                llargvals += [llargval];
                i += 1u;
            }
            ret llargvals;
        }

        fn build_ret(_bcx: block, _tys: @c_stack_tys,
                     _llargbundle: ValueRef, _llretval: ValueRef)  {
            // Nop. The return pointer in the Rust ABI function
            // is wired directly into the return slot in the shim struct
        }

        let shim_name = link::mangle_internal_name_by_path(
            ccx, path + [ast_map::path_name("__rust_stack_shim")]);
        ret build_shim_fn_(ccx, shim_name, llrustfn, tys,
                           lib::llvm::CCallConv,
                           build_args, build_ret);
    }

    fn build_wrap_fn(ccx: crate_ctxt, llshimfn: ValueRef,
                     llwrapfn: ValueRef, tys: @c_stack_tys) {

        fn build_args(bcx: block, tys: @c_stack_tys,
                      llwrapfn: ValueRef, llargbundle: ValueRef) {
            alt tys.x86_64_tys {
                option::some(x86_64) {
                    let atys = x86_64.arg_tys;
                    let attrs = x86_64.attrs;
                    let j = 0u;
                    let llretptr = if x86_64.sret {
                        atys = vec::tail(atys);
                        attrs = vec::tail(attrs);
                        j = 1u;
                        llvm::LLVMGetParam(llwrapfn, 0 as c_uint)
                    } else if x86_64.ret_ty.cast {
                        let retptr = alloca(bcx, x86_64.ret_ty.ty);
                        BitCast(bcx, retptr, T_ptr(tys.ret_ty))
                    } else {
                        alloca(bcx, tys.ret_ty)
                    };

                    let i = 0u, n = vec::len(atys);
                    while i < n {
                        let argval = llvm::LLVMGetParam(llwrapfn,
                                                        (i + j) as c_uint);
                        if option::is_some(attrs[i]) {
                            argval = Load(bcx, argval);
                            store_inbounds(bcx, argval, llargbundle,
                                                        [0, i as int]);
                        } else if atys[i].cast {
                            let argptr = GEPi(bcx, llargbundle,
                                                   [0, i as int]);
                            argptr = BitCast(bcx, argptr, T_ptr(atys[i].ty));
                            Store(bcx, argval, argptr);
                        } else {
                            store_inbounds(bcx, argval, llargbundle,
                                                        [0, i as int]);
                        }
                        i += 1u;
                    }
                    store_inbounds(bcx, llretptr, llargbundle, [0, n as int]);
                }
                _ {
                    let llretptr = alloca(bcx, tys.ret_ty);
                    let i = 0u, n = vec::len(tys.arg_tys);
                    while i < n {
                        let llargval = llvm::LLVMGetParam(llwrapfn,
                                                          i as c_uint);
                        store_inbounds(bcx, llargval, llargbundle,
                                                      [0, i as int]);
                        i += 1u;
                    }
                    store_inbounds(bcx, llretptr, llargbundle, [0, n as int]);
                }
            }
        }

        fn build_ret(bcx: block, tys: @c_stack_tys,
                     llargbundle: ValueRef) {
            alt tys.x86_64_tys {
                option::some(x86_64) {
                    if x86_64.sret || !tys.ret_def {
                        RetVoid(bcx);
                        ret;
                    }
                    let n = vec::len(tys.arg_tys);
                    let llretval = load_inbounds(bcx, llargbundle,
                                                      [0, n as int]);
                    let llretval = if x86_64.ret_ty.cast {
                        let retptr = BitCast(bcx, llretval,
                                                  T_ptr(x86_64.ret_ty.ty));
                        Load(bcx, retptr)
                    } else {
                        Load(bcx, llretval)
                    };
                    Ret(bcx, llretval);
                }
                _ {
                    let n = vec::len(tys.arg_tys);
                    let llretval = load_inbounds(bcx, llargbundle,
                                                      [0, n as int]);
                    let llretval = Load(bcx, llretval);
                    Ret(bcx, llretval);
                }
            }
        }

        build_wrap_fn_(ccx, tys, llshimfn, llwrapfn,
                       ccx.upcalls.call_shim_on_rust_stack,
                       build_args, build_ret);
    }

    let tys = c_stack_tys(ccx, id);
    // The internal Rust ABI function - runs on the Rust stack
    let llrustfn = build_rust_fn(ccx, path, decl, body, id);
    // The internal shim function - runs on the Rust stack
    let llshimfn = build_shim_fn(ccx, path, llrustfn, tys);
    // The external C function - runs on the C stack
    build_wrap_fn(ccx, llshimfn, llwrapfn, tys)
}

fn register_crust_fn(ccx: crate_ctxt, sp: span,
                     path: ast_map::path, node_id: ast::node_id)
    -> ValueRef {
    let t = ty::node_id_to_type(ccx.tcx, node_id);
    let (llargtys, llretty, ret_ty) = c_arg_and_ret_lltys(ccx, node_id);
    ret if ccx.sess.targ_cfg.arch == arch_x86_64 {
        let ret_def = !ty::type_is_bot(ret_ty) && !ty::type_is_nil(ret_ty);
        let x86_64 = x86_64_tys(llargtys, llretty, ret_def);
        decl_x86_64_fn(x86_64) {|fnty|
            register_fn_fuller(ccx, sp, path, "crust fn", node_id,
                               t, lib::llvm::CCallConv, fnty)
        }
    } else {
        let llfty = T_fn(llargtys, llretty);
        register_fn_fuller(ccx, sp, path, "crust fn", node_id,
                           t, lib::llvm::CCallConv, llfty)
    }
}

fn abi_of_native_fn(ccx: crate_ctxt, i: @ast::native_item)
    -> ast::native_abi {
    alt attr::get_meta_item_value_str_by_name(i.attrs, "abi") {
      none {
        alt check ccx.tcx.items.get(i.id) {
          ast_map::node_native_item(_, abi, _) { abi }
        }
      }
      some(_) {
        alt attr::native_abi(i.attrs) {
          either::right(abi) { abi }
          either::left(msg) { ccx.sess.span_fatal(i.span, msg); }
        }
      }
    }
}

fn decl_native_fn(ccx: crate_ctxt, i: @ast::native_item,
                  pth: ast_map::path) -> ValueRef {
    alt i.node {
      ast::native_item_fn(_, tps) {
        let node_type = ty::node_id_to_type(ccx.tcx, i.id);
        alt abi_of_native_fn(ccx, i) {
          ast::native_abi_rust_intrinsic {
            // For intrinsics: link the function directly to the intrinsic
            // function itself.
            let fn_type = type_of_fn_from_ty(
                ccx, node_type, param_bounds(ccx, tps));
            let ri_name = "rust_intrinsic_" + native::link_name(i);
            ccx.item_symbols.insert(i.id, ri_name);
            get_extern_fn(ccx.externs, ccx.llmod, ri_name,
                          lib::llvm::CCallConv, fn_type)
          }

          ast::native_abi_cdecl | ast::native_abi_stdcall {
            // For true external functions: create a rust wrapper
            // and link to that.  The rust wrapper will handle
            // switching to the C stack.
            register_fn(ccx, i.span, pth, "native fn", tps, i.id)
          }
        }
      }
    }
}
