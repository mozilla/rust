// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// error-pattern: not all control paths return a value

fn god_exists(a: isize) -> bool { return god_exists(a); }

fn f(a: isize) -> isize { if god_exists(a) { return 5; }; }

fn main() { f(12); }
