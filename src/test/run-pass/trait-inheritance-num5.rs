// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::cmp::PartialEq;
use std::num::NumCast;

pub trait NumExt: PartialEq + NumCast {}

impl NumExt for f32 {}
impl NumExt for int {}

fn num_eq_one<T:NumExt>() -> T {
    NumCast::from(1i).unwrap()
}

pub fn main() {
    num_eq_one::<int>(); // you need to actually use the function to trigger the ICE
}
