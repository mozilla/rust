// error-pattern:unsatisfied precondition

fn foo() -> int {
    let x: int;
    let i: int;

    do  { i = 0; break; x = 0; } while x != 0

    log(debug, x);

    ret 17;
}

fn main() { log(debug, foo()); }
