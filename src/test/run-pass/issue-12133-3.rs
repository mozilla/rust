// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// aux-build:issue-12133-rlib.rs
// aux-build:issue-12133-dylib.rs
// aux-build:issue-12133-dylib2.rs

extern crate "issue-12133-dylib2" as other;

fn main() {}
