// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Test that region inference correctly links up the regions when a
// `ref` borrow occurs inside a fn argument.

#![allow(dead_code)]

fn with<'a, F>(_: F) where F: FnOnce(&'a Vec<int>) -> &'a Vec<int> { }

fn foo() {
    with(|&ref ints| ints);
}

fn main() { }
