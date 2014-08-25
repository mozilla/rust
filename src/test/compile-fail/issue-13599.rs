// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Test that a mismatched proc / closure type is correctly reported.

fn expect_closure(_: ||) {}

fn expect_proc(_: proc()) {}

fn main() {
    expect_closure(proc() {});
    //~^ ERROR mismatched types: expected `||`, found `proc()` (expected closure, found proc)

    expect_proc(|| {});
    //~^ ERROR mismatched types: expected `proc()`, found `||` (expected proc, found closure)
}
