// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

mod x {
    pub fn g() -> uint {14}
}

pub fn main(){
    // should *not* shadow the module x:
    let x = 9i;
    // use it to avoid warnings:
    x+3;
    assert_eq!(x::g(),14);
}
