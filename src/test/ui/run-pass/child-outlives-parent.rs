// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// run-pass

// Reported as issue #126, child leaks the string.

// pretty-expanded FIXME #23616
// ignore-emscripten no threads support

use std::thread;

fn child2(_s: String) { }

pub fn main() {
    let _x = thread::spawn(move|| child2("hi".to_string()));
}
