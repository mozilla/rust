//! Memory allocation APIs

#![stable(feature = "alloc_module", since = "1.28.0")]

use core::intrinsics::{self, min_align_of_val, size_of_val};
use core::ptr::{self, NonNull, Unique};

#[stable(feature = "alloc_module", since = "1.28.0")]
#[doc(inline)]
pub use core::alloc::*;

#[cfg(test)]
mod tests;

extern "Rust" {
    // These are the magic symbols to call the global allocator.  rustc generates
    // them from the `#[global_allocator]` attribute if there is one, or uses the
    // default implementations in libstd (`__rdl_alloc` etc in `src/libstd/alloc.rs`)
    // otherwise.
    #[rustc_allocator]
    #[rustc_allocator_nounwind]
    fn __rust_alloc(size: usize, align: usize) -> *mut u8;
    #[rustc_allocator_nounwind]
    fn __rust_dealloc(ptr: *mut u8, size: usize, align: usize);
    #[rustc_allocator_nounwind]
    fn __rust_realloc(ptr: *mut u8, old_size: usize, align: usize, new_size: usize) -> *mut u8;
    #[rustc_allocator_nounwind]
    fn __rust_alloc_zeroed(size: usize, align: usize) -> *mut u8;
}

/// The global memory allocator.
///
/// This type implements the [`AllocRef`] trait by forwarding calls
/// to the allocator registered with the `#[global_allocator]` attribute
/// if there is one, or the `std` crate’s default.
///
/// Note: while this type is unstable, the functionality it provides can be
/// accessed through the [free functions in `alloc`](index.html#functions).
#[unstable(feature = "allocator_api", issue = "32838")]
#[derive(Copy, Clone, Default, Debug)]
pub struct Global;

/// Allocate memory with the global allocator.
///
/// This function forwards calls to the [`GlobalAlloc::alloc`] method
/// of the allocator registered with the `#[global_allocator]` attribute
/// if there is one, or the `std` crate’s default.
///
/// This function is expected to be deprecated in favor of the `alloc` method
/// of the [`Global`] type when it and the [`AllocRef`] trait become stable.
///
/// # Safety
///
/// See [`GlobalAlloc::alloc`].
///
/// # Examples
///
/// ```
/// use std::alloc::{alloc, dealloc, Layout};
///
/// unsafe {
///     let layout = Layout::new::<u16>();
///     let ptr = alloc(layout);
///
///     *(ptr as *mut u16) = 42;
///     assert_eq!(*(ptr as *mut u16), 42);
///
///     dealloc(ptr, layout);
/// }
/// ```
#[stable(feature = "global_alloc", since = "1.28.0")]
#[inline]
pub unsafe fn alloc(layout: Layout) -> *mut u8 {
    unsafe { __rust_alloc(layout.size(), layout.align()) }
}

/// Deallocate memory with the global allocator.
///
/// This function forwards calls to the [`GlobalAlloc::dealloc`] method
/// of the allocator registered with the `#[global_allocator]` attribute
/// if there is one, or the `std` crate’s default.
///
/// This function is expected to be deprecated in favor of the `dealloc` method
/// of the [`Global`] type when it and the [`AllocRef`] trait become stable.
///
/// # Safety
///
/// See [`GlobalAlloc::dealloc`].
#[stable(feature = "global_alloc", since = "1.28.0")]
#[inline]
pub unsafe fn dealloc(ptr: *mut u8, layout: Layout) {
    unsafe { __rust_dealloc(ptr, layout.size(), layout.align()) }
}

/// Reallocate memory with the global allocator.
///
/// This function forwards calls to the [`GlobalAlloc::realloc`] method
/// of the allocator registered with the `#[global_allocator]` attribute
/// if there is one, or the `std` crate’s default.
///
/// This function is expected to be deprecated in favor of the `realloc` method
/// of the [`Global`] type when it and the [`AllocRef`] trait become stable.
///
/// # Safety
///
/// See [`GlobalAlloc::realloc`].
#[stable(feature = "global_alloc", since = "1.28.0")]
#[inline]
pub unsafe fn realloc(ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
    unsafe { __rust_realloc(ptr, layout.size(), layout.align(), new_size) }
}

/// Allocate zero-initialized memory with the global allocator.
///
/// This function forwards calls to the [`GlobalAlloc::alloc_zeroed`] method
/// of the allocator registered with the `#[global_allocator]` attribute
/// if there is one, or the `std` crate’s default.
///
/// This function is expected to be deprecated in favor of the `alloc_zeroed` method
/// of the [`Global`] type when it and the [`AllocRef`] trait become stable.
///
/// # Safety
///
/// See [`GlobalAlloc::alloc_zeroed`].
///
/// # Examples
///
/// ```
/// use std::alloc::{alloc_zeroed, dealloc, Layout};
///
/// unsafe {
///     let layout = Layout::new::<u16>();
///     let ptr = alloc_zeroed(layout);
///
///     assert_eq!(*(ptr as *mut u16), 0);
///
///     dealloc(ptr, layout);
/// }
/// ```
#[stable(feature = "global_alloc", since = "1.28.0")]
#[inline]
pub unsafe fn alloc_zeroed(layout: Layout) -> *mut u8 {
    unsafe { __rust_alloc_zeroed(layout.size(), layout.align()) }
}

impl Global {
    #[inline]
    fn alloc_impl(&mut self, layout: Layout, zeroed: bool) -> Result<NonNull<[u8]>, AllocErr> {
        match layout.size() {
            0 => Ok(NonNull::slice_from_raw_parts(layout.dangling(), 0)),
            // SAFETY: `layout` is non-zero in size,
            size => unsafe {
                let raw_ptr = if zeroed { alloc_zeroed(layout) } else { alloc(layout) };
                let ptr = NonNull::new(raw_ptr).ok_or(AllocErr)?;
                Ok(NonNull::slice_from_raw_parts(ptr, size))
            },
        }
    }

    // Safety: Same as `AllocRef::grow`
    #[inline]
    unsafe fn grow_impl(
        &mut self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
        zeroed: bool,
    ) -> Result<NonNull<[u8]>, AllocErr> {
        debug_assert!(
            new_layout.size() >= old_layout.size(),
            "`new_layout.size()` must be greater than or equal to `old_layout.size()`"
        );

        match old_layout.size() {
            0 => self.alloc_impl(new_layout, zeroed),

            // SAFETY: `new_size` is non-zero as `old_size` is greater than or equal to `new_size`
            // as required by safety conditions. Other conditions must be upheld by the caller
            old_size if old_layout.align() == new_layout.align() => unsafe {
                let new_size = new_layout.size();

                // `realloc` probably checks for `new_size >= old_layout.size()` or something similar.
                intrinsics::assume(new_size >= old_layout.size());

                let raw_ptr = realloc(ptr.as_ptr(), old_layout, new_size);
                let ptr = NonNull::new(raw_ptr).ok_or(AllocErr)?;
                if zeroed {
                    raw_ptr.add(old_size).write_bytes(0, new_size - old_size);
                }
                Ok(NonNull::slice_from_raw_parts(ptr, new_size))
            },

            // SAFETY: because `new_layout.size()` must be greater than or equal to `old_size`,
            // both the old and new memory allocation are valid for reads and writes for `old_size`
            // bytes. Also, because the old allocation wasn't yet deallocated, it cannot overlap
            // `new_ptr`. Thus, the call to `copy_nonoverlapping` is safe. The safety contract
            // for `dealloc` must be upheld by the caller.
            old_size => unsafe {
                let new_ptr = self.alloc_impl(new_layout, zeroed)?;
                ptr::copy_nonoverlapping(ptr.as_ptr(), new_ptr.as_mut_ptr(), old_size);
                self.dealloc(ptr, old_layout);
                Ok(new_ptr)
            },
        }
    }
}

#[unstable(feature = "allocator_api", issue = "32838")]
unsafe impl AllocRef for Global {
    #[inline]
    fn alloc(&mut self, layout: Layout) -> Result<NonNull<[u8]>, AllocErr> {
        self.alloc_impl(layout, false)
    }

    #[inline]
    fn alloc_zeroed(&mut self, layout: Layout) -> Result<NonNull<[u8]>, AllocErr> {
        self.alloc_impl(layout, true)
    }

    #[inline]
    unsafe fn dealloc(&mut self, ptr: NonNull<u8>, layout: Layout) {
        if layout.size() != 0 {
            // SAFETY: `layout` is non-zero in size,
            // other conditions must be upheld by the caller
            unsafe { dealloc(ptr.as_ptr(), layout) }
        }
    }

    #[inline]
    unsafe fn grow(
        &mut self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocErr> {
        // SAFETY: all conditions must be upheld by the caller
        unsafe { self.grow_impl(ptr, old_layout, new_layout, false) }
    }

    #[inline]
    unsafe fn grow_zeroed(
        &mut self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocErr> {
        // SAFETY: all conditions must be upheld by the caller
        unsafe { self.grow_impl(ptr, old_layout, new_layout, true) }
    }

    #[inline]
    unsafe fn shrink(
        &mut self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocErr> {
        debug_assert!(
            new_layout.size() <= old_layout.size(),
            "`new_layout.size()` must be smaller than or equal to `old_layout.size()`"
        );

        match new_layout.size() {
            // SAFETY: conditions must be upheld by the caller
            0 => unsafe {
                self.dealloc(ptr, old_layout);
                Ok(NonNull::slice_from_raw_parts(new_layout.dangling(), 0))
            },

            // SAFETY: `new_size` is non-zero. Other conditions must be upheld by the caller
            new_size if old_layout.align() == new_layout.align() => unsafe {
                // `realloc` probably checks for `new_size <= old_layout.size()` or something similar.
                intrinsics::assume(new_size <= old_layout.size());

                let raw_ptr = realloc(ptr.as_ptr(), old_layout, new_size);
                let ptr = NonNull::new(raw_ptr).ok_or(AllocErr)?;
                Ok(NonNull::slice_from_raw_parts(ptr, new_size))
            },

            // SAFETY: because `new_size` must be smaller than or equal to `old_layout.size()`,
            // both the old and new memory allocation are valid for reads and writes for `new_size`
            // bytes. Also, because the old allocation wasn't yet deallocated, it cannot overlap
            // `new_ptr`. Thus, the call to `copy_nonoverlapping` is safe. The safety contract
            // for `dealloc` must be upheld by the caller.
            new_size => unsafe {
                let new_ptr = self.alloc(new_layout)?;
                ptr::copy_nonoverlapping(ptr.as_ptr(), new_ptr.as_mut_ptr(), new_size);
                self.dealloc(ptr, old_layout);
                Ok(new_ptr)
            },
        }
    }
}

/// The allocator for unique pointers.
// This function must not unwind. If it does, MIR codegen will fail.
#[cfg(not(test))]
#[lang = "exchange_malloc"]
#[inline]
unsafe fn exchange_malloc(size: usize, align: usize) -> *mut u8 {
    let layout = unsafe { Layout::from_size_align_unchecked(size, align) };
    match Global.alloc(layout) {
        Ok(ptr) => ptr.as_mut_ptr(),
        Err(_) => handle_alloc_error(layout),
    }
}

#[cfg_attr(not(test), lang = "box_free")]
#[inline]
// This signature has to be the same as `Box`, otherwise an ICE will happen.
// When an additional parameter to `Box` is added (like `A: AllocRef`), this has to be added here as
// well.
// For example if `Box` is changed to  `struct Box<T: ?Sized, A: AllocRef>(Unique<T>, A)`,
// this function has to be changed to `fn box_free<T: ?Sized, A: AllocRef>(Unique<T>, A)` as well.
pub(crate) unsafe fn box_free<T: ?Sized>(ptr: Unique<T>) {
    unsafe {
        let size = size_of_val(ptr.as_ref());
        let align = min_align_of_val(ptr.as_ref());
        let layout = Layout::from_size_align_unchecked(size, align);
        Global.dealloc(ptr.cast().into(), layout)
    }
}

/// Abort on memory allocation error or failure.
///
/// Callers of memory allocation APIs wishing to abort computation
/// in response to an allocation error are encouraged to call this function,
/// rather than directly invoking `panic!` or similar.
///
/// The default behavior of this function is to print a message to standard error
/// and abort the process.
/// It can be replaced with [`set_alloc_error_hook`] and [`take_alloc_error_hook`].
///
/// [`set_alloc_error_hook`]: ../../std/alloc/fn.set_alloc_error_hook.html
/// [`take_alloc_error_hook`]: ../../std/alloc/fn.take_alloc_error_hook.html
#[stable(feature = "global_alloc", since = "1.28.0")]
#[rustc_allocator_nounwind]
pub fn handle_alloc_error(layout: Layout) -> ! {
    extern "Rust" {
        #[lang = "oom"]
        fn oom_impl(layout: Layout) -> !;
    }
    unsafe { oom_impl(layout) }
}
