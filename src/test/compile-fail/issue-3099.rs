// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn a(x: String) -> String {
    format!("First function with {}", x)
}

fn a(x: String, y: String) -> String { //~ ERROR duplicate definition of value `a`
    format!("Second function with {} and {}", x, y)
}

fn main() {
    println!("Result: ");
}
