// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

mod foo {
    pub use self::bar::X;
    use self::bar::X;
    //~^ ERROR a value named `X` has already been imported in this module
    //~| ERROR a type named `X` has already been imported in this module

    mod bar {
        pub struct X;
    }
}

fn main() {
    let _ = foo::X;
}
