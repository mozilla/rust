// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.




fn f() -> int {
    if true {
        let _s: String = "should not leak".to_string();
        return 1;
    }
    return 0;
}

pub fn main() { f(); }
