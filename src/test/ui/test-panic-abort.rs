// no-prefer-dynamic
// compile-flags: --test -Cpanic=abort
// run-flags: --test-threads=1
// run-fail
// check-run-results

#![cfg(test)]

use std::io::Write;

#[test]
fn it_works() {
    assert_eq!(1 + 1, 2);
}

#[test]
#[should_panic]
fn it_panics() {
    assert_eq!(1 + 1, 4);
}

#[test]
fn it_fails() {
    println!("hello, world");
    writeln!(std::io::stdout(), "testing123").unwrap();
    writeln!(std::io::stderr(), "testing321").unwrap();
    assert_eq!(1 + 1, 5);
}

#[test]
fn it_exits() {
    std::process::exit(123);
}

#[test]
fn it_segfaults() {
    let x = unsafe { *(0 as *const u64) };
    println!("output: {}", x);
}
