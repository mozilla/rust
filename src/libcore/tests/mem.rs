// Copyright 2014-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use core::mem::*;

#[test]
fn size_of_basic() {
    assert_eq!(size_of::<u8>(), 1);
    assert_eq!(size_of::<u16>(), 2);
    assert_eq!(size_of::<u32>(), 4);
    assert_eq!(size_of::<u64>(), 8);
}

#[test]
#[cfg(target_pointer_width = "16")]
fn size_of_16() {
    assert_eq!(size_of::<usize>(), 2);
    assert_eq!(size_of::<*const usize>(), 2);
}

#[test]
#[cfg(target_pointer_width = "32")]
fn size_of_32() {
    assert_eq!(size_of::<usize>(), 4);
    assert_eq!(size_of::<*const usize>(), 4);
}

#[test]
#[cfg(target_pointer_width = "64")]
fn size_of_64() {
    assert_eq!(size_of::<usize>(), 8);
    assert_eq!(size_of::<*const usize>(), 8);
}

#[test]
fn size_of_val_basic() {
    assert_eq!(size_of_val(&1u8), 1);
    assert_eq!(size_of_val(&1u16), 2);
    assert_eq!(size_of_val(&1u32), 4);
    assert_eq!(size_of_val(&1u64), 8);
}

#[test]
fn size_of_val_const() {
    macro_rules! ez_byte_string {
        ($name: ident, $string: expr) => {
            static $name: [u8; size_of_val($string)] = *$string;
        }
    }

    ez_byte_string!(OWO, b"what's this?");
    ez_byte_string!(EMPTY, b"");

    assert_eq!(&OWO, b"what's this?");
    assert_eq!(&EMPTY, b"");

    // ~ one day ~
    // static SIZE_OF_OWO_SLICE: usize = size_of_val(&OWO[..]);
    // static SIZE_OF_EMPTY_SLICE: usize = size_of_val(&EMPTY[..]);

    // assert_eq!(SIZE_OF_OWO_SLICE, OWO.len());
    // assert_eq!(SIZE_OF_EMPTY_SLICE, EMPTY.len());

    const SIZE_OF_U8: usize = size_of_val(&0u8);
    const SIZE_OF_U16: usize = size_of_val(&1u16);
    static SIZE_OF_U32: usize = size_of_val(&9u32);

    assert_eq!(SIZE_OF_U8, 1);
    assert_eq!(SIZE_OF_U16, 2);
    assert_eq!(SIZE_OF_U32, 4);
}

#[test]
fn align_of_basic() {
    assert_eq!(align_of::<u8>(), 1);
    assert_eq!(align_of::<u16>(), 2);
    assert_eq!(align_of::<u32>(), 4);
}

#[test]
#[cfg(target_pointer_width = "16")]
fn align_of_16() {
    assert_eq!(align_of::<usize>(), 2);
    assert_eq!(align_of::<*const usize>(), 2);
}

#[test]
#[cfg(target_pointer_width = "32")]
fn align_of_32() {
    assert_eq!(align_of::<usize>(), 4);
    assert_eq!(align_of::<*const usize>(), 4);
}

#[test]
#[cfg(target_pointer_width = "64")]
fn align_of_64() {
    assert_eq!(align_of::<usize>(), 8);
    assert_eq!(align_of::<*const usize>(), 8);
}

#[test]
fn align_of_val_basic() {
    assert_eq!(align_of_val(&1u8), 1);
    assert_eq!(align_of_val(&1u16), 2);
    assert_eq!(align_of_val(&1u32), 4);
}

#[test]
fn align_of_val_const() {
    const ALIGN_OF_U8: usize = align_of_val(&0u8);
    const ALIGN_OF_U16: usize = align_of_val(&1u16);
    static ALIGN_OF_U32: usize = align_of_val(&9u32);

    assert_eq!(ALIGN_OF_U8, 1);
    assert_eq!(ALIGN_OF_U16, 2);
    assert_eq!(ALIGN_OF_U32, 4);
}

#[test]
fn test_swap() {
    let mut x = 31337;
    let mut y = 42;
    swap(&mut x, &mut y);
    assert_eq!(x, 42);
    assert_eq!(y, 31337);
}

#[test]
fn test_replace() {
    let mut x = Some("test".to_string());
    let y = replace(&mut x, None);
    assert!(x.is_none());
    assert!(y.is_some());
}

#[test]
fn test_transmute_copy() {
    assert_eq!(1, unsafe { transmute_copy(&1) });
}

#[test]
fn test_transmute() {
    trait Foo { fn dummy(&self) { } }
    impl Foo for isize {}

    let a = box 100isize as Box<Foo>;
    unsafe {
        let x: ::core::raw::TraitObject = transmute(a);
        assert!(*(x.data as *const isize) == 100);
        let _x: Box<Foo> = transmute(x);
    }

    unsafe {
        assert_eq!(transmute::<_, Vec<u8>>("L".to_string()), [76]);
    }
}

#[test]
#[allow(dead_code)]
fn test_discriminant_send_sync() {
    enum Regular {
        A,
        B(i32)
    }
    enum NotSendSync {
        A(*const i32)
    }

    fn is_send_sync<T: Send + Sync>() { }

    is_send_sync::<Discriminant<Regular>>();
    is_send_sync::<Discriminant<NotSendSync>>();
}
