// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![deny(warnings)]
#![allow(dead_code)]

fn main() {
    while true {} //~ ERROR: infinite
}

#[allow(warnings)]
fn foo() {
    while true {}
}

#[warn(warnings)]
fn bar() {
    while true {} //~ WARNING: infinite
}

#[forbid(warnings)]
fn baz() {
    while true {} //~ ERROR: warnings
}
