#![allow(unused_assignments)]
#![allow(unused_variables)]

// run-fail
// error-pattern:wooooo
// ignore-emscripten no processes

fn main() {
    let mut a = 1;
    if 1 == 1 {
        a = 2;
    }
    panic!(format!("woooo{}", "o"));
}
