// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn main() {
    &panic!()
    //~^ ERROR mismatched types
    //~| ERROR mismatched types
    //~| NOTE: possibly missing `;` here?
    //~| NOTE: expected (), found reference
    //~| NOTE: expected type `()`
    //~| NOTE: expected type `()`
    //~| NOTE:    found type `&_`
    //~| NOTE:    found type `&_`
}
