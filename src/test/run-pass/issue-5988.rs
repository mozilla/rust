// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::io;
trait B {
    fn f(&self);
}

trait T : B {
}

struct A;

impl<U: T> B for U {
    fn f(&self) { io::println("Hey, I'm a T!"); }
}

impl T for A {
}

fn main() {
    let a = A;
    let br = &a as &B;
    br.f();
}
