// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Test that astconv doesn't forget about mutability of &mut str

fn main() {
    fn foo<T: ?Sized>(_: &mut T) {}
    let _f: fn(&mut str) = foo;
}
