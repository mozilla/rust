// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

mod m2 {
    pub enum Foo {
        A,
        B(int),
        C { a: int },
    }

    impl Foo {
        pub fn foo() {}
    }
}

mod m {
    pub use m2::Foo::*;
}

fn _f(f: m2::Foo) {
    use m2::Foo::*;

    match f {
        A | B(_) | C { .. } => {}
    }
}

fn _f2(f: m2::Foo) {
    match f {
        m::A | m::B(_) | m::C { .. } => {}
    }
}

pub fn main() {}
