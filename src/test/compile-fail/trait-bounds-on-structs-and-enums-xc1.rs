// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// aux-build:trait_bounds_on_structs_and_enums_xc.rs

extern crate trait_bounds_on_structs_and_enums_xc;

use trait_bounds_on_structs_and_enums_xc::{Bar, Foo, Trait};

fn main() {
    let foo = Foo {
    //~^ ERROR not implemented
        x: 3is
    };
    let bar: Bar<f64> = return;
    //~^ ERROR not implemented
    let _ = bar;
}

