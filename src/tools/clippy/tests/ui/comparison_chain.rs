#![allow(dead_code)]
#![warn(clippy::comparison_chain)]

fn a() {}
fn b() {}
fn c() {}

fn f(x: u8, y: u8, z: u8) {
    // Ignored: Only one branch
    if x > y {
        a()
    }

    if x > y {
        a()
    } else if x < y {
        b()
    }

    // Ignored: Only one explicit conditional
    if x > y {
        a()
    } else {
        b()
    }

    if x > y {
        a()
    } else if x < y {
        b()
    } else {
        c()
    }

    if x > y {
        a()
    } else if y > x {
        b()
    } else {
        c()
    }

    if x > 1 {
        a()
    } else if x < 1 {
        b()
    } else if x == 1 {
        c()
    }

    // Ignored: Binop args are not equivalent
    if x > 1 {
        a()
    } else if y > 1 {
        b()
    } else {
        c()
    }

    // Ignored: Binop args are not equivalent
    if x > y {
        a()
    } else if x > z {
        b()
    } else if y > z {
        c()
    }

    // Ignored: Not binary comparisons
    if true {
        a()
    } else if false {
        b()
    } else {
        c()
    }
}

#[allow(clippy::float_cmp)]
fn g(x: f64, y: f64, z: f64) {
    // Ignored: f64 doesn't implement Ord
    if x > y {
        a()
    } else if x < y {
        b()
    }

    // Ignored: f64 doesn't implement Ord
    if x > y {
        a()
    } else if x < y {
        b()
    } else {
        c()
    }

    // Ignored: f64 doesn't implement Ord
    if x > y {
        a()
    } else if y > x {
        b()
    } else {
        c()
    }

    // Ignored: f64 doesn't implement Ord
    if x > 1.0 {
        a()
    } else if x < 1.0 {
        b()
    } else if x == 1.0 {
        c()
    }
}

fn h<T: Ord>(x: T, y: T, z: T) {
    if x > y {
        a()
    } else if x < y {
        b()
    }

    if x > y {
        a()
    } else if x < y {
        b()
    } else {
        c()
    }

    if x > y {
        a()
    } else if y > x {
        b()
    } else {
        c()
    }
}

fn main() {}
