fn main() {
    trait Seq { }

    impl<T> Seq<T> for Vec<T> {
        //~^ ERROR this trait takes 0 generic arguments but 1 generic argument
        /* ... */
    }

    impl Seq<bool> for u32 {
        //~^ ERROR this trait takes 0 generic arguments but 1 generic argument
        /* Treat the integer as a sequence of bits */
    }
}
