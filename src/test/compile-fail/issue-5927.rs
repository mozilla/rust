// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.



// error-pattern:unresolved enum variant

fn main() {
    let z = match 3 {
        x(1) => x(1)
    };
    assert_eq!(z,3);
}
