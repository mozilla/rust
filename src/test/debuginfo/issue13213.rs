// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// ignore-android: FIXME(#10381)
// min-lldb-version: 310

// aux-build:issue13213aux.rs
extern crate issue13213aux;

// compile-flags:-g

// This tests make sure that we get no linker error when using a completely inlined static. Some
// statics that are marked with AvailableExternallyLinkage in the importing crate, may actually not
// be available because they have been optimized out from the exporting crate.
fn main() {
    let b: issue13213aux::S = issue13213aux::A;
    ::std::io::println("Nothing to do here...");
}
