// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[allow(unnecessary_allocation)];

// rustc --test match_borrowed_str.rs.rs && ./match_borrowed_str.rs
extern crate extra;

fn compare(x: &str, y: &str) -> bool
{
    match x
    {
        "foo" => y == "foo",
        _ => y == "bar",
    }
}

pub fn main()
{
    fail_unless!(compare("foo", "foo"));
    fail_unless!(compare(~"foo", ~"foo"));
}
