// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// check-search-index

#![crate_type = "lib"]
#![feature(globs)]

mod m {
    pub use self::a::Foo;

    mod a {
        pub struct Foo;
    }

    mod b {
        pub use super::*;
    }
}

/* !search-index
{
    "recursion2": {}
}
*/
