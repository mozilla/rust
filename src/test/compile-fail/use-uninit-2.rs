// error-pattern:unsatisfied precondition

fn foo(x: int) { log(debug, x); }

fn main() { let x: int; if 1 > 2 { x = 10; } foo(x); }
