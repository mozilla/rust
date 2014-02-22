// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

pub fn main() {
    let x = ~[1, 2, 3];
    let mut y = 0;
    for i in x.iter() { info!("{:?}", *i); y += *i; }
    info!("{:?}", y);
    fail_unless_eq!(y, 6);
    let s = ~"hello there";
    let mut i: int = 0;
    for c in s.bytes() {
        if i == 0 { fail_unless!((c == 'h' as u8)); }
        if i == 1 { fail_unless!((c == 'e' as u8)); }
        if i == 2 { fail_unless!((c == 'l' as u8)); }
        if i == 3 { fail_unless!((c == 'l' as u8)); }
        if i == 4 { fail_unless!((c == 'o' as u8)); }
        // ...

        i += 1;
        info!("{:?}", i);
        info!("{:?}", c);
    }
    fail_unless_eq!(i, 11);
}
