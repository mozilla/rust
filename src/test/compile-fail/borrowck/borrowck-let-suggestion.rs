// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn f() {
    let x = [1].iter(); //~ ERROR borrowed value does not live long enough
                         //~^ NOTE reference must be valid for the block suffix following statement
                         //~^^ HELP consider using a `let` binding to increase its lifetime
                         //~^^^ NOTE ...but borrowed value is only valid for the statement at 12:4
}

fn main() {
    f();
}
