//! A test for calling `C-unwind` functions across foreign function boundaries.
//!
//! This test triggers a panic in a Rust library that our foreign function invokes. This shows
//! that we can unwind through the C code in that library, and catch the underlying panic.
#![feature(c_unwind)]

use std::panic::{catch_unwind, AssertUnwindSafe};

fn main() {
    // Call `add_small_numbers`, passing arguments that will trigger a panic, and catch it.
    let caught_unwind = catch_unwind(AssertUnwindSafe(|| {
        let (a, b) = (10, 1);
        let _c = unsafe { add_small_numbers(a, b) };
        unreachable!("should have unwound instead of returned");
    }));

    // Assert that we did indeed panic, then unwrap and downcast the panic into the sum.
    assert!(caught_unwind.is_err());
    let panic_obj = caught_unwind.unwrap_err();
    let panic_u32 = *panic_obj.downcast_ref::<u32>().unwrap();
    assert_eq!(panic_u32, 11);
}

#[link(name = "add")]
extern "C-unwind" {
    /// An external function, defined in C.
    ///
    /// Returns the sum of two numbers, or panics if the sum is greater than 10.
    fn add_small_numbers(a: u32, b: u32) -> u32;
}
