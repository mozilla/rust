// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

struct X<F> where F: FnOnce() + 'static + Send {
    field: F,
}

fn foo<F>(blk: F) -> X<F> where F: FnOnce() + 'static {
    //~^ ERROR the trait `core::marker::Send` is not implemented for the type
    return X { field: blk };
}

fn main() {
}
