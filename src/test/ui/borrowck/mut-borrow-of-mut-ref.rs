// run-rustfix
// Suggest not mutably borrowing a mutable reference
#![crate_type = "rlib"]

pub fn f(b: &mut i32) {
    h(&mut b);
    //~^ ERROR cannot borrow
}

pub fn g(b: &mut i32) {
    h(&mut &mut b);
    //~^ ERROR cannot borrow
}

pub fn h(_: &mut i32) {}
