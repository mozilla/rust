// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.



pub fn main() {
    if 1i == 2 {
        assert!((false));
    } else if 2i == 3 {
        assert!((false));
    } else if 3i == 4 { assert!((false)); } else { assert!((true)); }
    if 1i == 2 { assert!((false)); } else if 2i == 2 { assert!((true)); }
    if 1i == 2 {
        assert!((false));
    } else if 2i == 2 {
        if 1i == 1 {
            assert!((true));
        } else { if 2i == 1 { assert!((false)); } else { assert!((false)); } }
    }
    if 1i == 2 {
        assert!((false));
    } else { if 1i == 2 { assert!((false)); } else { assert!((true)); } }
}
