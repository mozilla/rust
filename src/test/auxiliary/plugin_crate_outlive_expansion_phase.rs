// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// force-host

#![feature(plugin_registrar)]

extern crate rustc;

use std::any::Any;
use rustc::plugin::Registry;

struct Foo {
    foo: int
}

impl Drop for Foo {
    fn drop(&mut self) {}
}

#[plugin_registrar]
pub fn registrar(_: &mut Registry) {
    local_data_key!(foo: Box<Any+Send+'static>);
    foo.replace(Some(box Foo { foo: 10 } as Box<Any+Send+'static>));
}

