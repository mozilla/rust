// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// ignore-tidy-linelength

pub struct Foo;

mod bar {
    use Foo;

    impl Foo { //~ERROR inherent implementations are only allowed on types defined in the current module
        fn baz(&self) {}
    }
}
fn main() {}

