#![feature(rustc_attrs)]

pub trait Foo<P> {}

pub trait Bar {
    type Output: 'static;
}

impl Foo<i32> for i32 { }

impl<A:Bar> Foo<A::Output> for A { }

impl Bar for i32 {
    type Output = u32;
}

#[rustc_error]
fn main() {} //~ ERROR compilation successful
