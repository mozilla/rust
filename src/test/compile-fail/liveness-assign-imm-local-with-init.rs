// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn test() {
    let v: isize = 1; //~ NOTE first assignment
    v.clone();
    v = 2; //~ ERROR cannot assign twice to immutable variable
           //~| NOTE cannot assign twice to immutable
    v.clone();
}

fn main() {
}
