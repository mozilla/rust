// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![crate_type="lib"]
#![deny(warnings)]

pub use src::aliases::B;
pub use src::hidden_core::make;

mod src {
    pub mod aliases {
        use super::hidden_core::A;
        pub type B = A<f32>;
    }

    pub mod hidden_core {
        use super::aliases::B;

        pub struct A<T>;

        pub fn make() -> B { A }

        impl<T> A<T> {
            pub fn foo(&mut self) { println!("called foo"); }
        }
    }
}
