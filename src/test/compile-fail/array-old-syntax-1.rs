// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Test that the old fixed length array syntax is a parsing error.

fn main() {
    let _x: [isize, ..3] = [0is, 1, 2]; //~ ERROR
}
