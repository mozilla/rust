// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// aux-build:issue-7178.rs

extern crate "issue-7178" as cross_crate_self;

pub fn main() {
    let _ = cross_crate_self::Foo::new(&1i);
}
