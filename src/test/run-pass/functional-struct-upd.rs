// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[derive(Show)]
struct Foo {
    x: int,
    y: int
}

pub fn main() {
    let a = Foo { x: 1, y: 2 };
    let c = Foo { x: 4, .. a};
    println!("{:?}", c);
}
