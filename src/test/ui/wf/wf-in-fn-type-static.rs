// Check that we enforce WF conditions related to regions also for
// types in fns.

#![allow(dead_code)]
#![feature(rustc_attrs)]

struct MustBeCopy<T:Copy> {
    t: T
}

struct Foo<T> {
    // needs T: 'static
    x: fn() -> &'static T //~ ERROR E0310
}

struct Bar<T> {
    // needs T: Copy
    x: fn(&'static T) //~ ERROR E0310
}

#[rustc_error]
fn main() { }
