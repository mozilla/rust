use std;
import str::*;
import uint::*;

fn main() unsafe {
    let a: uint = 1u;
    let b: uint = 4u;
    let c: uint = 17u;
    check (le(a, b));
    c <- a;
    log(debug, str::unsafe::safe_slice("kitties", c, b));
}
