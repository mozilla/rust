// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn bar(blk: &fn:'static()) {
}

fn foo(x: &()) {
    do bar {
        let _ = x; //~ ERROR cannot capture variable of type `&()`, which does not fulfill `'static`, in a bounded closure
        //~^ NOTE this closure's environment must satisfy `'static`
    }
}

fn main() {
}
