// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

struct Baz { q: Option<Foo> }
//~^ ERROR illegal recursive struct type; wrap the inner value in a box to make it representable

struct Foo { q: Option<Baz> }
//~^ ERROR illegal recursive struct type; wrap the inner value in a box to make it representable

impl Foo { fn bar(&self) {} }

fn main() {}
