// DO NOT EDIT: autogenerated by etc/platform-intrinsics/generator.py
// ignore-tidy-linelength

#![allow(unused_imports)]

use IntrinsicDef::Named;
use {Intrinsic, Type};

pub fn find(name: &str) -> Option<Intrinsic> {
    if !name.starts_with("nvptx") {
        return None;
    }
    Some(match &name["nvptx".len()..] {
        "_syncthreads" => Intrinsic {
            inputs: {
                static INPUTS: [&'static Type; 0] = [];
                &INPUTS
            },
            output: &::VOID,
            definition: Named("llvm.cuda.syncthreads"),
        },
        "_block_dim_x" => Intrinsic {
            inputs: {
                static INPUTS: [&'static Type; 0] = [];
                &INPUTS
            },
            output: &::I32,
            definition: Named("llvm.nvvm.read.ptx.sreg.ntid.x"),
        },
        "_block_dim_y" => Intrinsic {
            inputs: {
                static INPUTS: [&'static Type; 0] = [];
                &INPUTS
            },
            output: &::I32,
            definition: Named("llvm.nvvm.read.ptx.sreg.ntid.y"),
        },
        "_block_dim_z" => Intrinsic {
            inputs: {
                static INPUTS: [&'static Type; 0] = [];
                &INPUTS
            },
            output: &::I32,
            definition: Named("llvm.nvvm.read.ptx.sreg.ntid.z"),
        },
        "_block_idx_x" => Intrinsic {
            inputs: {
                static INPUTS: [&'static Type; 0] = [];
                &INPUTS
            },
            output: &::I32,
            definition: Named("llvm.nvvm.read.ptx.sreg.ctaid.x"),
        },
        "_block_idx_y" => Intrinsic {
            inputs: {
                static INPUTS: [&'static Type; 0] = [];
                &INPUTS
            },
            output: &::I32,
            definition: Named("llvm.nvvm.read.ptx.sreg.ctaid.y"),
        },
        "_block_idx_z" => Intrinsic {
            inputs: {
                static INPUTS: [&'static Type; 0] = [];
                &INPUTS
            },
            output: &::I32,
            definition: Named("llvm.nvvm.read.ptx.sreg.ctaid.z"),
        },
        "_grid_dim_x" => Intrinsic {
            inputs: {
                static INPUTS: [&'static Type; 0] = [];
                &INPUTS
            },
            output: &::I32,
            definition: Named("llvm.nvvm.read.ptx.sreg.nctaid.x"),
        },
        "_grid_dim_y" => Intrinsic {
            inputs: {
                static INPUTS: [&'static Type; 0] = [];
                &INPUTS
            },
            output: &::I32,
            definition: Named("llvm.nvvm.read.ptx.sreg.nctaid.y"),
        },
        "_grid_dim_z" => Intrinsic {
            inputs: {
                static INPUTS: [&'static Type; 0] = [];
                &INPUTS
            },
            output: &::I32,
            definition: Named("llvm.nvvm.read.ptx.sreg.nctaid.z"),
        },
        "_thread_idx_x" => Intrinsic {
            inputs: {
                static INPUTS: [&'static Type; 0] = [];
                &INPUTS
            },
            output: &::I32,
            definition: Named("llvm.nvvm.read.ptx.sreg.tid.x"),
        },
        "_thread_idx_y" => Intrinsic {
            inputs: {
                static INPUTS: [&'static Type; 0] = [];
                &INPUTS
            },
            output: &::I32,
            definition: Named("llvm.nvvm.read.ptx.sreg.tid.y"),
        },
        "_thread_idx_z" => Intrinsic {
            inputs: {
                static INPUTS: [&'static Type; 0] = [];
                &INPUTS
            },
            output: &::I32,
            definition: Named("llvm.nvvm.read.ptx.sreg.tid.z"),
        },
        _ => return None,
    })
}
