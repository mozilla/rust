// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

extern crate cci_lib;
use cci_lib::{cci_fn};

fn call1() -> uint {
    cci_fn()
}

mod a {
    use cci_lib::cci_fn;
    pub fn call2() -> uint {
        cci_fn()
    }
}

mod b {
    pub fn call3() -> uint {
        0
    }
}

fn main() {
    call1();
    a::call2();
    b::call3();
}
