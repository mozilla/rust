// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// error-pattern: unresolved

mod circ1 {
    pub use circ2::f2;
    pub fn f1() { println!("f1"); }
    pub fn common() -> usize { return 0us; }
}

mod circ2 {
    pub use circ1::f1;
    pub fn f2() { println!("f2"); }
    pub fn common() -> usize { return 1us; }
}

mod test {
    use circ1::*;

    fn test() { f1066(); }
}
