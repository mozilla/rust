// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Check that static methods are not object-safe.

trait Tr {
    fn foo();
}

struct St;

impl Tr for St {
    fn foo() {}
}

fn main() {
    let _: &Tr = &St; //~ ERROR cannot convert to a trait object because trait `Tr` is not
}
