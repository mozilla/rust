// Test for interaction between #[automatically_derived] attribute used by
// built-in derives and lints generated by liveness pass.
//
// edition:2018
// check-pass
#![warn(unused)]

pub trait T: Sized {
    const N: usize;
    fn t(&self) -> Self;
}

impl T for u32 {
    const N: usize = {
        let a = 0; //~ WARN unused variable: `a`
        4
    };

    fn t(&self) -> Self {
        let b = 16; //~ WARN unused variable: `b`
        0
    }
}

#[automatically_derived]
impl T for i32 {
    const N: usize = {
        let c = 0;
        4
    };

    fn t(&self) -> Self {
        let d = 17;
        0
    }
}

fn main() {}
