// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

enum Foo {
    Bar {
        a: int,
        b: int
    },
    Baz {
        c: f64,
        d: f64
    }
}

pub fn main() {
    let _x = Foo::Bar { a: 2, b: 3 };
}
