// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

struct BuildData {
    foo: isize,
}

fn main() {
    let foo = BuildData {
        foo: 0,
        bar: 0 //~ ERROR structure `BuildData` has no field named `bar`
    };
}
