// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn f<T>() -> bool {
    struct S(T); //~ ERROR use of undeclared type name `T`
    //~^ ERROR attempt to use a type argument out of scope

    true
}

fn main() {
    let b = f::<int>();
    assert!(b);
}
