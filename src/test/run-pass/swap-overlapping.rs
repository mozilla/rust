// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Issue #5041 - avoid overlapping memcpy when src and dest of a swap are the same

use std::ptr;

pub fn main() {
    let mut test = TestDescAndFn {
        desc: TestDesc {
            name: TestName::DynTestName("test".to_string()),
            should_fail: false
        },
        testfn: TestFn::DynTestFn(22),
    };
    do_swap(&mut test);
}

fn do_swap(test: &mut TestDescAndFn) {
    unsafe {
        ptr::swap(test, test);
    }
}

pub enum TestName {
    DynTestName(String)
}

pub enum TestFn {
    DynTestFn(int),
    DynBenchFn(int),
}

pub struct TestDesc {
    name: TestName,
    should_fail: bool
}

pub struct TestDescAndFn {
    desc: TestDesc,
    testfn: TestFn,
}
