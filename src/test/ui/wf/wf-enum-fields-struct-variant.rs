// Test that we check struct fields for WFedness.

#![feature(associated_type_defaults)]
#![feature(rustc_attrs)]
#![allow(dead_code)]

struct IsCopy<T:Copy> {
    value: T
}

enum AnotherEnum<A> {
    AnotherVariant {
        f: IsCopy<A> //~ ERROR E0277
    }
}

#[rustc_error]
fn main() { }
