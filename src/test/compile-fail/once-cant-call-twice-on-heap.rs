// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Testing guarantees provided by once functions.
// This program would segfault if it were legal.

#![feature(once_fns)]
use std::sync::Arc;

fn foo<F:FnOnce()>(blk: F) {
    blk();
    blk(); //~ ERROR use of moved value
}

fn main() {
    let x = Arc::new(true);
    foo(move|| {
        assert!(*x);
        drop(x);
    });
}
