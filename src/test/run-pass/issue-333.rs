// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn quux<T>(x: T) -> T { let f = id::<T>; return f(x); }

fn id<T>(x: T) -> T { return x; }

pub fn main() { assert!((quux(10i) == 10i)); }
