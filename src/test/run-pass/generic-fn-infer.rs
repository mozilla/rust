// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.





// Issue #45: infer type parameters in function applications
fn id<T>(x: T) -> T { return x; }

pub fn main() { let x: int = 42; let y: int = id(x); assert!((x == y)); }
