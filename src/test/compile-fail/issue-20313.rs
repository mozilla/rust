// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

extern {
    #[link_name = "llvm.sqrt.f32"]
    fn sqrt(x: f32) -> f32; //~ ERROR linking to LLVM intrinsics is experimental
}

fn main(){
}
