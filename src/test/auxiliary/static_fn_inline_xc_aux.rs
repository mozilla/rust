// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.


pub mod num {
    pub trait Num2 {
        fn from_int2(n: int) -> Self;
    }
}

pub mod f64 {
    impl ::num::Num2 for f64 {
        #[inline]
        fn from_int2(n: int) -> f64 { return n as f64;  }
    }
}
