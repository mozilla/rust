// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.



enum foo { large, small, }

impl Eq for foo {
    fn eq(&self, other: &foo) -> bool {
        ((*self) as uint) == ((*other) as uint)
    }
    fn ne(&self, other: &foo) -> bool { !(*self).eq(other) }
}

pub fn main() {
    let a = (1, 2, 3);
    let b = (1, 2, 3);
    fail_unless_eq!(a, b);
    fail_unless!((a != (1, 2, 4)));
    fail_unless!((a < (1, 2, 4)));
    fail_unless!((a <= (1, 2, 4)));
    fail_unless!(((1, 2, 4) > a));
    fail_unless!(((1, 2, 4) >= a));
    let x = large;
    let y = small;
    fail_unless!((x != y));
    fail_unless_eq!(x, large);
    fail_unless!((x != small));
}
