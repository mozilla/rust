// -*- rust -*-
fn ho(f: fn@(int) -> int) -> int { let n: int = f(3); ret n; }

fn direct(x: int) -> int { ret x + 1; }

fn main() {
    let a: int = direct(3); // direct
    let b: int = ho(direct); // indirect unbound

    assert (a == b);
}
