fn siphash<T>() {

    trait t {
        fn g(&self, x: T) -> T;  //~ ERROR can't use type parameters from outer function
        //~^ ERROR can't use type parameters from outer function
    }
}

fn main() {}
