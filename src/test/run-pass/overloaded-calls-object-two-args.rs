// Copyright 2014-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Tests calls to closure arguments where the closure takes 2 arguments.
// This is a bit tricky due to rust-call ABI.


fn foo(f: &mut FnMut(isize, isize) -> isize) -> isize {
    f(1, 2)
}

fn main() {
    let z = foo(&mut |x, y| x * 10 + y);
    assert_eq!(z, 12);
}
