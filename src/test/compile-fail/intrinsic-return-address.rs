// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(warnings)]
#![feature(intrinsics)]

extern "rust-intrinsic" {
    fn return_address() -> *const u8;
}

unsafe fn f() {
    let _ = return_address();
    //~^ ERROR invalid use of `return_address` intrinsic: function does not use out pointer
}

unsafe fn g() -> isize {
    let _ = return_address();
    //~^ ERROR invalid use of `return_address` intrinsic: function does not use out pointer
    0
}

fn main() {}


