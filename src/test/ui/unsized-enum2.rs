// Copyright 206 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::ops::Deref;

// Due to aggressive error message deduplication, we require 20 *different*
// unsized types (even Path and [u8] are considered the "same").

trait Foo {}
trait Bar {}
trait FooBar {}
trait BarFoo {}

trait PathHelper1 {}
trait PathHelper2 {}
trait PathHelper3 {}
trait PathHelper4 {}

struct Path1(PathHelper1);
struct Path2(PathHelper2);
struct Path3(PathHelper3);
struct Path4(PathHelper4);

enum E<W: ?Sized, X: ?Sized, Y: ?Sized, Z: ?Sized> {
    // parameter
    VA(W),
    //~^ ERROR the size for value values of type
    VB{x: X},
    //~^ ERROR the size for value values of type
    VC(isize, Y),
    //~^ ERROR the size for value values of type
    VD{u: isize, x: Z},
    //~^ ERROR the size for value values of type

    // slice / str
    VE([u8]),
    //~^ ERROR the size for value values of type
    VF{x: str},
    //~^ ERROR the size for value values of type
    VG(isize, [f32]),
    //~^ ERROR the size for value values of type
    VH{u: isize, x: [u32]},
    //~^ ERROR the size for value values of type

    // unsized struct
    VI(Path1),
    //~^ ERROR the size for value values of type
    VJ{x: Path2},
    //~^ ERROR the size for value values of type
    VK(isize, Path3),
    //~^ ERROR the size for value values of type
    VL{u: isize, x: Path4},
    //~^ ERROR the size for value values of type

    // plain trait
    VM(Foo),
    //~^ ERROR the size for value values of type
    VN{x: Bar},
    //~^ ERROR the size for value values of type
    VO(isize, FooBar),
    //~^ ERROR the size for value values of type
    VP{u: isize, x: BarFoo},
    //~^ ERROR the size for value values of type

    // projected
    VQ(<&'static [i8] as Deref>::Target),
    //~^ ERROR the size for value values of type
    VR{x: <&'static [char] as Deref>::Target},
    //~^ ERROR the size for value values of type
    VS(isize, <&'static [f64] as Deref>::Target),
    //~^ ERROR the size for value values of type
    VT{u: isize, x: <&'static [i32] as Deref>::Target},
    //~^ ERROR the size for value values of type
}


fn main() { }

