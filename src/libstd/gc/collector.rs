// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use container::Container;
use gc::collector::ptr_map::PtrMap;
use iter::Iterator;
use libc;
use option::{Some, None};
use ops::Drop;
use ptr::RawPtr;
use vec::{MutableVector, OwnedVector, ImmutableVector};
use unstable::intrinsics;

mod ptr_map;

static DEFAULT_ALLOCS_PER_COLLECTION_MASK: uint = (1 << 10) - 1;

/// A thread local (almost) conservative garbage collector.
///
/// This makes no effort to check global variables, or even
/// thread-local ones.
///
/// # Design
///
/// This is a very crude mark-and-sweep conservative[1]
/// non-generational garbage collector. It stores two sets of
/// pointers, the GC'd pointers themselves, and regions of memory that
/// are roots for GC'd objects (that is, the regions that could
/// possibly contain references to GC'd pointers).
///
/// For a collection, it scans the roots and the stack to find any
/// bitpatterns that look like GC'd pointers that we know about, and
/// then scans each of these to record all the reachable
/// objects. After doing so, any unreachable objects are freed.
///
/// Currently, this just calls `malloc` and `free` for every
/// allocation. It could (and should) be reusing allocations.
///
/// Also, this only counts pointers to the start of GC'd memory
/// regions as valid. This is fine for a simple type like `Gc<T>`,
/// since the only way to get a pointer actually pointing inside the
/// contents requires a `.borrow()`, which freezes the `Gc<T>`
/// reference that was borrowed. This `Gc<T>` reference is
/// presumably[2] in a root (or some other place that is scanned) and
/// points at the start of the allocation, so the subpointer will
/// always be valid. (Yay for lifetimes & borrowing.)
///
/// [1]: it has some smarts, the user can indicate that an allocation
/// should not be scanned, so that allocations that can never
/// contain a GC pointer are ignored.
///
/// [2]: If the `Gc<T>` reference is reachable but not being scanned
/// then the user already has a problem.
pub struct GarbageCollector {
    /// Non-garbage-collectable roots
    priv roots: PtrMap,
    /// Garbage-collectable pointers.
    priv gc_ptrs: PtrMap,
    /// number of GC-able allocations performed.
    priv gc_allocs: uint,
    /// the number of allocations to do before collection (in mask
    /// form, i.e. we are detecting `gc_allocs % (1 << n) == 0` for
    /// some n).
    priv gc_allocs_per_collection_mask: uint
}

unsafe fn alloc_inner(ptrs: &mut PtrMap, size: uint, scan: bool) -> *mut u8 {
    let ptr = if scan {
        libc::calloc(size as libc::size_t, 1)
    } else {
        // no need to clear if we're not going to be scanning it
        // anyway.
        libc::malloc(size as libc::size_t)
    };

    if ptr.is_null() {
        intrinsics::abort();
    }
    ptrs.insert_alloc(ptr as uint, size, scan);
    ptr as *mut u8
}

impl GarbageCollector {
    pub fn new() -> GarbageCollector {
        GarbageCollector {
            roots: PtrMap::new(),
            gc_ptrs: PtrMap::new(),
            gc_allocs: 0,
            gc_allocs_per_collection_mask: DEFAULT_ALLOCS_PER_COLLECTION_MASK
        }
    }

    /// Run a garbage collection if we're due for one.
    pub unsafe fn occasional_collection(&mut self, stack_top: uint) {
        if self.gc_allocs & self.gc_allocs_per_collection_mask == 0 {
            self.collect(stack_top)
        }
    }

    /// Allocate `size` bytes of memory such that they are scanned for
    /// other GC'd pointers (for use by types like `Gc<Gc<int>>`).
    pub unsafe fn alloc_gc(&mut self, size: uint) -> *mut u8 {
        self.gc_allocs += 1;
        alloc_inner(&mut self.gc_ptrs, size, true)
    }

    /// Allocate `size` bytes of memory such that they are not scanned
    /// for other GC'd pointers; this should be used for types like
    /// `Gc<int>`, or (in theory) `Gc<~Gc<int>>` (note the
    /// indirection).
    pub unsafe fn alloc_gc_no_scan(&mut self, size: uint) -> *mut u8 {
        self.gc_allocs += 1;
        alloc_inner(&mut self.gc_ptrs, size, false)
    }

    /// Register the block of memory [`start`, `end`) for scanning for
    /// GC'd pointers.
    pub unsafe fn register_root(&mut self, start: *(), end: *()) {
        self.roots.insert_alloc(start as uint, end as uint - start as uint, true)
    }
    /// Stop scanning the root starting at `start` for GC'd pointers.
    pub unsafe fn unregister_root(&mut self, start: *()) {
        self.roots.remove(start as uint);
    }

    /// Collect garbage. An upper bound on the position of any GC'd
    /// pointers on the stack should be passed as `stack_top`.
    pub unsafe fn collect(&mut self, stack_top: uint) {
        clear_registers(0, 0, 0, 0, 0, 0);

        let stack: uint = 1;
        let stack_end = &stack as *uint;

        let GarbageCollector { ref mut roots, ref mut gc_ptrs, .. } = *self;

        // Step 1.
        gc_ptrs.mark_all_unreachable();

        // Step 2. mark any reachable pointers

        // the list of pointers that are reachable and scannable, but
        // haven't actually been scanned yet.
        let mut grey_list = ~[];

        // Step 2.1: search for GC'd pointers in any registered roots.
        for (low, descr) in roots.iter() {
            mark_words_between(gc_ptrs, &mut grey_list,
                               low as *uint, descr.high as *uint)
        }

        // Step 2.2: search for them on the stack.
        mark_words_between(gc_ptrs, &mut grey_list, stack_end, stack_top as *uint);

        // Step 2.3: search for GC references inside other reachable
        // GC references.
        let mut count = 0;
        loop {
            match grey_list.pop_opt() {
                Some((low, high)) => {
                    count += 1;
                    mark_words_between(gc_ptrs, &mut grey_list,
                                       low as *uint, high as *uint);
                }
                // everything scanned
                None => break
            }
        }

        // Step 3. sweep all the unreachable ones for deallocation.
        let unreachable = gc_ptrs.find_unreachable();
        for &(ptr, finaliser) in unreachable.iter() {
            debug!("freeing {:x}", ptr);

            match finaliser {
                Some(f) => f(ptr as *mut ()),
                None => {}
            }
            gc_ptrs.remove(ptr);
            libc::free(ptr as *libc::c_void);
        }

        info!("GC scan: {} dead, {} live, {} scanned: took <unsupported> ms",
               unreachable.len(), gc_ptrs.len(), count);
    }
}

impl Drop for GarbageCollector {
    fn drop(&mut self) {
        // free all the pointers we're controlling.
        for (ptr, descr) in self.gc_ptrs.iter() {
            match descr.finaliser {
                Some(f) => f(ptr as *mut ()),
                None => {}
            }
            unsafe {libc::free(ptr as *libc::c_void)}
        }
    }
}

/// Scan the words from `low` to `high`, conservatively registering
/// any GC pointer bit patterns found.
unsafe fn mark_words_between(gc_ptrs: &mut PtrMap, grey_list: &mut ~[(uint, uint)],
                             mut low: *uint, high: *uint) {
    debug!("scanning from {} to {}", low, high);
    while low < high {
        match gc_ptrs.mark_reachable_scan_info(*low) {
            Some((top, scan)) => {
                debug!("found {:x} at {:x}", *low, low as uint);
                if scan {
                    grey_list.push((*low, top));
                }
            }
            None => {}
        }

        low = low.offset(1);
    }
}

// cargo culted from Boehm.
#[inline(never)]
fn clear_registers(_: uint, _: uint, _: uint,
                   _: uint, _: uint, _: uint) {}
