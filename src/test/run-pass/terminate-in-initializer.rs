// xfail-win32 leaks
// Issue #787
// Don't try to clean up uninitizaed locals

use std;

fn test_break() { while true { let x: @int = break; } }

fn test_cont() { let i = 0; while i < 1 { i += 1; let x: @int = cont; } }

fn test_ret() { let x: @int = ret; }

fn test_fail() {
    fn f() { let x: @int = fail; }
    task::try {|| f() };
}

fn test_fail_indirect() {
    fn f() -> ! { fail; }
    fn g() { let x: @int = f(); }
    task::try {|| g() };
}

fn main() {
    test_break();
    test_cont();
    test_ret();
    test_fail();
    test_fail_indirect();
}
