// Copyright 2013-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use super::{Target, TargetOptions, CrossTarget};

use std::path::PathBuf;

#[cfg(not(target_os = "nacl"))]
fn get_cross_target() -> Option<CrossTarget> {
    fn pnacl_toolchain(mut cross_path: PathBuf) -> Result<PathBuf, String> {
        #[cfg(windows)]
        fn get() -> Result<&'static str, String> { Ok("win") }
        #[cfg(target_os = "linux")]
        fn get() -> Result<&'static str, String> { Ok("linux") }
        #[cfg(target_os = "macos")]
        fn get() -> Result<&'static str, String> { Ok("mac") }
        #[cfg(all(not(windows),
                  not(target_os = "linux"),
                  not(target_os = "macos"),
                  not(target_os = "nacl")))]
        fn get() -> Result<&'static str, String> {
            Err("the NaCl/PNaCl toolchain is unsupported on this platform \
                 (update this if that's changed)".to_string());
        }

        cross_path.push("toolchain");
        cross_path.push(&format!("{}_pnacl", try!(get())));
        cross_path.push("bin");
        Ok(cross_path)
    }

    Some(CrossTarget {
        toolchain_env_key: Some(From::from("NACL_SDK_ROOT")),
        get_tool_bin_path: Some(pnacl_toolchain),
    })
}
#[cfg(target_os = "nacl")]
fn get_cross_target() -> Option<CrossTarget> { None }

pub fn target() -> Target {
    let opts = TargetOptions {
        linker: "pnacl-ld".to_string(),
        ar: "pnacl-ar".to_string(),

        pre_link_args: vec!("--pnacl-exceptions=sjlj".to_string()),

        dynamic_linking: false,
        executables: true,
        morestack: false,
        exe_suffix: ".pexe".to_string(),
        no_compiler_rt: true,
        linker_is_gnu: true,
        is_like_pnacl: true,
        no_asm: true,
        lto_supported: false, // `pnacl-ld` runs "LTO".
        requires_cross_path: cfg!(not(target_os = "nacl")),
        .. Default::default()
    };
    Target {
        data_layout: "e-i1:8:8-i8:8:8-i16:16:16-i32:32:32-\
                      i64:64:64-f32:32:32-f64:64:64-p:32:32:32-v128:32:32".to_string(),
        // Pretend that we are ARM for name mangling and assembly conventions.
        // https://code.google.com/p/nativeclient/issues/detail?id=2554
        llvm_target: "armv7a-none-nacl-gnueabi".to_string(),
        target_endian: "little".to_string(),
        target_pointer_width: "32".to_string(),
        target_os: "nacl".to_string(),
        target_env: "".to_string(),
        arch: "le32".to_string(),
        options: opts,
        cross: get_cross_target(),
    }
}
