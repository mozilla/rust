use std;
import std::ivec;

fn ivec_equal[T](v: &[T], u: &[T], element_equality_test: fn(&T, &T) -> bool )
   -> bool {
    let Lv = ivec::len(v);
    if Lv != ivec::len(u) { ret false; }
    let i = 0u;
    while i < Lv {
        if !element_equality_test(v.(i), u.(i)) { ret false; }
        i += 1u;
    }
    ret true;
}

fn builtin_equal[T](a: &T, b: &T) -> bool { ret a == b; }

fn main() {
    assert (builtin_equal(5, 5));
    assert (!builtin_equal(5, 4));
    assert (!ivec_equal(~[5, 5], ~[5], builtin_equal));
    assert (!ivec_equal(~[5, 5], ~[5, 4], builtin_equal));
    assert (!ivec_equal(~[5, 5], ~[4, 5], builtin_equal));
    assert (ivec_equal(~[5, 5], ~[5, 5], builtin_equal));

    log_err "Pass";
}
