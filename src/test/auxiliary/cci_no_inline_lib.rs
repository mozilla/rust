// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![crate_name="cci_no_inline_lib"]


// same as cci_iter_lib, more-or-less, but not marked inline
pub fn iter<F>(v: Vec<uint> , mut f: F) where F: FnMut(uint) {
    let mut i = 0u;
    let n = v.len();
    while i < n {
        f(v[i]);
        i += 1u;
    }
}
