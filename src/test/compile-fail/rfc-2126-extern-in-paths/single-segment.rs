// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// aux-build:xcrate.rs
// rustc-env:RUST_LOG=info

#![feature(extern_in_paths)]

use extern; //~ ERROR unresolved import `extern`
            //~^ NOTE no `extern` in the root
use extern::*; //~ ERROR unresolved import `extern::*`
               //~^ NOTE cannot glob-import all possible crates

fn main() {
    let s = extern::xcrate; //~ ERROR expected value, found module `extern::xcrate`
                            //~^ NOTE not a value
}
//~^ ERROR should fail kthxbye
