// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Ensure that we get an error and not an ICE for this problematic case.
struct Foo<T = Option<U>, U = bool>;
//~^ ERROR type parameters with a default cannot use forward declared identifiers
fn main() {
    let x: Foo;
}
