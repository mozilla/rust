// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

pub enum E {
    A,
    B,
}

pub mod b {
    pub fn key(e: ::E) -> &'static str {
        match e {
            A => "A",
//~^ WARN pattern binding `A` is named the same as one of the variants of the type `E`
            B => "B", //~ ERROR: unreachable pattern
//~^ WARN pattern binding `B` is named the same as one of the variants of the type `E`
        }
    }
}

fn main() {}
