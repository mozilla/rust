// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

enum X {
    Foo(uint),
    Bar(bool)
}

fn main() {
    let x = match X::Foo(42) {
        X::Foo(..) => 1i,
        _ if true => 0,
        X::Bar(..) => panic!("Oh dear")
    };
    assert_eq!(x, 1);

    let x = match X::Foo(42) {
        _ if true => 0i,
        X::Foo(..) => 1,
        X::Bar(..) => panic!("Oh dear")
    };
    assert_eq!(x, 0);
}
