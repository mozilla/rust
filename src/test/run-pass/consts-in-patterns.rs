// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

static FOO: int = 10;
static BAR: int = 3;

pub fn main() {
    let x: int = 3;
    let y = match x {
        FOO => 1,
        BAR => 2,
        _ => 3
    };
    fail_unless_eq!(y, 2);
}
