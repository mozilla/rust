// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(dead_assignment)]
#![allow(unused_variable)]

fn f() {
    let x = 10i; let mut y = 11i;
    if true { match x { _ => { y = x; } } } else { }
}

pub fn main() {
    let x = 10i;
    let mut y = 11i;
    if true { while false { y = x; } } else { }
}
