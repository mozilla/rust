// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.


trait Foo {
    fn f(&self) {
        println!("Hello!");
        self.g();
    }
    fn g(&self);
}

struct A {
    x: int
}

impl Foo for A {
    fn g(&self) {
        println!("Goodbye!");
    }
}

pub fn main() {
    let a = A { x: 1 };
    a.f();
}
