// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// aux-build:where_clauses_xc.rs

extern crate where_clauses_xc;

use where_clauses_xc::{Equal, equal};

fn main() {
    println!("{}", equal(&1i, &2i));
    println!("{}", equal(&1i, &1i));
    println!("{}", "hello".equal(&"hello"));
    println!("{}", "hello".equals::<int,&str>(&1i, &1i, &"foo", &"bar"));
}

