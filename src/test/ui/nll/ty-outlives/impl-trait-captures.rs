// compile-flags:-Zborrowck=mir -Zverbose

#![allow(warnings)]

trait Foo<'a> {
}

impl<'a, T> Foo<'a> for T { }

fn foo<'a, T>(x: &T) -> impl Foo<'a> {
    x
        //~^ WARNING not reporting region error due to nll
        //~| ERROR explicit lifetime required in the type of `x` [E0621]
}

fn main() {}
