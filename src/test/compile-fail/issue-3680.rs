// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn main() {
    match None {
        Err(_) => ()
        //~^ ERROR mismatched types
        //~| expected `core::option::Option<_>`
        //~| found `core::result::Result<_, _>`
        //~| expected enum `core::option::Option`
        //~| found enum `core::result::Result`
    }
}
