// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// DO NOT EDIT: autogenerated by etc/platform-intrinsics/generator.py
// ignore-tidy-linelength

#![allow(unused_imports)]

use {Intrinsic, Type};
use IntrinsicDef::Named;

// The default inlining settings trigger a pathological behaviour in
// LLVM, which causes makes compilation very slow. See #28273.
#[inline(never)]
pub(crate) fn find(name: &str) -> Option<Intrinsic> {
    if !name.starts_with("nvptx") { return None }
    Some(match &name["nvptx".len()..] {
        "_syncthreads" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 0] = []; &INPUTS },
            output: &::VOID,
            definition: Named("llvm.cuda.syncthreads")
        },
        "_block_dim_x" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 0] = []; &INPUTS },
            output: &::I32,
            definition: Named("llvm.nvvm.read.ptx.sreg.ntid.x")
        },
        "_block_dim_y" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 0] = []; &INPUTS },
            output: &::I32,
            definition: Named("llvm.nvvm.read.ptx.sreg.ntid.y")
        },
        "_block_dim_z" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 0] = []; &INPUTS },
            output: &::I32,
            definition: Named("llvm.nvvm.read.ptx.sreg.ntid.z")
        },
        "_block_idx_x" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 0] = []; &INPUTS },
            output: &::I32,
            definition: Named("llvm.nvvm.read.ptx.sreg.ctaid.x")
        },
        "_block_idx_y" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 0] = []; &INPUTS },
            output: &::I32,
            definition: Named("llvm.nvvm.read.ptx.sreg.ctaid.y")
        },
        "_block_idx_z" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 0] = []; &INPUTS },
            output: &::I32,
            definition: Named("llvm.nvvm.read.ptx.sreg.ctaid.z")
        },
        "_grid_dim_x" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 0] = []; &INPUTS },
            output: &::I32,
            definition: Named("llvm.nvvm.read.ptx.sreg.nctaid.x")
        },
        "_grid_dim_y" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 0] = []; &INPUTS },
            output: &::I32,
            definition: Named("llvm.nvvm.read.ptx.sreg.nctaid.y")
        },
        "_grid_dim_z" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 0] = []; &INPUTS },
            output: &::I32,
            definition: Named("llvm.nvvm.read.ptx.sreg.nctaid.z")
        },
        "_thread_idx_x" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 0] = []; &INPUTS },
            output: &::I32,
            definition: Named("llvm.nvvm.read.ptx.sreg.tid.x")
        },
        "_thread_idx_y" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 0] = []; &INPUTS },
            output: &::I32,
            definition: Named("llvm.nvvm.read.ptx.sreg.tid.y")
        },
        "_thread_idx_z" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 0] = []; &INPUTS },
            output: &::I32,
            definition: Named("llvm.nvvm.read.ptx.sreg.tid.z")
        },
        _ => return None,
    })
}
