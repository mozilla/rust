// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Regression test for issue #1448 and #1386

fn foo(a: usize) -> usize { a }

fn main() {
    println!("{}", foo(10is)); //~ ERROR mismatched types
}
