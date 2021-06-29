// Test that AsRepr can be derived to convert an int-repr'd enum into its repr.

// run-pass
// gate-test-enum_as_repr

#![feature(enum_as_repr)]

use std::enums::AsRepr;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(AsRepr, Debug, PartialEq, Eq)]
#[repr(u8)]
enum PositiveNumber {
    Zero,
    One,
}

#[derive(AsRepr, Debug, PartialEq, Eq)]
#[repr(i8)]
enum Number {
    MinusOne = -1,
    Zero,
    One,
    Four = 4,
}

static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

#[derive(AsRepr, Debug, PartialEq, Eq)]
#[repr(usize)]
enum DroppableNumber {
    Zero,
    One,
}

impl Drop for DroppableNumber {
    fn drop(&mut self) {
        DROP_COUNT.fetch_add(1, Ordering::SeqCst);
    }
}

fn main() {
    let n = PositiveNumber::Zero.as_repr();
    assert_eq!(n, 0);
    let n = PositiveNumber::One.as_repr();
    assert_eq!(n, 1);

    let n = std::mem::discriminant(&PositiveNumber::Zero).as_repr();
    assert_eq!(n, 0);
    let n = std::mem::discriminant(&PositiveNumber::One).as_repr();
    assert_eq!(n, 1);

    let n = Number::MinusOne.as_repr();
    assert_eq!(n, -1);
    let n = Number::Zero.as_repr();
    assert_eq!(n, 0);
    let n = Number::One.as_repr();
    assert_eq!(n, 1);
    let n = Number::Four.as_repr();
    assert_eq!(n, 4);

    assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 0);
    {
        let n = DroppableNumber::Zero;
        assert_eq!(n.as_repr(), 0);
    }
    assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 1);
    {
        let n = DroppableNumber::One;
        assert_eq!(n.as_repr(), 1);
    }
    assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 2);
}
