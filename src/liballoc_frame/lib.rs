// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![crate_name = "alloc_frame"]
#![crate_type = "rlib"]
#![no_std]
#![allocator]
#![cfg_attr(not(stage0), deny(warnings))]
#![unstable(feature = "alloc_frame",
            reason = "this library is unlikely to be stabilized in its current \
                      form or name",
            issue = "0")]
#![feature(allocator)]
#![feature(const_fn)]
#![feature(staged_api)]
#![feature(libc)]

extern crate libc;

use core::ptr;
use core::sync::atomic::{AtomicBool, Ordering};

// The minimum alignment guaranteed by the architecture. This value is used to
// add fast paths for low alignment values. In practice, the alignment is a
// constant at the call site and the branch will be optimized out.
#[cfg(all(any(target_arch = "x86",
              target_arch = "arm",
              target_arch = "mips",
              target_arch = "powerpc",
              target_arch = "powerpc64",
              target_arch = "asmjs",
              target_arch = "wasm32")))]
const MIN_ALIGN: usize = 8;
#[cfg(all(any(target_arch = "x86_64",
              target_arch = "aarch64",
              target_arch = "mips64",
              target_arch = "s390x")))]
const MIN_ALIGN: usize = 16;

// The size of the chunk which is actually allocated
const CHUNK_SIZE: usize = 4096 * 16;
const CHUNK_ALIGN: usize = 4096;

static mut HEAP: *mut u8 = ptr::null_mut();
static mut HEAP_LEFT: usize = 0;
static HEAP_MUTEX: AtomicBool = AtomicBool::new(false);

#[no_mangle]
pub extern "C" fn __rust_allocate(size: usize, align: usize) -> *mut u8 {
    let new_align = if align < MIN_ALIGN { MIN_ALIGN } else { align };
    let new_size = (size + new_align - 1) & !(new_align - 1);

    unsafe {
        if new_size > CHUNK_SIZE {
            return imp::allocate(size, align);
        }
        
        while HEAP_MUTEX.compare_and_swap(false, true, Ordering::SeqCst) {}

        if new_size < HEAP_LEFT {
            let p = HEAP;
            HEAP = p.offset(new_size as isize);
            HEAP_LEFT -= new_size;
            HEAP_MUTEX.store(false, Ordering::SeqCst);
            return p;
        } else {
            let p = imp::allocate(CHUNK_SIZE, CHUNK_ALIGN);
            HEAP = p.offset(new_size as isize);
            HEAP_LEFT = CHUNK_SIZE - new_size;
            HEAP_MUTEX.store(false, Ordering::SeqCst);
            return p;
        }
    }
}

#[no_mangle]
pub extern "C" fn __rust_deallocate(_ptr: *mut u8, _old_size: usize, _align: usize) {
}

#[no_mangle]
pub extern "C" fn __rust_reallocate(ptr: *mut u8,
                                    old_size: usize,
                                    size: usize,
                                    align: usize)
                                    -> *mut u8 {
    let new_ptr = __rust_allocate(size, align);
    unsafe { libc::memcpy(new_ptr as *mut _, ptr as *mut _, old_size); } 
    new_ptr
}

#[no_mangle]
pub extern "C" fn __rust_reallocate_inplace(_ptr: *mut u8,
                                            old_size: usize,
                                            _size: usize,
                                            _align: usize)
                                            -> usize {
    old_size
}

#[no_mangle]
pub extern "C" fn __rust_usable_size(size: usize, _align: usize) -> usize {
    size
}

#[cfg(any(unix, target_os = "redox"))]
mod imp {
    use libc;
    use core::ptr;
    use MIN_ALIGN;

    pub unsafe fn allocate(size: usize, align: usize) -> *mut u8 {
        if align <= MIN_ALIGN {
            libc::malloc(size as libc::size_t) as *mut u8
        } else {
            aligned_malloc(size, align)
        }
    }

    #[cfg(any(target_os = "android", target_os = "redox"))]
    unsafe fn aligned_malloc(size: usize, align: usize) -> *mut u8 {
        // On android we currently target API level 9 which unfortunately
        // doesn't have the `posix_memalign` API used below. Instead we use
        // `memalign`, but this unfortunately has the property on some systems
        // where the memory returned cannot be deallocated by `free`!
        //
        // Upon closer inspection, however, this appears to work just fine with
        // Android, so for this platform we should be fine to call `memalign`
        // (which is present in API level 9). Some helpful references could
        // possibly be chromium using memalign [1], attempts at documenting that
        // memalign + free is ok [2] [3], or the current source of chromium
        // which still uses memalign on android [4].
        //
        // [1]: https://codereview.chromium.org/10796020/
        // [2]: https://code.google.com/p/android/issues/detail?id=35391
        // [3]: https://bugs.chromium.org/p/chromium/issues/detail?id=138579
        // [4]: https://chromium.googlesource.com/chromium/src/base/+/master/
        //                                       /memory/aligned_memory.cc
        libc::memalign(align as libc::size_t, size as libc::size_t) as *mut u8
    }

    #[cfg(not(any(target_os = "android", target_os = "redox")))]
    unsafe fn aligned_malloc(size: usize, align: usize) -> *mut u8 {
        let mut out = ptr::null_mut();
        let ret = libc::posix_memalign(&mut out, align as libc::size_t, size as libc::size_t);
        if ret != 0 {
            ptr::null_mut()
        } else {
            out as *mut u8
        }
    }
}

#[cfg(windows)]
#[allow(bad_style)]
mod imp {
    use MIN_ALIGN;

    type LPVOID = *mut u8;
    type HANDLE = LPVOID;
    type SIZE_T = usize;
    type DWORD = u32;

    extern "system" {
        fn GetProcessHeap() -> HANDLE;
        fn HeapAlloc(hHeap: HANDLE, dwFlags: DWORD, dwBytes: SIZE_T) -> LPVOID;
    }

    #[repr(C)]
    struct Header(*mut u8);

    unsafe fn get_header<'a>(ptr: *mut u8) -> &'a mut Header {
        &mut *(ptr as *mut Header).offset(-1)
    }

    unsafe fn align_ptr(ptr: *mut u8, align: usize) -> *mut u8 {
        let aligned = ptr.offset((align - (ptr as usize & (align - 1))) as isize);
        *get_header(aligned) = Header(ptr);
        aligned
    }

    pub unsafe fn allocate(size: usize, align: usize) -> *mut u8 {
        if align <= MIN_ALIGN {
            HeapAlloc(GetProcessHeap(), 0, size as SIZE_T) as *mut u8
        } else {
            let ptr = HeapAlloc(GetProcessHeap(), 0, (size + align) as SIZE_T) as *mut u8;
            if ptr.is_null() {
                return ptr;
            }
            align_ptr(ptr, align)
        }
    }
}
