// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// aux-build:static_fn_trait_xc_aux.rs

extern crate "static_fn_trait_xc_aux" as mycore;

use mycore::num;

pub fn main() {
    let _1: f64 = num::Num2::from_int2(1i);
}
