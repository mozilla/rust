// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Test that cross-borrowing (implicitly converting from `Box<T>` to `&T`) is
// forbidden when `T` is a trait.

#![feature(box_syntax)]

struct Foo;
trait Trait {}
impl Trait for Foo {}

pub fn main() {
    let x: Box<Trait> = box Foo;
    let _y: &Trait = x; //~  ERROR mismatched types
                        //~| expected `&Trait`
                        //~| found `Box<Trait>`
                        //~| expected &-ptr
                        //~| found box
}

