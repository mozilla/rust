// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(unboxed_closures)]

fn f<F:Nonexist(isize) -> isize>(x: F) {} //~ ERROR nonexistent trait `Nonexist`

type Typedef = isize;

fn g<F:Typedef(isize) -> isize>(x: F) {} //~ ERROR `Typedef` is not a trait

fn main() {}

