// Copyright 2012-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// ignore-test #9383

// shouldn't affect evaluation of $ex:
macro_rules! bad_macro {
    ($ex:expr) => ({(|_x| { $ex }) (9) })
}

fn takes_x(_x : isize) {
    assert_eq!(bad_macro!(_x),8);
}
fn main() {
    takes_x(8);
}
