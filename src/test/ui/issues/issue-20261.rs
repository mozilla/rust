fn main() {
    // NB: this (almost) typechecks when default binding modes are enabled.
    for (ref i,) in [].iter() {
        i.clone();
        //~^ ERROR type annotations needed
    }
}
