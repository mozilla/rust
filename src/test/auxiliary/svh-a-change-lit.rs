// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! The `svh-a-*.rs` files are all deviations from the base file
//! svh-a-base.rs with some difference (usually in `fn foo`) that
//! should not affect the strict version hash (SVH) computation
//! (#14132).

#![crate_name = "a"]

macro_rules! three {
    () => { 3 }
}

pub trait U {}
pub trait V {}
impl U for () {}
impl V for () {}

static A_CONSTANT : int = 2;

pub fn foo<T:U>(_: int) -> int {
    0
}

pub fn an_unused_name() -> int {
    4
}
