// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn foo(s: &String) -> bool {
    match s.as_slice() {
        "kitty" => true,
        _ => false
    }
}

pub fn main() {
    assert!(foo(&"kitty".to_string()));
    assert!(!foo(&"gata".to_string()));
}
