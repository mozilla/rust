// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use target::{Target, TargetOptions};
use super::apple_ios_base::{Arch, opts};

pub fn target() -> Target {
    Target {
        llvm_target: "arm64-apple-ios".to_string(),
        target_endian: "little".to_string(),
        target_pointer_width: "64".to_string(),
        data_layout: "e-m:o-i64:64-i128:128-n32:64-S128".to_string(),
        arch: "aarch64".to_string(),
        target_os: "ios".to_string(),
        target_env: "".to_string(),
        target_vendor: "apple".to_string(),
        options: TargetOptions {
            features: "+neon,+fp-armv8,+cyclone".to_string(),
            eliminate_frame_pointer: false,
            max_atomic_width: 128,
            ..opts(Arch::Arm64)
        },
    }
}
