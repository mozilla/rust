// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.



// a bug was causing this to complain about leaked memory on exit

enum t { foo(int, uint), bar(int, Option<int>), }

fn nested(o: t) {
    match o {
        t::bar(_i, Some::<int>(_)) => { println!("wrong pattern matched"); panic!(); }
        _ => { println!("succeeded"); }
    }
}

pub fn main() { nested(t::bar(1, None::<int>)); }
