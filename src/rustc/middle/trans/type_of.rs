import common::*;
import lib::llvm::{TypeRef};
import syntax::ast;
import lib::llvm::llvm;
import driver::session::session;
import std::map::hashmap;

export type_of;
export type_of_dtor;
export type_of_explicit_args;
export type_of_fn_from_ty;
export type_of_fn;
export type_of_glue_fn;
export type_of_non_gc_box;
export type_of_rooted;

fn type_of_explicit_args(cx: @crate_ctxt,
                         inputs: ~[ty::arg]) -> ~[TypeRef] {
    do vec::map(inputs) |arg| {
        let arg_ty = arg.ty;
        let llty = type_of(cx, arg_ty);
        match ty::resolved_mode(cx.tcx, arg.mode) {
          ast::by_val => llty,
          _ => T_ptr(llty)
        }
    }
}

fn type_of_fn(cx: @crate_ctxt, inputs: ~[ty::arg],
              output: ty::t) -> TypeRef {
    let mut atys: ~[TypeRef] = ~[];

    // Arg 0: Output pointer.
    vec::push(atys, T_ptr(type_of(cx, output)));

    // Arg 1: Environment
    vec::push(atys, T_opaque_box_ptr(cx));

    // ... then explicit args.
    vec::push_all(atys, type_of_explicit_args(cx, inputs));
    return T_fn(atys, llvm::LLVMVoidType());
}

// Given a function type and a count of ty params, construct an llvm type
fn type_of_fn_from_ty(cx: @crate_ctxt, fty: ty::t) -> TypeRef {
    type_of_fn(cx, ty::ty_fn_args(fty), ty::ty_fn_ret(fty))
}

fn type_of_non_gc_box(cx: @crate_ctxt, t: ty::t) -> TypeRef {
    assert !ty::type_needs_infer(t);

    let t_norm = ty::normalize_ty(cx.tcx, t);
    if t != t_norm {
        type_of_non_gc_box(cx, t_norm)
    } else {
        match ty::get(t).struct {
          ty::ty_box(mt) => {
            T_ptr(T_box(cx, type_of(cx, mt.ty)))
          }
          ty::ty_uniq(mt) => {
            T_ptr(T_unique(cx, type_of(cx, mt.ty)))
          }
          _ => {
            cx.sess.bug(~"non-box in type_of_non_gc_box");
          }
        }
    }
}

fn type_of(cx: @crate_ctxt, t: ty::t) -> TypeRef {
    debug!("type_of %?: %?", t, ty::get(t));

    // Check the cache.
    if cx.lltypes.contains_key(t) { return cx.lltypes.get(t); }

    // Replace any typedef'd types with their equivalent non-typedef
    // type. This ensures that all LLVM nominal types that contain
    // Rust types are defined as the same LLVM types.  If we don't do
    // this then, e.g. `option<{myfield: bool}>` would be a different
    // type than `option<myrec>`.
    let t_norm = ty::normalize_ty(cx.tcx, t);

    if t != t_norm {
        let llty = type_of(cx, t_norm);
        cx.lltypes.insert(t, llty);
        return llty;
    }

    let llty = match ty::get(t).struct {
      ty::ty_nil | ty::ty_bot => T_nil(),
      ty::ty_bool => T_bool(),
      ty::ty_int(t) => T_int_ty(cx, t),
      ty::ty_uint(t) => T_uint_ty(cx, t),
      ty::ty_float(t) => T_float_ty(cx, t),
      ty::ty_estr(ty::vstore_uniq) => {
        T_unique_ptr(T_unique(cx, T_vec(cx, T_i8())))
      }
      ty::ty_enum(did, substs) => {
        // Only create the named struct, but don't fill it in. We
        // fill it in *after* placing it into the type cache. This
        // avoids creating more than one copy of the enum when one
        // of the enum's variants refers to the enum itself.

        common::T_named_struct(llvm_type_name(cx, an_enum, did, substs.tps))
      }
      ty::ty_estr(ty::vstore_box) => {
        T_box_ptr(T_box(cx, T_vec(cx, T_i8())))
      }
      ty::ty_evec(mt, ty::vstore_box) => {
        T_box_ptr(T_box(cx, T_vec(cx, type_of(cx, mt.ty))))
      }
      ty::ty_box(mt) => T_box_ptr(T_box(cx, type_of(cx, mt.ty))),
      ty::ty_opaque_box => T_box_ptr(T_box(cx, T_i8())),
      ty::ty_uniq(mt) => T_unique_ptr(T_unique(cx, type_of(cx, mt.ty))),
      ty::ty_evec(mt, ty::vstore_uniq) => {
        T_unique_ptr(T_unique(cx, T_vec(cx, type_of(cx, mt.ty))))
      }
      ty::ty_unboxed_vec(mt) => {
        T_vec(cx, type_of(cx, mt.ty))
      }
      ty::ty_ptr(mt) => T_ptr(type_of(cx, mt.ty)),
      ty::ty_rptr(_, mt) => T_ptr(type_of(cx, mt.ty)),

      ty::ty_evec(mt, ty::vstore_slice(_)) => {
        T_struct(~[T_ptr(type_of(cx, mt.ty)),
                   T_uint_ty(cx, ast::ty_u)])
      }

      ty::ty_estr(ty::vstore_slice(_)) => {
        T_struct(~[T_ptr(T_i8()),
                   T_uint_ty(cx, ast::ty_u)])
      }

      ty::ty_estr(ty::vstore_fixed(n)) => {
        T_array(T_i8(), n + 1u /* +1 for trailing null */)
      }

      ty::ty_evec(mt, ty::vstore_fixed(n)) => {
        T_array(type_of(cx, mt.ty), n)
      }

      ty::ty_rec(fields) => {
        let mut tys: ~[TypeRef] = ~[];
        for vec::each(fields) |f| {
            let mt_ty = f.mt.ty;
            vec::push(tys, type_of(cx, mt_ty));
        }
        T_struct(tys)
      }
      ty::ty_fn(_) => T_fn_pair(cx, type_of_fn_from_ty(cx, t)),
      ty::ty_trait(_, _, _) => T_opaque_trait(cx),
      ty::ty_type => T_ptr(cx.tydesc_type),
      ty::ty_tup(elts) => {
        let mut tys = ~[];
        for vec::each(elts) |elt| {
            vec::push(tys, type_of(cx, elt));
        }
        T_struct(tys)
      }
      ty::ty_opaque_closure_ptr(_) => T_opaque_box_ptr(cx),
      ty::ty_class(did, substs) => {
        // Only create the named struct, but don't fill it in. We fill it
        // in *after* placing it into the type cache. This prevents
        // infinite recursion with recursive class types.

        common::T_named_struct(llvm_type_name(cx, a_class, did, substs.tps))
      }
      ty::ty_self => cx.tcx.sess.unimpl(~"type_of: ty_self"),
      ty::ty_var(_) => cx.tcx.sess.bug(~"type_of with ty_var"),
      ty::ty_param(*) => cx.tcx.sess.bug(~"type_of with ty_param"),
      ty::ty_var_integral(_) => {
        cx.tcx.sess.bug(~"type_of shouldn't see a ty_var_integral");
      }
    };

    cx.lltypes.insert(t, llty);

    // If this was an enum or class, fill in the type now.
    match ty::get(t).struct {
      ty::ty_enum(did, _) => {
        fill_type_of_enum(cx, did, t, llty);
      }
      ty::ty_class(did, ref substs) => {
        // Only instance vars are record fields at runtime.
        let fields = ty::lookup_class_fields(cx.tcx, did);
        let mut tys = do vec::map(fields) |f| {
            let t = ty::lookup_field_type(cx.tcx, did, f.id, substs);
            type_of(cx, t)
        };

        if ty::ty_dtor(cx.tcx, did) != None {
            // resource type
            tys = ~[T_i8(), T_struct(tys)];
        }

        common::set_struct_body(llty, tys);
      }
      _ => ()
    }

    return llty;
}

fn fill_type_of_enum(cx: @crate_ctxt, did: ast::def_id, t: ty::t,
                     llty: TypeRef) {

    debug!("type_of_enum %?: %?", t, ty::get(t));

    let lltys = {
        let degen = (*ty::enum_variants(cx.tcx, did)).len() == 1u;
        let size = shape::static_size_of_enum(cx, t);
        if !degen {
            ~[T_enum_discrim(cx), T_array(T_i8(), size)]
        }
        else if size == 0u {
            ~[T_enum_discrim(cx)]
        }
        else {
            ~[T_array(T_i8(), size)]
        }
    };

    common::set_struct_body(llty, lltys);
}

// Want refinements! (Or case classes, I guess
enum named_ty { a_class, an_enum }

fn llvm_type_name(cx: @crate_ctxt,
                  what: named_ty,
                  did: ast::def_id,
                  tps: ~[ty::t]
                  ) -> ~str {
    let name = match what { a_class => { "~class" } an_enum => { "~enum" } };
    return fmt!(
        "%s %s[#%d]",
          name,
        util::ppaux::parameterized(
            cx.tcx,
            ty::item_path_str(cx.tcx, did),
            None,
            tps),
        did.crate
    );
}

fn type_of_dtor(ccx: @crate_ctxt, self_ty: ty::t) -> TypeRef {
    T_fn(~[T_ptr(type_of(ccx, ty::mk_nil(ccx.tcx))),
          T_ptr(type_of(ccx, self_ty))],
         llvm::LLVMVoidType())
}

fn type_of_rooted(ccx: @crate_ctxt, t: ty::t) -> TypeRef {
    let addrspace = base::get_tydesc(ccx, t).addrspace;
    debug!("type_of_rooted %s in addrspace %u",
           ty_to_str(ccx.tcx, t), addrspace as uint);
    return T_root(type_of(ccx, t), addrspace);
}

fn type_of_glue_fn(ccx: @crate_ctxt, t: ty::t) -> TypeRef {
    let tydescpp = T_ptr(T_ptr(ccx.tydesc_type));
    let llty = T_ptr(type_of(ccx, t));
    return T_fn(~[T_ptr(T_nil()), T_ptr(T_nil()), tydescpp, llty],
                T_void());
}
