// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(unknown_features)]
#![feature(box_syntax)]

struct Foo(Box<isize>, isize);

struct Bar(isize, isize);

fn main() {
    let x = (box 1is, 2is);
    let r = &x.0;
    let y = x; //~ ERROR cannot move out of `x` because it is borrowed

    let mut x = (1is, 2is);
    let a = &x.0;
    let b = &mut x.0; //~ ERROR cannot borrow `x.0` as mutable because it is also borrowed as

    let mut x = (1is, 2is);
    let a = &mut x.0;
    let b = &mut x.0; //~ ERROR cannot borrow `x.0` as mutable more than once at a time


    let x = Foo(box 1is, 2is);
    let r = &x.0;
    let y = x; //~ ERROR cannot move out of `x` because it is borrowed

    let mut x = Bar(1is, 2is);
    let a = &x.0;
    let b = &mut x.0; //~ ERROR cannot borrow `x.0` as mutable because it is also borrowed as

    let mut x = Bar(1is, 2is);
    let a = &mut x.0;
    let b = &mut x.0; //~ ERROR cannot borrow `x.0` as mutable more than once at a time
}
