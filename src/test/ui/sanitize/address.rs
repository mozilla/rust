// needs-sanitizer-support
// only-x86_64
//
// compile-flags: -Z sanitizer=address -O -g
//
// run-fail
// error-pattern: AddressSanitizer: stack-buffer-overflow
// error-pattern: 'xs' (line 15) <== Memory access at offset

#![feature(test)]

use std::hint::black_box;

fn main() {
    let xs = [0, 1, 2, 3];
    // Avoid optimizing everything out.
    let xs = black_box(xs.as_ptr());
    let code = unsafe { *xs.offset(4) };
    std::process::exit(code);
}
