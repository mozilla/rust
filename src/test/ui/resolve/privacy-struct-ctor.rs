// aux-build:privacy-struct-ctor.rs

extern crate privacy_struct_ctor as xcrate;

mod m {
    pub struct S(u8);
    pub struct S2 {
        s: u8
    }

    pub mod n {
        pub(in m) struct Z(pub(in m::n) u8);
    }

    use m::n::Z; // OK, only the type is imported

    fn f() {
        Z;
        //~^ ERROR expected value, found struct `Z`
    }
}

use m::S; // OK, only the type is imported
use m::S2; // OK, only the type is imported

fn main() {
    S;
    //~^ ERROR expected value, found struct `S`

    S2;
    //~^ ERROR expected value, found struct `S2`

    xcrate::S;
    //~^ ERROR expected value, found struct `xcrate::S`
}
