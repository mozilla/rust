// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

struct StringBuffer {
    s: ~str
}

impl StringBuffer {
    pub fn append(&mut self, v: &str) {
        self.s.push_str(v);
    }
}

fn to_str(sb: StringBuffer) -> ~str {
    sb.s
}

pub fn main() {
    let mut sb = StringBuffer {s: ~""};
    sb.append("Hello, ");
    sb.append("World!");
    let str = to_str(sb);
    fail_unless_eq!(str, ~"Hello, World!");
}
