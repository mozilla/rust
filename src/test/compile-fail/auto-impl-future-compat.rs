// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(optin_builtin_traits, immovable_types)]

use std::marker::Move;

trait Foo: ?Move {}
impl Foo for .. {}
//~^ ERROR The form `impl Foo for .. {}` will be removed, please use `auto trait Foo {}`
//~^^ WARN this was previously accepted by the compiler
