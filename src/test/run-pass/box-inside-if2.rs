// -*- rust -*-
// xfail-stage0
use std;
import std::_vec;

fn some_vec(int x) -> vec[int] {
  ret vec();
}

fn is_odd(int n) -> bool { ret true; }

fn length_is_even(vec[int] vs) -> bool {
  ret true;
}

fn foo(int acc, int n) -> () {
 
  if (is_odd(n) || length_is_even(some_vec(1))) {
    log_err("bloop");
  }
}

fn main() {
  foo(67, 5);
}
