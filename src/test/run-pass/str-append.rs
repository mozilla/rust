

// -*- rust -*-
use std;
import std::str;

fn test1() {
    let s: istr = ~"hello";
    s += ~"world";
    log s;
    assert (s[9] == 'd' as u8);
}

fn test2() {
    // This tests for issue #163

    let ff: istr = ~"abc";
    let a: istr = ff + ~"ABC" + ff;
    let b: istr = ~"ABC" + ff + ~"ABC";
    log a;
    log b;
    assert (str::eq(a, ~"abcABCabc"));
    assert (str::eq(b, ~"ABCabcABC"));
}

fn main() { test1(); test2(); }
