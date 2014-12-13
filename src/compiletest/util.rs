// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use common::Config;

#[cfg(target_os = "windows")]
use std::os::getenv;

/// Conversion table from triple OS name to Rust SYSNAME
static OS_TABLE: &'static [(&'static str, &'static str)] = &[
    ("mingw32", "windows"),
    ("win32", "windows"),
    ("windows", "windows"),
    ("darwin", "macos"),
    ("android", "android"),
    ("linux", "linux"),
    ("freebsd", "freebsd"),
    ("dragonfly", "dragonfly"),
];

/// Table to help extracting architecture from triple
static ARCH_TABLE: &'static [&'static str] = &[
    "arm",
    "mips",
    "mipsel",
    "x86",
    "x86_64",
];

pub fn get_os(triple: &str) -> &'static str {
    for &(triple_os, os) in OS_TABLE.iter() {
        if triple.contains(triple_os) {
            return os
        }
    }
    panic!("Cannot determine OS from triple");
}

pub fn get_arch(triple: &str) -> &'static str {
    for &triple_arch in ARCH_TABLE.iter() {
        if triple.contains(triple_arch) {
            return triple_arch
        }
    }
    panic!("Cannot determine architecture from triple");
}

#[cfg(target_os = "windows")]
pub fn make_new_path(path: &str) -> String {

    // Windows just uses PATH as the library search path, so we have to
    // maintain the current value while adding our own
    match getenv(lib_path_env_var()) {
      Some(curr) => {
        format!("{}{}{}", path, path_div(), curr)
      }
      None => path.to_string()
    }
}

#[cfg(target_os = "windows")]
pub fn lib_path_env_var() -> &'static str { "PATH" }

#[cfg(target_os = "windows")]
pub fn path_div() -> &'static str { ";" }

pub fn logv(config: &Config, s: String) {
    debug!("{}", s);
    if config.verbose { println!("{}", s); }
}
