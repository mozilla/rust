// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(catch_expr)]

pub fn main() {
    let catch_result = catch {
        let x = 5;
        x
    };
    assert_eq!(catch_result, 5);

    let mut catch = true;
    while catch { catch = false; }
    assert_eq!(catch, false);

    catch = if catch { false } else { true };
    assert_eq!(catch, true);

    match catch {
        _ => {}
    };
}
