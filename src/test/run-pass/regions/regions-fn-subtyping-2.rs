// run-pass
#![allow(dead_code)]
// Issue #2263.

// Here, `f` is a function that takes a pointer `x` and a function
// `g`, where `g` requires its argument `y` to be in the same region
// that `x` is in.
// pretty-expanded FIXME(#23616):

fn has_same_region(f: Box<for<'a> FnMut(&'a isize, Box<FnMut(&'a isize)>)>) {
    // `f` should be the type that `wants_same_region` wants, but
    // right now the compiler complains that it isn't.
    wants_same_region(f);
}

fn wants_same_region(_f: Box<for<'b> FnMut(&'b isize, Box<FnMut(&'b isize)>)>) {
}

pub fn main() {
}
