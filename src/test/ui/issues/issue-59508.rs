// run-rustfix

#![allow(dead_code)]

// This test checks that generic parameter re-ordering diagnostic suggestions contain bounds.

struct A;

impl A {
    pub fn do_things<T, 'a, 'b: 'a>() {
    //~^ ERROR incorrect parameter order
        println!("panic");
    }
}

fn main() {}
