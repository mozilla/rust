// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

struct T;

mod t1 {
    type Foo = ::T;
    mod Foo {} //~ ERROR: duplicate definition of type or module `Foo`
}

mod t2 {
    type Foo = ::T;
    struct Foo; //~ ERROR: duplicate definition of type or module `Foo`
}

mod t3 {
    type Foo = ::T;
    enum Foo {} //~ ERROR: duplicate definition of type or module `Foo`
}

mod t4 {
    type Foo = ::T;
    fn Foo() {} // ok
}

mod t5 {
    type Bar<T> = T;
    mod Bar {} //~ ERROR: duplicate definition of type or module `Bar`
}

mod t6 {
    type Foo = ::T;
    impl Foo {} // ok
}


fn main() {}
