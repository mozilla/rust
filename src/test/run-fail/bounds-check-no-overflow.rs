// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// error-pattern:index out of bounds: the len is 3 but the index is

use std::uint::MAX_VALUE;
use std::mem::size_of;

fn main() {
    let xs = [1, 2, 3];
    xs[MAX_VALUE / size_of::<int>() + 1];
}
