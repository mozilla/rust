// Test that a nominal type (like `Foo<'a>`) outlives `'b` if its
// arguments (like `'a`) outlive `'b`.
//
// Rule OutlivesNominalType from RFC 1214.

#![feature(rustc_attrs)]
#![allow(dead_code)]

mod variant_struct_type {
    struct Foo<T> {
        x: T
    }
    enum Bar<'a,'b> {
        F(&'a Foo<&'b i32>) //~ ERROR reference has a longer lifetime
    }
}

#[rustc_error]
fn main() { }
