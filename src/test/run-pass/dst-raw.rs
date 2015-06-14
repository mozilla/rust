// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Test DST raw pointers


trait Trait {
    fn foo(&self) -> isize;
}

struct A {
    f: isize
}
impl Trait for A {
    fn foo(&self) -> isize {
        self.f
    }
}

struct Foo<T: ?Sized> {
    f: T
}

pub fn main() {
    // raw trait object
    let x = A { f: 42 };
    let z: *const Trait = &x;
    let r = unsafe {
        (&*z).foo()
    };
    assert_eq!(r, 42);

    // raw DST struct
    let p = Foo {f: A { f: 42 }};
    let o: *const Foo<Trait> = &p;
    let r = unsafe {
        (&*o).f.foo()
    };
    assert_eq!(r, 42);

    // raw slice
    let a: *const [_] = &[1, 2, 3];
    unsafe {
        let b = (*a)[2];
        assert_eq!(b, 3);
        let len = (*a).len();
        assert_eq!(len, 3);
    }

    // raw slice with explicit cast
    let a = &[1, 2, 3] as *const [i32];
    unsafe {
        let b = (*a)[2];
        assert_eq!(b, 3);
        let len = (*a).len();
        assert_eq!(len, 3);
    }

    // raw DST struct with slice
    let c: *const Foo<[_]> = &Foo {f: [1, 2, 3]};
    unsafe {
        let b = (&*c).f[0];
        assert_eq!(b, 1);
        let len = (&*c).f.len();
        assert_eq!(len, 3);
    }

    // all of the above with *mut
    let mut x = A { f: 42 };
    let z: *mut Trait = &mut x;
    let r = unsafe {
        (&*z).foo()
    };
    assert_eq!(r, 42);

    let mut p = Foo {f: A { f: 42 }};
    let o: *mut Foo<Trait> = &mut p;
    let r = unsafe {
        (&*o).f.foo()
    };
    assert_eq!(r, 42);

    let a: *mut [_] = &mut [1, 2, 3];
    unsafe {
        let b = (*a)[2];
        assert_eq!(b, 3);
        let len = (*a).len();
        assert_eq!(len, 3);
    }

    let a = &mut [1, 2, 3] as *mut [i32];
    unsafe {
        let b = (*a)[2];
        assert_eq!(b, 3);
        let len = (*a).len();
        assert_eq!(len, 3);
    }

    let c: *mut Foo<[_]> = &mut Foo {f: [1, 2, 3]};
    unsafe {
        let b = (&*c).f[0];
        assert_eq!(b, 1);
        let len = (&*c).f.len();
        assert_eq!(len, 3);
    }
}
