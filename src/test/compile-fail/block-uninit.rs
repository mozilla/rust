// error-pattern:unsatisfied precondition constraint

fn force(f: fn()) { f(); }
fn main() { let x: int; force(fn&() { log(error, x); }); }
