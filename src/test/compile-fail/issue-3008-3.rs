// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

enum E1 { V1(E2<E1>), }
enum E2<T> { V2(E2<E1>), }
//~^ ERROR illegal recursive enum type; wrap the inner value in a box to make it representable

impl E1 { fn foo(&self) {} }

fn main() {
}
