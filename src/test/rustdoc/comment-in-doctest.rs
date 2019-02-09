// compile-flags:--test

// Comments, both doc comments and regular ones, used to trick rustdoc's doctest parser into
// thinking that everything after it was part of the regular program. Combined with the libsyntax
// parser loop failing to detect the manual main function, it would wrap everything in `fn main`,
// which would cause the doctest to fail as the "extern crate" declaration was no longer valid.
// Oddly enough, it would pass in 2018 if a crate was in the extern prelude. See
// issue #56727.

//! ```
//! // crate: proc-macro-test
//! //! this is a test
//!
//! // used to pull in proc-macro specific items
//! extern crate proc_macro;
//!
//! use proc_macro::TokenStream;
//!
//! # fn main() {}
//! ```
