// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// ignore-lexer-test FIXME #15877

trait Fooable {
    fn yes(self);
}

impl Fooable for uint {
    fn yes(self) {
        for _ in range(0, self) { println!("yes"); }
    }
}

pub fn main() {
    2.yes();
}
