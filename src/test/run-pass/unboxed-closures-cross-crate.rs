// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Test that unboxed closures work with cross-crate inlining
// Acts as a regression test for #16790, #18378 and #18543

// aux-build:unboxed-closures-cross-crate.rs
extern crate "unboxed-closures-cross-crate" as ubcc;

fn main() {
    assert_eq!(ubcc::has_closures(), 2u);
    assert_eq!(ubcc::has_generic_closures(2u, 3u), 5u);
}
