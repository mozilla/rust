// run-pass

// Test that both `&&` and `||` actually short-circuit.
// Formerly, both sides were evaluated unconditionally

#![feature(const_panic)]

const TRUE: bool = true || panic!();
const FALSE: bool = false && panic!();

fn main() {
    assert!(TRUE);
    assert!(!FALSE);
}
