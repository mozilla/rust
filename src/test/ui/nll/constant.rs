// Test that MIR borrowck and NLL analysis can handle constants of
// arbitrary types without ICEs.

// compile-flags:-Zborrowck=mir
// compile-pass

const HI: &str = "hi";

fn main() {
    assert_eq!(HI, "hi");
}
