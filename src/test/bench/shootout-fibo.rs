// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::os;

fn fib(n: int) -> int {
    if n < 2 {
        return 1;
    } else {
        return fib(n - 1) + fib(n - 2);
    }
}

fn main() {
    let args = os::args();
    let args = if os::getenv("RUST_BENCH").is_some() {
        vec!("".to_string(), "40".to_string())
    } else if args.len() <= 1u {
        vec!("".to_string(), "30".to_string())
    } else {
        args.into_iter().collect()
    };
    let n = args[1].parse().unwrap();
    println!("{}\n", fib(n));
}
