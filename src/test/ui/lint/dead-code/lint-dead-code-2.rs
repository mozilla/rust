#![allow(unused_variables)]
#![deny(dead_code)]
#![feature(start)]

struct Foo;

trait Bar {
    fn bar1(&self);
    fn bar2(&self) {
        self.bar1();
    }
}

impl Bar for Foo {
    fn bar1(&self) {
        live_fn();
    }
}

fn live_fn() {}

fn dead_fn() {} //~ ERROR: function is never used

fn used_fn() {}

#[start]
fn start(_: isize, _: *const *const u8) -> isize {
    used_fn();
    let foo = Foo;
    foo.bar2();
    0
}

// this is not main
fn main() { //~ ERROR: function is never used
    dead_fn();
}
