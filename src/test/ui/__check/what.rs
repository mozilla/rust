// check-pass
// run-rustfix
#![warn(unused_parens)]
fn test(_: u32) {}

fn main() {
    test((
        7
    ));
    //~^^^ WARN unnecessary parentheses around function argument
}
