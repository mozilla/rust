// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Test how overloaded deref interacts with borrows when DerefMut
// is implemented.

use std::ops::{Deref, DerefMut};

struct Own<T> {
    value: *mut T
}

impl<T> Deref<T> for Own<T> {
    fn deref<'a>(&'a self) -> &'a T {
        unsafe { &*self.value }
    }
}

impl<T> DerefMut<T> for Own<T> {
    fn deref_mut<'a>(&'a mut self) -> &'a mut T {
        unsafe { &mut *self.value }
    }
}

struct Point {
    x: int,
    y: int
}

impl Point {
    fn get(&self) -> (int, int) {
        (self.x, self.y)
    }

    fn set(&mut self, x: int, y: int) {
        self.x = x;
        self.y = y;
    }

    fn x_ref<'a>(&'a self) -> &'a int {
        &self.x
    }

    fn y_mut<'a>(&'a mut self) -> &'a mut int {
        &mut self.y
    }
}

fn deref_imm_field(x: Own<Point>) {
    let _i = &x.y;
}

fn deref_mut_field1(x: Own<Point>) {
    let _i = &mut x.y; //~ ERROR cannot borrow
}

fn deref_mut_field2(mut x: Own<Point>) {
    let _i = &mut x.y;
}

fn deref_extend_field<'a>(x: &'a Own<Point>) -> &'a int {
    &x.y
}

fn deref_extend_mut_field1<'a>(x: &'a Own<Point>) -> &'a mut int {
    &mut x.y //~ ERROR cannot borrow
}

fn deref_extend_mut_field2<'a>(x: &'a mut Own<Point>) -> &'a mut int {
    &mut x.y
}

fn deref_extend_mut_field3<'a>(x: &'a mut Own<Point>) {
    // Hmm, this is unfortunate, because with ~ it would work,
    // but it's presently the expected outcome. See `deref_extend_mut_field4`
    // for the workaround.

    let _x = &mut x.x;
    let _y = &mut x.y; //~ ERROR cannot borrow
}

fn deref_extend_mut_field4<'a>(x: &'a mut Own<Point>) {
    let p = &mut **x;
    let _x = &mut p.x;
    let _y = &mut p.y;
}

fn assign_field1<'a>(x: Own<Point>) {
    x.y = 3; //~ ERROR cannot borrow
}

fn assign_field2<'a>(x: &'a Own<Point>) {
    x.y = 3; //~ ERROR cannot assign
}

fn assign_field3<'a>(x: &'a mut Own<Point>) {
    x.y = 3;
}

fn assign_field4<'a>(x: &'a mut Own<Point>) {
    let _p: &mut Point = &mut **x;
    x.y = 3; //~ ERROR cannot borrow
}

// FIXME(eddyb) #12825 This shouldn't attempt to call deref_mut.
/*
fn deref_imm_method(x: Own<Point>) {
    let _i = x.get();
}
*/

fn deref_mut_method1(x: Own<Point>) {
    x.set(0, 0); //~ ERROR cannot borrow
}

fn deref_mut_method2(mut x: Own<Point>) {
    x.set(0, 0);
}

fn deref_extend_method<'a>(x: &'a Own<Point>) -> &'a int {
    x.x_ref()
}

fn deref_extend_mut_method1<'a>(x: &'a Own<Point>) -> &'a mut int {
    x.y_mut() //~ ERROR cannot borrow
}

fn deref_extend_mut_method2<'a>(x: &'a mut Own<Point>) -> &'a mut int {
    x.y_mut()
}

fn assign_method1<'a>(x: Own<Point>) {
    *x.y_mut() = 3; //~ ERROR cannot borrow
}

fn assign_method2<'a>(x: &'a Own<Point>) {
    *x.y_mut() = 3; //~ ERROR cannot borrow
}

fn assign_method3<'a>(x: &'a mut Own<Point>) {
    *x.y_mut() = 3;
}

pub fn main() {}
