// Issue 46112: An extern crate pub re-exporting libcore was causing
// paths rooted from `std` to be misrendered in the diagnostic output.

// ignore-windows
// aux-build:xcrate_issue_43189_a.rs
// aux-build:xcrate_issue_43189_b.rs

extern crate xcrate_issue_43189_b;
fn main() {
    ().a();
    //~^ ERROR no method named `a` found for type `()` in the current scope [E0599]
}
