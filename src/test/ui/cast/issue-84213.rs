// run-rustfix

struct Something {
    pub field: u32,
}

fn main() {
    let mut something = Something { field: 1337 };
    let _ = something.field;

    let _pointer_to_something = something as *const Something;
    //~^ ERROR: non-primitive cast

    let _mut_pointer_to_something = something as *mut Something;
    //~^ ERROR: non-primitive cast
}
