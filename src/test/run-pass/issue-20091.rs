// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// ignore-windows currently windows requires UTF-8 for spawning processes

use std::io::Command;
use std::os;

fn main() {
    if os::args().len() == 1 {
        assert!(Command::new(os::self_exe_name().unwrap()).arg(b"\xff")
                        .status().unwrap().success())
    }
}
