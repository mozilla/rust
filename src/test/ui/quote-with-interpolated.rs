// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(quote)]
fn main() {
    macro_rules! foo {
        ($bar:expr)  => {
            quote_expr!(cx, $bar)
            //~^ ERROR quote! with interpolated token
            //~| ERROR failed to resolve: maybe a missing `extern crate syntax;`?
            //~| ERROR failed to resolve: maybe a missing `extern crate syntax;`?
            //~| ERROR cannot find value `cx` in this scope
            //~| ERROR cannot find function `new_parser_from_tts` in this scope
        }
    }
    foo!(bar);
}
