trait Nat {
    const VALUE: usize;
}

struct Zero;
struct Succ<N>(N);

impl Nat for Zero {
    const VALUE: usize = 0;
}

impl<N: Nat> Nat for Succ<N> {
    const VALUE: usize = N::VALUE + 1;
}

fn main() {
    let x: [i32; <Succ<Succ<Succ<Succ<Zero>>>>>::VALUE] = [1, 2, 3, 4];
}
