// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Test HRTB used with the `Fn` trait.

#![feature(unboxed_closures)]

fn foo<F:Fn(&int)>(f: F) {
    let x = 22;
    f(&x);
}

fn main() {
    foo(|&: x: &int| println!("{}", *x));
}
