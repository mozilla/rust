// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(overloaded_calls, unboxed_closures)]

// Make sure we don't ICE when making an overloaded call with the
// wrong arity.

fn _foo<F: Fn()> (f: F) {
    |&: t| f(t); //~ ERROR E0057
}

fn main() {}
