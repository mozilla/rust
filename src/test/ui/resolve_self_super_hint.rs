// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(alloc)]
#![allow(unused_extern_crates)]

mod a {
    extern crate alloc;
    use alloc::HashMap;
    //~^ ERROR unresolved import `alloc` [E0432]
    //~| did you mean `self::alloc`?
    mod b {
        use alloc::HashMap;
        //~^ ERROR unresolved import `alloc` [E0432]
        //~| did you mean `super::alloc`?
        mod c {
            use alloc::HashMap;
            //~^ ERROR unresolved import `alloc` [E0432]
            //~| did you mean `a::alloc`?
            mod d {
                use alloc::HashMap;
                //~^ ERROR unresolved import `alloc` [E0432]
                //~| did you mean `a::alloc`?
            }
        }
    }
}

fn main() {}
