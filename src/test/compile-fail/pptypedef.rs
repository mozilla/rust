// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn let_in<T, F>(x: T, f: F) where F: FnOnce(T) {}

fn main() {
    let_in(3us, |i| { assert!(i == 3is); });
    //~^ ERROR mismatched types
    //~| expected `usize`
    //~| found `isize`
    //~| expected usize
    //~| found isize

    let_in(3is, |i| { assert!(i == 3us); });
    //~^ ERROR mismatched types
    //~| expected `isize`
    //~| found `usize`
    //~| expected isize
    //~| found usize
}
