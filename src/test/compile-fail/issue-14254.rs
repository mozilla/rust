// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

trait Foo {
    fn bar(&self);
    fn baz(&self) { }
    fn bah(_: Option<Self>) { }
}

struct BarTy {
    x : isize,
    y : f64,
}

impl BarTy {
    fn a() {}
    fn b(&self) {}
}

impl Foo for *const BarTy {
    fn bar(&self) {
        baz();
        //~^ ERROR: unresolved name `baz`. Did you mean to call `self.baz`?
        a;
        //~^ ERROR: unresolved name `a`. Did you mean to call `BarTy::a`?
    }
}

impl<'a> Foo for &'a BarTy {
    fn bar(&self) {
        baz();
        //~^ ERROR: unresolved name `baz`. Did you mean to call `self.baz`?
        x;
        //~^ ERROR: unresolved name `x`. Did you mean `self.x`?
        y;
        //~^ ERROR: unresolved name `y`. Did you mean `self.y`?
        a;
        //~^ ERROR: unresolved name `a`. Did you mean to call `BarTy::a`?
        bah;
        //~^ ERROR: unresolved name `bah`. Did you mean to call `Foo::bah`?
        b;
        //~^ ERROR: unresolved name `b`. Did you mean to call `self.b`?
    }
}

impl<'a> Foo for &'a mut BarTy {
    fn bar(&self) {
        baz();
        //~^ ERROR: unresolved name `baz`. Did you mean to call `self.baz`?
        x;
        //~^ ERROR: unresolved name `x`. Did you mean `self.x`?
        y;
        //~^ ERROR: unresolved name `y`. Did you mean `self.y`?
        a;
        //~^ ERROR: unresolved name `a`. Did you mean to call `BarTy::a`?
        bah;
        //~^ ERROR: unresolved name `bah`. Did you mean to call `Foo::bah`?
        b;
        //~^ ERROR: unresolved name `b`. Did you mean to call `self.b`?
    }
}

impl Foo for Box<BarTy> {
    fn bar(&self) {
        baz();
        //~^ ERROR: unresolved name `baz`. Did you mean to call `self.baz`?
        bah;
        //~^ ERROR: unresolved name `bah`. Did you mean to call `Foo::bah`?
    }
}

impl Foo for *const isize {
    fn bar(&self) {
        baz();
        //~^ ERROR: unresolved name `baz`. Did you mean to call `self.baz`?
        bah;
        //~^ ERROR: unresolved name `bah`. Did you mean to call `Foo::bah`?
    }
}

impl<'a> Foo for &'a isize {
    fn bar(&self) {
        baz();
        //~^ ERROR: unresolved name `baz`. Did you mean to call `self.baz`?
        bah;
        //~^ ERROR: unresolved name `bah`. Did you mean to call `Foo::bah`?
    }
}

impl<'a> Foo for &'a mut isize {
    fn bar(&self) {
        baz();
        //~^ ERROR: unresolved name `baz`. Did you mean to call `self.baz`?
        bah;
        //~^ ERROR: unresolved name `bah`. Did you mean to call `Foo::bah`?
    }
}

impl Foo for Box<isize> {
    fn bar(&self) {
        baz();
        //~^ ERROR: unresolved name `baz`. Did you mean to call `self.baz`?
        bah;
        //~^ ERROR: unresolved name `bah`. Did you mean to call `Foo::bah`?
    }
}
