// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// aux-build:issue-7899.rs

extern crate "issue-7899" as testcrate;

fn main() {
    let f = testcrate::V2(1.0f32, 2.0f32);
}
