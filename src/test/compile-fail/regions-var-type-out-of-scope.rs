// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn foo(cond: bool) {
    // Here we will infer a type that uses the
    // region of the if stmt then block:
    let mut x;

    if cond {
        x = &3is; //~ ERROR borrowed value does not live long enough
        assert_eq!(*x, 3is);
    }
}

fn main() {}
