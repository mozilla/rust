// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// aux-build:cci_iter_lib.rs

extern crate cci_iter_lib;

pub fn main() {
    //let bt0 = sys::rusti::frame_address(1u32);
    //println!("%?", bt0);
    cci_iter_lib::iter(&[1i, 2, 3], |i| {
        println!("{}", *i);
        //assert!(bt0 == sys::rusti::frame_address(2u32));
    })
}
