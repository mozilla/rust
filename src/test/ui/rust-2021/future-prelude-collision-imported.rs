// run-rustfix
// edition:2018
// check-pass
#![warn(future_prelude_collision)]
#![allow(dead_code)]
#![allow(unused_imports)]

mod m {
    pub trait TryIntoU32 {
        fn try_into(self) -> Result<u32, ()>;
    }

    impl TryIntoU32 for u8 {
        fn try_into(self) -> Result<u32, ()> {
            Ok(self as u32)
        }
    }

    pub trait AnotherTrick {}
}

mod a {
    use crate::m::TryIntoU32;

    fn main() {
        // In this case, we can just use `TryIntoU32`
        let _: u32 = 3u8.try_into().unwrap();
        //~^ WARNING trait method `try_into` will become ambiguous in Rust 2021
        //~^^ WARNING this was previously accepted by the compiler but is being phased out; it will become a hard error in the 2021 edition!
    }
}

mod b {
    use crate::m::AnotherTrick as TryIntoU32;
    use crate::m::TryIntoU32 as _;

    fn main() {
        // In this case, a `TryIntoU32::try_into` rewrite will not work, and we need to use
        // the path `crate::m::TryIntoU32` (with which it was imported).
        let _: u32 = 3u8.try_into().unwrap();
        //~^ WARNING trait method `try_into` will become ambiguous in Rust 2021
        //~^^ WARNING this was previously accepted by the compiler but is being phased out; it will become a hard error in the 2021 edition!
    }
}

mod c {
    use super::m::TryIntoU32 as _;
    use crate::m::AnotherTrick as TryIntoU32;

    fn main() {
        // In this case, a `TryIntoU32::try_into` rewrite will not work, and we need to use
        // the path `super::m::TryIntoU32` (with which it was imported).
        let _: u32 = 3u8.try_into().unwrap();
        //~^ WARNING trait method `try_into` will become ambiguous in Rust 2021
        //~^^ WARNING this was previously accepted by the compiler but is being phased out; it will become a hard error in the 2021 edition!
    }
}

fn main() {}
