// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::unstable;

fn main() {
    unsafe {
        let x = Some(unstable::sync::exclusive(false));
        match x {
            Some(copy z) => { //~ ERROR copying a value of non-copyable type
                do z.with |b| { assert!(!*b); }
            }
            None => fail!()
        }
    }
}
