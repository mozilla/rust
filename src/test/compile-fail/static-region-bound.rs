// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(box_syntax)]

fn f<T:'static>(_: T) {}

fn main() {
    let x = box 3is;
    f(x);
    let x = &3is; //~ ERROR borrowed value does not live long enough
    f(x);
}
