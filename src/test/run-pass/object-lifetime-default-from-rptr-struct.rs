// Test that the lifetime from the enclosing `&` is "inherited"
// through the `MyBox` struct.

// pretty-expanded FIXME #23616

#![allow(dead_code)]

trait Test {
    fn foo(&self) { }
}

struct SomeStruct<'a> {
    t: &'a MyBox<Test>,
    u: &'a MyBox<Test+'a>,
}

struct MyBox<T:?Sized> {
    b: Box<T>
}

fn a<'a>(t: &'a MyBox<Test>, mut ss: SomeStruct<'a>) {
    ss.t = t;
}

fn b<'a>(t: &'a MyBox<Test>, mut ss: SomeStruct<'a>) {
    ss.u = t;
}

// see also compile-fail/object-lifetime-default-from-rptr-box-error.rs

fn d<'a>(t: &'a MyBox<Test+'a>, mut ss: SomeStruct<'a>) {
    ss.u = t;
}

fn main() {
}
