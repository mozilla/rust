// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

mod a {
    pub struct Foo {
        x: isize
    }
}

fn main() {
    let s = a::Foo { x: 1 };    //~ ERROR field `x` of struct `a::Foo` is private
}
