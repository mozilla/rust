// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn with_int(f: |x: &int|) {
    let x = 3;
    f(&x);
}

fn main() {
    let mut x = None;
         //~^ ERROR lifetime of variable does not enclose its declaration
         //~^^ ERROR type of expression contains references that are not valid during the expression
    with_int(|y| x = Some(y));
}
