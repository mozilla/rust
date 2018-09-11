// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// run-pass

// Test old and new syntax for inclusive range patterns.

fn main() {
    assert!(match 42 { 0 ... 100 => true, _ => false });
    assert!(match 42 { 0 ..= 100 => true, _ => false });

    assert!(match 'x' { 'a' ... 'z' => true, _ => false });
    assert!(match 'x' { 'a' ..= 'z' => true, _ => false });
}

