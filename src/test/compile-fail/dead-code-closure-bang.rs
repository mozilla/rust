// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// ignore-test FIXME(#20574)

#![deny(unreachable_code)]

fn main() {
    let x = |:| panic!();
    x();
    std::io::println("Foo bar"); //~ ERROR: unreachable statement
}
