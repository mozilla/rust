// error-pattern:in non-returning function f, some control paths may return
fn f() -> ! { ret 42; fail; }
fn main() { }
