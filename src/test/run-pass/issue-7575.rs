// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

trait Foo {
    fn new() -> bool { false }
}

trait Bar {
    fn new(&self) -> bool { true }
}

impl Bar for int {}
impl Foo for int {}

fn main() {
    assert!(1i.new());
}
