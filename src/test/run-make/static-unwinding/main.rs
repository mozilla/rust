// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

extern crate lib;

use std::thread::Thread;

static mut statik: int = 0;

struct A;
impl Drop for A {
    fn drop(&mut self) {
        unsafe { statik = 1; }
    }
}

fn main() {
    Thread::scoped(move|| {
        let _a = A;
        lib::callback(|| panic!());
        1i
    }).join().err().unwrap();

    unsafe {
        assert!(lib::statik == 1);
        assert!(statik == 1);
    }
}
