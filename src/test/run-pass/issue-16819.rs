// `#[cfg]` on struct field permits empty unusable struct

struct S {
    #[cfg(untrue)]
    a: int,
}

fn main() {
    let s = S {};
}
