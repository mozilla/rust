// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn siphash(k0 : u64) {

    struct siphash {
        v0: u64,
    }

    impl siphash {
        pub fn reset(&mut self) {
           self.v0 = k0 ^ 0x736f6d6570736575; //~ ERROR can't capture dynamic environment
           //~^ ERROR unresolved name `k0`
        }
    }
}

fn main() {}
