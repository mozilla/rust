// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// aux-build:default_type_params_xc.rs

extern crate default_type_params_xc;

struct Vec<T, A = default_type_params_xc::Heap>;

struct Foo;

fn main() {
    let _a = Vec::<int>;
    let _b = Vec::<int, default_type_params_xc::FakeHeap>;
    let _c = default_type_params_xc::FakeVec::<int>;
    let _d = default_type_params_xc::FakeVec::<int, Foo>;
}
