// Test that NLL generates proper error spans for trait HRTB errors
//
// compile-flags:-Zno-leak-check

#![feature(nll)]

trait Foo<'a> {}

fn make_foo<'a>() -> Box<dyn Foo<'a>> {
    panic!()
}

fn main() {
    let x: Box<dyn Foo<'static>> = make_foo();
    let y: Box<dyn for<'a> Foo<'a>> = x; //~ ERROR higher-ranked subtype error
}
