#![allow(clippy::all)]
#![warn(clippy::pattern_type_mismatch)]

fn main() {}

fn should_lint() {
    let value = &Some(23);
    match value {
        Some(_) => (),
        _ => (),
    }

    let value = &mut Some(23);
    match value {
        Some(_) => (),
        _ => (),
    }
}

fn should_not_lint() {
    let value = &Some(23);
    match value {
        &Some(_) => (),
        _ => (),
    }
    match *value {
        Some(_) => (),
        _ => (),
    }

    let value = &mut Some(23);
    match value {
        &mut Some(_) => (),
        _ => (),
    }
    match *value {
        Some(_) => (),
        _ => (),
    }
}
