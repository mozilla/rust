// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn main() {
    concat!(b'f');    // b'' concat is supported
    concat!(b"foo");  // b"" concat is supported
    concat!(foo);
    //~^ ERROR: expected a literal
    concat!(foo());
    //~^ ERROR: expected a literal
    concat!(b'a', b"bc", b"def");
    concat!("abc", b"def", 'g', "hi", b"jkl");
    //~^ ERROR cannot concatenate a byte string literal with other literals
    // `concat!()` cannot mix "" and b"" literals (it might allow it in the future)
    concat!(1, b"def");
    //~^ ERROR cannot concatenate a byte string literal with other literals
    concat!(true, b"def");
    //~^ ERROR cannot concatenate a byte string literal with other literals
    concat!(1, true, b"def");
    //~^ ERROR cannot concatenate a byte string literal with other literals
    concat!(1, true, "abc", b"def");
    //~^ ERROR cannot concatenate a byte string literal with other literals
    concat!(true, "abc", b"def");
    //~^ ERROR cannot concatenate a byte string literal with other literals
    concat!(1, "abc", b"def");
    //~^ ERROR cannot concatenate a byte string literal with other literals
}
