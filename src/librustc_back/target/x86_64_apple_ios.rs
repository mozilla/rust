// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use target::Target;
use super::apple_ios_base::{opts, Arch};

pub fn target() -> Target {
    Target {
        data_layout: "e-p:64:64:64-i1:8:8-i8:8:8-i16:16:16-i32:32:32-i64:64:64-\
                      f32:32:32-f64:64:64-v64:64:64-v128:128:128-a0:0:64-\
                      s0:64:64-f80:128:128-n8:16:32:64".to_string(),
        llvm_target: "x86_64-apple-ios".to_string(),
        target_endian: "little".to_string(),
        target_pointer_width: "64".to_string(),
        arch: "x86_64".to_string(),
        target_os: "ios".to_string(),
        options: opts(Arch::X86_64)
    }
}
