// build-fail

// Regression test for #66975
#![warn(const_err)]
#![feature(const_panic)]


struct PrintName;

impl PrintName {
    const VOID: ! = panic!();
    //~^ ERROR evaluation of constant value failed
}

fn main() {
    let _ = PrintName::VOID;
    //~^ ERROR erroneous constant used
}
