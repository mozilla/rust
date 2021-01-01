// This test checks that a suggestion to add a `self: ` parameter name is provided
// to functions where this is applicable.

pub fn foo(Box<Self>) { }
//~^ ERROR expected one of

struct Bar;

impl Bar {
    fn bar(Box<Self>) { }
    //~^ ERROR expected one of
}

fn main() { }
