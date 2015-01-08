// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Test which of the builtin types are considered sendable. The tests
// in this file all test region bound and lifetime violations that are
// detected during type check.

extern crate core;
use core::ptr::Unique;

fn assert_send<T:Send>() { }
trait Dummy:Send { }

// lifetime pointers with 'static lifetime are ok

fn static_lifime_ok<'a,T,U:Send>(_: &'a isize) {
    assert_send::<&'static isize>();
    assert_send::<&'static str>();
    assert_send::<&'static [isize]>();

    // whether or not they are mutable
    assert_send::<&'static mut isize>();
}

// otherwise lifetime pointers are not ok

fn param_not_ok<'a>(x: &'a isize) {
    assert_send::<&'a isize>(); //~ ERROR declared lifetime bound not satisfied
}

fn param_not_ok1<'a>(_: &'a isize) {
    assert_send::<&'a str>(); //~ ERROR declared lifetime bound not satisfied
}

fn param_not_ok2<'a>(_: &'a isize) {
    assert_send::<&'a [isize]>(); //~ ERROR declared lifetime bound not satisfied
}

// boxes are ok

fn box_ok() {
    assert_send::<Box<isize>>();
    assert_send::<String>();
    assert_send::<Vec<isize>>();
}

// but not if they own a bad thing

fn box_with_region_not_ok<'a>() {
    assert_send::<Box<&'a isize>>(); //~ ERROR declared lifetime bound not satisfied
}

// objects with insufficient bounds no ok

fn object_with_random_bound_not_ok<'a>() {
    assert_send::<&'a (Dummy+'a)>();
    //~^ ERROR reference has a longer lifetime
}

fn object_with_send_bound_not_ok<'a>() {
    assert_send::<&'a (Dummy+Send)>();
    //~^ ERROR declared lifetime bound not satisfied
}

// unsafe pointers are ok unless they point at unsendable things

struct UniqueUnsafePtr(Unique<*const isize>);

unsafe impl Send for UniqueUnsafePtr {}

fn unsafe_ok1<'a>(_: &'a isize) {
    assert_send::<UniqueUnsafePtr>();
}

fn main() {
}
