#![feature(nll)]

// Test that we enforce user-provided type annotations on closures.

fn foo<'a>() {
    // Here `x` is free in the closure sig:
    |x: &'a i32| -> &'static i32 {
        return x; //~ ERROR unsatisfied lifetime constraints
    };
}

fn foo1() {
    // Here `x` is bound in the closure sig:
    |x: &i32| -> &'static i32 {
        return x; //~ ERROR unsatisfied lifetime constraints
    };
}

fn bar<'a>() {
    // Here `x` is free in the closure sig:
    |x: &'a i32, b: fn(&'static i32)| {
        b(x); //~ ERROR unsatisfied lifetime constraints
    };
}

fn bar1() {
    // Here `x` is bound in the closure sig:
    |x: &i32, b: fn(&'static i32)| {
        b(x); //~ ERROR borrowed data escapes outside of closure
    };
}

fn main() { }
