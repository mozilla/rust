// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// aux-build:cci_impl_lib.rs

extern crate cci_impl_lib;
use cci_impl_lib::uint_helpers;

pub fn main() {
    //let bt0 = sys::frame_address();
    //println!("%?", bt0);

    3u.to(10u, |i| {
        println!("{}", i);

        //let bt1 = sys::frame_address();
        //println!("%?", bt1);
        //assert!(bt0 == bt1);
    })
}
