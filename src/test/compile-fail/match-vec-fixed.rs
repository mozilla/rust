// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn a() {
    let v = [1is, 2, 3];
    match v {
        [_, _, _] => {}
        [_, _, _] => {} //~ ERROR unreachable pattern
    }
    match v {
        [_, 1, _] => {}
        [_, 1, _] => {} //~ ERROR unreachable pattern
        _ => {}
    }
}

fn main() {
    a();
}
