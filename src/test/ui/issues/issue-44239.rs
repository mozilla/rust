// run-rustfix
#![allow(dead_code, non_upper_case_globals)]
fn main() {
    let n: usize = 0;

    struct Foo;
    impl Foo {
        const N: usize = n;
        //~^ ERROR attempt to use a non-constant value
    }
}
