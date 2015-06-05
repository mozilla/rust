// Copyright 2012-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// compile-flags: -Z parse-only

fn f(a: isize, b: isize) : lt(a, b) { }
//~^ ERROR expected one of `->`, `where`, or `{`

fn lt(a: isize, b: isize) { }

fn main() { let a: isize = 10; let b: isize = 23; check (lt(a, b)); f(a, b); }
