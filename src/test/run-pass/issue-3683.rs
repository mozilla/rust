// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// ignore-lexer-test FIXME #15877


trait Foo {
    fn a(&self) -> int;
    fn b(&self) -> int {
        self.a() + 2
    }
}

impl Foo for int {
    fn a(&self) -> int {
        3
    }
}

pub fn main() {
    assert_eq!(3.b(), 5);
}
