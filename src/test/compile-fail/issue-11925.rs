// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(box_syntax)]

fn main() {
    let r = {
        let x = box 42is;
        let f = move|:| &x; //~ ERROR: `x` does not live long enough
        f()
    };

    drop(r);
}
