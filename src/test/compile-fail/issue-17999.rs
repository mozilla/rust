// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![deny(unused_variables)]
#![allow(unstable)]

fn main() {
    for _ in range(1is, 101) {
        let x = (); //~ ERROR: unused variable: `x`
        match () {
            a => {} //~ ERROR: unused variable: `a`
        }
    }
}
