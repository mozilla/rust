// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(box_syntax)]

struct Struct {
    person: &'static str
}

trait Trait<T> {
    fn f(&self, x: T);
}

impl Trait<&'static str> for Struct {
    fn f(&self, x: &'static str) {
        println!("Hello, {}!", x);
    }
}

fn main() {
    let s: Box<Trait<isize>> = box Struct { person: "Fred" };
    //~^ ERROR the trait `Trait<isize>` is not implemented for the type `Struct`
    s.f(1);
}
