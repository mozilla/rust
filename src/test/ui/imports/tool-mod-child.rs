use clippy::a; //~ ERROR unresolved import `clippy`
use clippy::a::b; //~ ERROR failed to resolve: maybe a missing crate `clippy`?

use rustdoc::a; //~ ERROR unresolved import `rustdoc`
use rustdoc::a::b; //~ ERROR failed to resolve: maybe a missing crate `rustdoc`?

fn main() {}
