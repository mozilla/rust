// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(start)]
#![no_std] // makes debugging this test *a lot* easier (during resolve)

// Test to make sure that private items imported through globs remain private
// when  they're used.

mod bar {
    pub use self::glob::*;

    mod glob {
        fn gpriv() {}
    }
}

pub fn foo() {}

fn test1() {
    use bar::gpriv;
    //~^ ERROR unresolved import `bar::gpriv`. There is no `gpriv` in `bar`
    gpriv();
}

#[start] fn main(_: isize, _: *const *const u8) -> isize { 3 }
