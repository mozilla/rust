// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn foo<'r>(s: &'r uint) -> bool {
    match s {
        &3 => true,
        _ => false
    }
}

pub fn main() {
    assert!(foo(&3));
    assert!(!foo(&4));
}
