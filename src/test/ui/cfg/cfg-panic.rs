// build-pass
#![feature(cfg_panic)]

#[cfg(panic = "abort")]
pub fn bad() -> i32 { }

#[cfg(not(panic = "unwind"))]
pub fn bad() -> i32 { }

#[cfg(panic = "some_imaginary_future_panic_handler")]
pub fn bad() -> i32 { }

#[cfg(panic = "unwind")]
pub fn main() { }
