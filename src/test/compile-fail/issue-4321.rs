// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn main() {
    let tup = (true, true);
    println!("foo {:}", match tup { //~ ERROR non-exhaustive patterns: `(true, false)` not covered
        (false, false) => "foo",
        (false, true) => "bar",
        (true, true) => "baz"
    });
}
