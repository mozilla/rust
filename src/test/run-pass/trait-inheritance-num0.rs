// xfail-fast

// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Extending Num and using inherited static methods

use core::num::NumCast::from;

trait Num {
    static fn from_int(i: int) -> Self;
    fn gt(&self, other: &Self) -> bool;
}

pub trait NumExt: Num + NumCast { }

fn greater_than_one<T:NumExt>(n: &T) -> bool {
    n.gt(&from(1))
}

pub fn main() {}
