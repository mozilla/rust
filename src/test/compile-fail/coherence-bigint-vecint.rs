// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// aux-build:coherence-lib.rs

extern crate "coherence-lib" as lib;
use lib::Remote1;

pub struct BigInt;

impl Remote1<BigInt> for Vec<isize> { } //~ ERROR E0117

fn main() { }
