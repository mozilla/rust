// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! # The Rust Core Library
//!
//! The Rust Core Library is the dependency-free foundation of [The
//! Rust Standard Library](../std/index.html). It is the portable glue
//! between the language and its libraries, defining the intrinsic and
//! primitive building blocks of all Rust code. It links to no
//! upstream libraries, no system libraries, and no libc.
//!
//! The core library is *minimal*: it isn't even aware of heap allocation,
//! nor does it provide concurrency or I/O. These things require
//! platform integration, and this library is platform-agnostic.
//!
//! *It is not recommended to use the core library*. The stable
//! functionality of libcore is reexported from the
//! [standard library](../std/index.html). The composition of this library is
//! subject to change over time; only the interface exposed through libstd is
//! intended to be stable.
//!
//! # How to use the core library
//!
// FIXME: Fill me in with more detail when the interface settles
//! This library is built on the assumption of a few existing symbols:
//!
//! * `memcpy`, `memcmp`, `memset` - These are core memory routines which are
//!   often generated by LLVM. Additionally, this library can make explicit
//!   calls to these functions. Their signatures are the same as found in C.
//!   These functions are often provided by the system libc, but can also be
//!   provided by `librlibc` which is distributed with the standard rust
//!   distribution.
//!
//! * `rust_begin_unwind` - This function takes three arguments, a
//!   `&fmt::Arguments`, a `&str`, and a `uint`. These three arguments dictate
//!   the panic message, the file at which panic was invoked, and the line.
//!   It is up to consumers of this core library to define this panic
//!   function; it is only required to never return.

// Since libcore defines many fundamental lang items, all tests live in a
// separate crate, libcoretest, to avoid bizarre issues.

#![crate_name = "core"]
#![experimental]
#![crate_type = "rlib"]
#![doc(html_logo_url = "http://www.rust-lang.org/logos/rust-logo-128x128-blk-v2.png",
       html_favicon_url = "http://www.rust-lang.org/favicon.ico",
       html_root_url = "http://doc.rust-lang.org/nightly/",
       html_playground_url = "http://play.rust-lang.org/")]

#![no_std]
#![allow(unknown_features)]
#![feature(globs, intrinsics, lang_items, macro_rules, phase)]
#![feature(simd, unsafe_destructor, slicing_syntax)]
#![feature(default_type_params)]
#![deny(missing_docs)]

mod macros;

#[path = "num/float_macros.rs"] mod float_macros;
#[path = "num/int_macros.rs"]   mod int_macros;
#[path = "num/uint_macros.rs"]  mod uint_macros;

#[path = "num/int.rs"]  pub mod int;
#[path = "num/i8.rs"]   pub mod i8;
#[path = "num/i16.rs"]  pub mod i16;
#[path = "num/i32.rs"]  pub mod i32;
#[path = "num/i64.rs"]  pub mod i64;

#[path = "num/uint.rs"] pub mod uint;
#[path = "num/u8.rs"]   pub mod u8;
#[path = "num/u16.rs"]  pub mod u16;
#[path = "num/u32.rs"]  pub mod u32;
#[path = "num/u64.rs"]  pub mod u64;

#[path = "num/f32.rs"]   pub mod f32;
#[path = "num/f64.rs"]   pub mod f64;

pub mod num;

/* The libcore prelude, not as all-encompassing as the libstd prelude */

pub mod prelude;

/* Core modules for ownership management */

pub mod intrinsics;
pub mod mem;
pub mod ptr;

/* Core language traits */

pub mod kinds;
pub mod ops;
pub mod cmp;
pub mod clone;
pub mod default;

/* Core types and methods on primitives */

pub mod any;
pub mod atomic;
pub mod bool;
pub mod borrow;
pub mod cell;
pub mod char;
pub mod panicking;
pub mod finally;
pub mod iter;
pub mod option;
pub mod raw;
pub mod result;
pub mod simd;
pub mod slice;
pub mod str;
pub mod tuple;
// FIXME #15320: primitive documentation needs top-level modules, this
// should be `core::tuple::unit`.
#[path = "tuple/unit.rs"]
pub mod unit;
pub mod fmt;

// note: does not need to be public
mod array;

#[doc(hidden)]
mod core {
    pub use panicking;
}

#[doc(hidden)]
mod std {
    pub use clone;
    pub use cmp;
    pub use kinds;
    pub use option;
    pub use fmt;
}
