// Copyright 2014-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use libc::c_uint;
use llvm;
use llvm::{Integer, Pointer, Float, Double, Struct, Array, Attribute};
use trans::cabi::{FnType, ArgType};
use trans::context::CrateContext;
use trans::type_::Type;

use std::cmp;

fn align_up_to(off: usize, a: usize) -> usize {
    return (off + a - 1) / a * a;
}

fn align(off: usize, ty: Type) -> usize {
    let a = ty_align(ty);
    return align_up_to(off, a);
}

fn ty_align(ty: Type) -> usize {
    match ty.kind() {
        Integer => {
            unsafe {
                ((llvm::LLVMGetIntTypeWidth(ty.to_ref()) as usize) + 7) / 8
            }
        }
        Pointer => 4,
        Float => 4,
        Double => 8,
        Struct => {
          if ty.is_packed() {
            1
          } else {
            let str_tys = ty.field_types();
            str_tys.iter().fold(1, |a, t| cmp::max(a, ty_align(*t)))
          }
        }
        Array => {
            let elt = ty.element_type();
            ty_align(elt)
        }
        _ => panic!("ty_size: unhandled type")
    }
}

fn ty_size(ty: Type) -> usize {
    match ty.kind() {
        Integer => {
            unsafe {
                ((llvm::LLVMGetIntTypeWidth(ty.to_ref()) as usize) + 7) / 8
            }
        }
        Pointer => 4,
        Float => 4,
        Double => 8,
        Struct => {
            if ty.is_packed() {
                let str_tys = ty.field_types();
                str_tys.iter().fold(0, |s, t| s + ty_size(*t))
            } else {
                let str_tys = ty.field_types();
                let size = str_tys.iter().fold(0, |s, t| align(s, *t) + ty_size(*t));
                align(size, ty)
            }
        }
        Array => {
            let len = ty.array_length();
            let elt = ty.element_type();
            let eltsz = ty_size(elt);
            len * eltsz
        }
        _ => panic!("ty_size: unhandled type")
    }
}

fn classify_ret_ty(_ccx: &CrateContext, ty: Type) -> ArgType {
    if is_reg_ty(ty) {
        if ty.kind() == Integer && ty.int_width() < 32 {
            ArgType::extend(ty)
        } else {
            ArgType::direct(ty, None, None, None)
        }
    } else {
        ArgType::indirect(ty, Some(Attribute::StructRet))
    }
}

fn classify_arg_ty(ccx: &CrateContext, ty: Type, offset: &mut usize) -> ArgType {
    let orig_offset = *offset;
    let size = ty_size(ty) * 8;
    let mut align = ty_align(ty);

    align = cmp::min(cmp::max(align, 4), 8);
    *offset = align_up_to(*offset, align);
    *offset += align_up_to(size, align * 8) / 8;

    if is_reg_ty(ty) {
        if ty.kind() == Integer && ty.int_width() < 32 {
            ArgType::extend(ty)
        } else {
            ArgType::direct(ty, None, None, None)
        }
    } else {
        ArgType::direct(
            ty,
            Some(struct_ty(ccx, ty)),
            padding_ty(ccx, align, orig_offset),
            None
        )
    }
}

fn is_reg_ty(ty: Type) -> bool {
    return match ty.kind() {
        Integer
        | Pointer
        | Float
        | Double => true,
        _ => false
    };
}

fn padding_ty(ccx: &CrateContext, align: usize, offset: usize) -> Option<Type> {
    if ((align - 1 ) & offset) > 0 {
        Some(Type::i32(ccx))
    } else {
        None
    }
}

fn coerce_to_int(ccx: &CrateContext, size: usize) -> Vec<Type> {
    let int_ty = Type::i32(ccx);
    let mut args = Vec::new();

    let mut n = size / 32;
    while n > 0 {
        args.push(int_ty);
        n -= 1;
    }

    let r = size % 32;
    if r > 0 {
        unsafe {
            args.push(Type::from_ref(llvm::LLVMIntTypeInContext(ccx.llcx(), r as c_uint)));
        }
    }

    args
}

fn struct_ty(ccx: &CrateContext, ty: Type) -> Type {
    let size = ty_size(ty) * 8;
    Type::struct_(ccx, &coerce_to_int(ccx, size), false)
}

pub fn compute_abi_info(ccx: &CrateContext,
                        atys: &[Type],
                        rty: Type,
                        ret_def: bool) -> FnType {
    let ret_ty = if ret_def {
        classify_ret_ty(ccx, rty)
    } else {
        ArgType::direct(Type::void(ccx), None, None, None)
    };

    let sret = ret_ty.is_indirect();
    let mut arg_tys = Vec::new();
    let mut offset = if sret { 4 } else { 0 };

    for aty in atys {
        let ty = classify_arg_ty(ccx, *aty, &mut offset);
        arg_tys.push(ty);
    };

    return FnType {
        arg_tys: arg_tys,
        ret_ty: ret_ty,
    };
}
