#![allow(dead_code)]

use crate::alloc::{GlobalAlloc, Layout, System};
use crate::cmp;
use crate::ptr;

// The minimum alignment guaranteed by the architecture. This value is used to
// add fast paths for low alignment values.
#[cfg(all(any(
    target_arch = "x86",
    target_arch = "arm",
    target_arch = "mips",
    target_arch = "powerpc",
    target_arch = "powerpc64",
    target_arch = "asmjs",
    target_arch = "wasm32",
    target_arch = "hexagon"
)))]
pub const MIN_ALIGN: usize = 8;
#[cfg(all(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "mips64",
    target_arch = "s390x",
    target_arch = "sparc64",
    target_arch = "riscv64"
)))]
pub const MIN_ALIGN: usize = 16;

pub unsafe fn realloc_fallback(
    alloc: &System,
    ptr: *mut u8,
    old_layout: Layout,
    new_size: usize,
) -> *mut u8 {
    // SAFETY: as stated in docs for GlobalAlloc::realloc, the caller
    // must guarantee that `new_size` is valid for a `Layout`.
    // The `old_layout.align()` is guaranteed to be valid as it comes
    // from a `Layout`.
    let new_layout = unsafe { Layout::from_size_align_unchecked(new_size, old_layout.align()) };

    // SAFETY: as stated in docs for GlobalAlloc::realloc, the caller
    // must guarantee that `new_size` is greater than zero.
    let new_ptr = unsafe { GlobalAlloc::alloc(alloc, new_layout) };
    if !new_ptr.is_null() {
        let size = cmp::min(old_layout.size(), new_size);
        // SAFETY: the newly allocated memory cannot overlap the previously
        // allocated memory. Also, the call to `dealloc` is safe since
        // the caller must guarantee that `ptr` is allocated via this allocator
        // and layout is the same layout that was used to allocate `ptr`.
        unsafe {
            ptr::copy_nonoverlapping(ptr, new_ptr, size);
            GlobalAlloc::dealloc(alloc, ptr, old_layout);
        }
    }
    new_ptr
}
