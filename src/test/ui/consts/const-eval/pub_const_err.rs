// check-pass
#![warn(const_err)]

#![crate_type = "lib"]

pub const Z: u32 = 0 - 1;
//~^ WARN any use of this value will cause an error
//~| WARN this was previously accepted by the compiler but is being phased out

//pub type Foo = [i32; 0 - 1];
//^ evaluation of constant value failed
