// compile-flags: --emit=mir,link

// Variant of panic-assoc-never-type.rs.
// Ensure that mir opts don't hide errors due to the usage of erroneous constants
// in unused code.

#![warn(const_err)]
#![feature(const_panic)]
#![feature(never_type)]

struct PrintName;

impl PrintName {
    const VOID: ! = panic!();
    //~^ WARN any use of this value will cause an error
    //~| WARN this was previously accepted by the compiler but is being phased out
}

fn foo() {
    let _ = PrintName::VOID;
    //~^ ERROR erroneous constant used
}

fn main() {}
