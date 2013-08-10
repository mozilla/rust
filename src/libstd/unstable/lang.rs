// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Runtime calls emitted by the compiler.

use c_str::ToCStr;
use cast::transmute;
use libc::{c_char, c_uchar, c_void, size_t, uintptr_t};
use str;
use sys;
#[cfg(not(no_rt))]
use rt::task::Task;
#[cfg(not(no_rt))]
use rt::local::Local;
#[cfg(not(no_rt))]
use rt::borrowck;

#[lang="fail_"]
#[cfg(not(no_rt))]
pub fn fail_(expr: *c_char, file: *c_char, line: size_t) -> ! {
    sys::begin_unwind_(expr, file, line);
}

#[lang="fail_"]
#[cfg(no_rt)]
pub fn fail_(expr: *c_char, file: *c_char, line: size_t) -> ! {
    unsafe { ::libc::abort() }
}

#[lang="fail_bounds_check"]
#[cfg(not(no_rt))]
pub fn fail_bounds_check(file: *c_char, line: size_t,
                         index: size_t, len: size_t) {
    let msg = fmt!("index out of bounds: the len is %d but the index is %d",
                    len as int, index as int);
    do msg.to_c_str().with_ref |buf| {
        fail_(buf, file, line);
    }
}

#[lang="fail_bounds_check"]
#[cfg(no_rt)]
pub fn fail_bounds_check(file: *c_char, line: size_t,
                         index: size_t, len: size_t) {
    unsafe { ::libc::abort() }
}

#[lang="malloc"]
#[cfg(not(no_rt))]
pub unsafe fn local_malloc(td: *c_char, size: uintptr_t) -> *c_char {
    let mut alloc = ::ptr::null();
    do Local::borrow::<Task,()> |task| {
        rtdebug!("task pointer: %x, heap pointer: %x",
                 ::borrow::to_uint(task),
                 ::borrow::to_uint(&task.heap));
        alloc = task.heap.alloc(td as *c_void, size as uint) as *c_char;
    }
    return alloc;
}

#[lang="malloc"]
#[cfg(no_rt)]
pub unsafe fn local_malloc(td: *c_char, size: uintptr_t) -> *c_char {
    transmute(::libc::malloc(transmute(size)))
}

// NB: Calls to free CANNOT be allowed to fail, as throwing an exception from
// inside a landing pad may corrupt the state of the exception handler. If a
// problem occurs, call exit instead.
#[lang="free"]
#[cfg(not(no_rt))]
pub unsafe fn local_free(ptr: *c_char) {
    ::rt::local_heap::local_free(ptr);
}

#[lang="free"]
#[cfg(no_rt)]
pub unsafe fn local_free(ptr: *c_char) {
    ::libc::free(transmute(ptr))
}

#[cfg(not(test))]
#[lang="log_type"]
#[allow(missing_doc)]
#[cfg(no_rt)]
pub fn log_type<T>(_level: u32, object: &T) { }

#[lang="borrow_as_imm"]
#[inline]
#[cfg(not(no_rt))]
pub unsafe fn borrow_as_imm(a: *u8, file: *c_char, line: size_t) -> uint {
    borrowck::borrow_as_imm(a, file, line)
}

#[lang="borrow_as_mut"]
#[inline]
#[cfg(not(no_rt))]
pub unsafe fn borrow_as_mut(a: *u8, file: *c_char, line: size_t) -> uint {
    borrowck::borrow_as_mut(a, file, line)
}

#[lang="record_borrow"]
#[cfg(not(no_rt))]
pub unsafe fn record_borrow(a: *u8, old_ref_count: uint,
                            file: *c_char, line: size_t) {
    borrowck::record_borrow(a, old_ref_count, file, line)
}

#[lang="unrecord_borrow"]
#[cfg(not(no_rt))]
pub unsafe fn unrecord_borrow(a: *u8, old_ref_count: uint,
                              file: *c_char, line: size_t) {
    borrowck::unrecord_borrow(a, old_ref_count, file, line)
}

#[lang="return_to_mut"]
#[inline]
#[cfg(not(no_rt))]
pub unsafe fn return_to_mut(a: *u8, orig_ref_count: uint,
                            file: *c_char, line: size_t) {
    borrowck::return_to_mut(a, orig_ref_count, file, line)
}

#[lang="check_not_borrowed"]
#[inline]
#[cfg(not(no_rt))]
pub unsafe fn check_not_borrowed(a: *u8,
                                 file: *c_char,
                                 line: size_t) {
    borrowck::check_not_borrowed(a, file, line)
}

#[lang="strdup_uniq"]
#[inline]
pub unsafe fn strdup_uniq(ptr: *c_uchar, len: uint) -> ~str {
    str::raw::from_buf_len(ptr, len)
}

#[lang="annihilate"]
#[cfg(not(no_rt))]
pub unsafe fn annihilate() {
    ::cleanup::annihilate()
}

#[lang="start"]
#[cfg(not(no_rt))]
pub fn start(main: *u8, argc: int, argv: **c_char,
             crate_map: *u8) -> int {
    use rt;

    unsafe {
        return do rt::start(argc, argv as **u8, crate_map) {
            let main: extern "Rust" fn() = transmute(main);
            main();
        };
    }
}
