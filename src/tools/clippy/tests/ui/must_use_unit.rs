//run-rustfix
// aux-build:macro_rules.rs

#![warn(clippy::must_use_unit)]
#![allow(clippy::unused_unit)]

#[macro_use]
extern crate macro_rules;

#[must_use]
pub fn must_use_default() {}

#[must_use]
pub fn must_use_unit() -> () {}

#[must_use = "With note"]
pub fn must_use_with_note() {}

fn main() {
    must_use_default();
    must_use_unit();
    must_use_with_note();

    // We should not lint in external macros
    must_use_unit!();
}
