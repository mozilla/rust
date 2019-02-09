// Test that cleanups for the RHS of shortcircuiting operators work.

// pretty-expanded FIXME(#23616)
// ignore-cloudabi no std::env support

use std::env;

pub fn main() {
    let args: Vec<String> = env::args().collect();

    // Here, the rvalue `"signal".to_string()` requires cleanup. Older versions
    // of the code had a problem that the cleanup scope for this
    // expression was the end of the `if`, and as the `"signal".to_string()`
    // expression was never evaluated, we wound up trying to clean
    // uninitialized memory.

    if args.len() >= 2 && args[1] == "signal" {
        // Raise a segfault.
        unsafe { *(0 as *mut isize) = 0; }
    }
}
