// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[allow(missing_doc)];

use container::Container;
use mem::size_of;
use unstable::intrinsics::move_val_init;
use unstable::raw;
use cast::{forget, transmute};
use libc::{free, malloc, realloc};
use ops::Drop;
use vec::{VecIterator, ImmutableVector};
use libc::{c_void, size_t};
use ptr::{read_ptr, RawPtr};
use num::CheckedMul;
use option::{Option, Some, None};
use iter::{Iterator, DoubleEndedIterator};
use gc;
use gc::Trace;

pub struct Vec<T> {
    priv len: uint,
    priv cap: uint,
    priv ptr: *mut T
}

pub fn trace<T: Trace>(ptr: *(), length: uint, tracer: &mut gc::GcTracer) {
    debug!("libvec::trace: {} {}", ptr, length);
    let v: &[T] = unsafe {transmute(raw::Slice { data: ptr as *T, len: length })};
    for t in v.iter() {
        t.trace(tracer)
    }
}

impl<T: Trace> Vec<T> {
    #[inline(always)]
    pub fn new() -> Vec<T> {
        Vec { len: 0, cap: 0, ptr: 0 as *mut T }
    }

    pub fn with_capacity(capacity: uint) -> Vec<T> {
        if capacity == 0 {
            Vec::new()
        } else {
            let size = capacity.checked_mul(&size_of::<T>()).expect("out of mem");
            unsafe {
                let ptr = malloc(size as size_t);
                if ptr.is_null() { fail!("null pointer") }

                gc::register_root_changes([], [(ptr as *T, 0, trace::<T>)]);
                Vec { len: 0, cap: capacity, ptr: ptr as *mut T }
            }
        }
    }
}

impl<T> Container for Vec<T> {
    #[inline(always)]
    fn len(&self) -> uint {
        self.len
    }
}

impl<T: Trace> Vec<T> {
    #[inline(always)]
    pub fn capacity(&self) -> uint {
        self.cap
    }

    pub fn reserve(&mut self, capacity: uint) {
        if capacity >= self.len {
            let size = capacity.checked_mul(&size_of::<T>()).expect("out of mem");
            self.cap = capacity;
            unsafe {
                let ptr = realloc(self.ptr as *mut c_void, size as size_t) as *mut T;
                if ptr.is_null() { fail!("null pointer") }

                gc::register_root_changes([self.ptr as *T],
                                          [(ptr as *T, self.len, trace::<T>)]);
                self.ptr = ptr;
            }
        }
    }

    #[inline]
    pub fn shrink_to_fit(&mut self) {
        unsafe {
            if self.len == 0 {
                gc::register_root_changes([self.ptr as *T], []);
                free(self.ptr as *c_void);
                self.cap = 0;
                self.ptr = 0 as *mut T;
            } else {
                let ptr = realloc(self.ptr as *mut c_void,
                                  (self.len * size_of::<T>()) as size_t) as *mut T;
                if ptr.is_null() { fail!("null pointer") }
                gc::register_root_changes([self.ptr as *T], [(ptr as *T, self.len, trace::<T>)]);

                self.cap = self.len;
            }
        }
    }

    #[inline]
    pub fn push(&mut self, value: T) {
        if self.len == self.cap {
            if self.cap == 0 { self.cap += 2 }
            let old_size = self.cap * size_of::<T>();
            self.cap = self.cap * 2;
            let size = old_size * 2;
            if old_size > size { fail!("out of mem") }
            unsafe {
                let ptr = realloc(self.ptr as *mut c_void, size as size_t) as *mut T;
                gc::register_root_changes([self.ptr as *T],
                                          [(ptr as *T, self.len, trace::<T>)]);
                self.ptr = ptr;
            }
        }

        unsafe {
            let end = self.ptr.offset(self.len as int) as *mut T;
            move_val_init(&mut *end, value);
            self.len += 1;
            gc::update_metadata(self.ptr as *T, self.len);
        }
    }
}

impl<T> Vec<T> {
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            unsafe {
                self.len -= 1;
                gc::update_metadata(self.ptr as *T, self.len);
                Some(read_ptr(self.as_slice().unsafe_ref(self.len())))
            }
        }
    }

    #[inline]
    pub fn as_slice<'a>(&'a self) -> &'a [T] {
        let slice = raw::Slice { data: self.ptr as *T, len: self.len };
        unsafe { transmute(slice) }
    }

    #[inline]
    pub fn as_mut_slice<'a>(&'a mut self) -> &'a mut [T] {
        let slice = raw::Slice { data: self.ptr as *T, len: self.len };
        unsafe { transmute(slice) }
    }

    pub fn move_iter(self) -> MoveIterator<T> {
        unsafe {
            let iter = transmute(self.as_slice().iter());
            let ptr = self.ptr as *mut u8;
            forget(self);
            MoveIterator { allocation: ptr, iter: iter }
        }
    }
}


#[unsafe_destructor]
impl<T> Drop for Vec<T> {
    fn drop(&mut self) {
        unsafe {
            for x in self.as_slice().iter() {
                read_ptr(x as *T);
            }
            gc::register_root_changes([self.ptr as *T], []);
            free(self.ptr as *c_void)
        }
    }
}

impl<T: Trace> Trace for Vec<T> {
    fn trace(&self, tracer: &mut gc::GcTracer) {
        if tracer.pointer_first_trace(self.ptr as *()) {
            for val in self.as_slice().iter() {
                val.trace(tracer);
            }
        }
    }
}

pub struct MoveIterator<T> {
    priv allocation: *mut u8, // the block of memory allocated for the vector
    priv iter: VecIterator<'static, T>
}

impl<T> Iterator<T> for MoveIterator<T> {
    fn next(&mut self) -> Option<T> {
        unsafe {
            self.iter.next().map(|x| read_ptr(x))
        }
    }

    fn size_hint(&self) -> (uint, Option<uint>) {
        self.iter.size_hint()
    }
}

impl<T> DoubleEndedIterator<T> for MoveIterator<T> {
    fn next_back(&mut self) -> Option<T> {
        unsafe {
            self.iter.next_back().map(|x| read_ptr(x))
        }
    }
}

#[unsafe_destructor]
impl<T> Drop for MoveIterator<T> {
    fn drop(&mut self) {
        // destroy the remaining elements
        for _x in *self {}
        unsafe {
            gc::register_root_changes([self.allocation as *T], []);
            free(self.allocation as *c_void)
        }
    }
}
