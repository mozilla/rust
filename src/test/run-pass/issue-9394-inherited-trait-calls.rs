// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

trait Base: Base2 + Base3{
    fn foo(&self) -> ~str;
    fn foo1(&self) -> ~str;
    fn foo2(&self) -> ~str{
        ~"base foo2"
    }
}

trait Base2: Base3{
    fn baz(&self) -> ~str;
}

trait Base3{
    fn root(&self) -> ~str;
}

trait Super: Base{
    fn bar(&self) -> ~str;
}

struct X;

impl Base for X {
    fn foo(&self) -> ~str{
        ~"base foo"
    }
    fn foo1(&self) -> ~str{
        ~"base foo1"
    }

}

impl Base2 for X {
    fn baz(&self) -> ~str{
        ~"base2 baz"
    }
}

impl Base3 for X {
    fn root(&self) -> ~str{
        ~"base3 root"
    }
}

impl Super for X {
    fn bar(&self) -> ~str{
        ~"super bar"
    }
}

pub fn main() {
    let n = X;
    let s = &n as &Super;
    fail_unless_eq!(s.bar(),~"super bar");
    fail_unless_eq!(s.foo(),~"base foo");
    fail_unless_eq!(s.foo1(),~"base foo1");
    fail_unless_eq!(s.foo2(),~"base foo2");
    fail_unless_eq!(s.baz(),~"base2 baz");
    fail_unless_eq!(s.root(),~"base3 root");
}
