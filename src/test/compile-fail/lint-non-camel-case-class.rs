#[forbid(non_camel_case_types)]
struct foo { //~ ERROR type, variant, or trait must be camel case
    let bar: int;
}

fn main() {
}
