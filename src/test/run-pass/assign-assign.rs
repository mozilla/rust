// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Issue 483 - Assignment expressions result in nil
fn test_assign() {
    let mut x: int;
    let y: () = x = 10;
    fail_unless_eq!(x, 10);
    fail_unless_eq!(y, ());
    let mut z = x = 11;
    fail_unless_eq!(x, 11);
    fail_unless_eq!(z, ());
    z = x = 12;
    fail_unless_eq!(x, 12);
    fail_unless_eq!(z, ());
}

fn test_assign_op() {
    let mut x: int = 0;
    let y: () = x += 10;
    fail_unless_eq!(x, 10);
    fail_unless_eq!(y, ());
    let mut z = x += 11;
    fail_unless_eq!(x, 21);
    fail_unless_eq!(z, ());
    z = x += 12;
    fail_unless_eq!(x, 33);
    fail_unless_eq!(z, ());
}

pub fn main() { test_assign(); test_assign_op(); }
