// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// error-pattern: type `&Foo` does not implement any method in scope named `foo`

trait Foo {
    fn foo(self: Box<Self>);
}

impl Foo for isize {
    fn foo(self: Box<isize>) { }
}

fn main() {
    (&5 as &Foo).foo();
}
