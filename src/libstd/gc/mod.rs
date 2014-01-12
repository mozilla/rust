// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/*! Task-local garbage-collected boxes

The `Gc` type provides shared ownership of an immutable value. Destruction is not deterministic, and
will occur some time between every `Gc` handle being gone and the end of the task. The garbage
collector is task-local so `Gc<T>` is not sendable.

*/

#[allow(missing_doc)];
#[allow(experimental)];

use kinds::Freeze;
use container::Container;
use clone::{Clone, DeepClone};
use iter::Iterator;
use mem;
use option::{Some, None};
use ptr;
use ptr::RawPtr;
use rt::local;
use rt::task::{Task, GcUninit, GcExists, GcBorrowed};
use util::replace;
use vec::ImmutableVector;

use unstable::intrinsics::{reaches_new_managed, move_val_init, needs_drop};

use gc::collector::GarbageCollector;

pub mod collector;

fn pointer_run_dtor<T>(p: *mut ()) {
    unsafe {
        ptr::read_ptr(p as *T);
    }
}
fn pointer_trace<T: Trace>(p: *(), _: uint, tracer: &mut GcTracer) {
    unsafe {
        (*(p as *T)).trace(tracer)
    }
}

/// Possibly register the changes to the GC roots described by the
/// arguments.
///
/// - `removals` contains the beginning of memory regions that were
///   (possibly) previously registered as GC roots. These pointers do
///   not have to have previously been registered nor do they even
///   have to be valid pointers.
/// - `additions` contains the beginning and length of memory regions
///   to register as new GC roots (including ones that are already
///   registered but now have a different length)
///
/// The elements of `removals` are removed before `additions` are
/// added.
///
/// The registration only occurs if `T` actually does have the
/// possibility to contain `Gc<T>` (determined statically). Pointers
/// passed in `additions` should be removed as roots just before they
/// are deallocated or otherwise become invalid.
#[inline]
pub unsafe fn register_root_changes<T>(removals: &[*T],
                                       additions: &[(*T, uint, collector::TracingFunc)]) {
    if reaches_new_managed::<T>() {
        register_root_changes_always::<T>(removals, additions)
    }
}

/// Unconditionally perform the registration and unregistration of GC
/// roots, ignoring type information.
///
/// See the conditional but otherwise identical
/// `register_root_changes` for description.
pub unsafe fn register_root_changes_always<T>(removals: &[*T],
                                              additions: &[(*T, uint, collector::TracingFunc)]) {
    let mut gc = {
        let mut task = local::Local::borrow(None::<Task>);

        // we don't execute any external code inside here and
        // everything is task local, so Uninit is fine (because
        // nothing else will read it)
        replace(&mut task.get().gc, GcUninit)
    };
    match gc {
        // first GC interaction in this task, so create a new
        // collector
        GcUninit => {
            if additions.len() != 0 {
                // we need to add some roots, and we need a GC for
                // that.
                gc = GcExists(~GarbageCollector::new())
            } else {
                // we are only removing things, and if the GC doesn't
                // exist, the pointers are already not registered as
                // roots.
                return
            }
        }
        // the task is cleaning up, so registering root changes would
        // be pointless.
        GcBorrowed(ptr) if ptr.is_null() => return,
        _ => {}
    }
    {
        let gc_ref = match gc {
            GcUninit => unreachable!(),
            GcExists(ref mut gc) => &mut **gc,
            // you might wonder why we can do this safely. We hit this
            // code path when a collection runs a finaliser that
            // wishes to change any roots (usually deregistering a
            // root). Finalisers run after all the scanning, and we
            // don't touch the root information data structure while
            // running them, so we're fine to modify it.
            //
            // (if `unsafe_gc` is null we'd've already returned from
            // the check above)
            GcBorrowed(unsafe_gc) => &mut *unsafe_gc
        };

        for ptr in removals.iter() {
            debug!("unregistering {}", *ptr);
            gc_ref.nongc_unregister(*ptr as *());
        }
        for &(ptr, metadata, tracer) in additions.iter() {
            debug!("registering {} {}", ptr, metadata);
            gc_ref.nongc_register(ptr as *(), metadata, tracer)
        }
    }

    let mut task = local::Local::borrow(None::<Task>);
    task.get().gc = gc;
}

pub unsafe fn update_metadata<T>(ptr: *T, metadata: uint) {
    if reaches_new_managed::<T>() {
        update_metadata_always(ptr as *(), metadata)
    }
}

pub unsafe fn update_metadata_always(ptr: *(), metadata: uint) {
    debug!("update_metadata_always: setting {} MD to {}", ptr, metadata);
    let mut gc = {
        let mut task = local::Local::borrow(None::<Task>);

        // we don't execute any external code inside here and
        // everything is task local, so Uninit is fine (because
        // nothing else will read it)
        replace(&mut task.get().gc, GcUninit)
    };
    match gc {
        // can't update the metadata of a bad_pointer
        GcUninit => return,
        // the task is cleaning up, so registering root changes would
        // be pointless.
        GcBorrowed(ptr) if ptr.is_null() => return,
        _ => {}
    }
    {
        let gc_ref = match gc {
            GcUninit => unreachable!(),
            GcExists(ref mut gc) => &mut **gc,
            // you might wonder why we can do this safely. We hit this
            // code path when a collection runs a finaliser that
            // wishes to change any roots (usually deregistering a
            // root). Finalisers run after all the scanning, and we
            // don't touch the root information data structure while
            // running them, so we're fine to modify it.
            //
            // (if `unsafe_gc` is null we'd've already returned from
            // the check above)
            GcBorrowed(unsafe_gc) => &mut *unsafe_gc
        };
        gc_ref.nongc_update_metadata(ptr, metadata);
    }

    let mut task = local::Local::borrow(None::<Task>);
    task.get().gc = gc;
}

/// Immutable garbage-collected pointer type.
///
/// # Warning
///
/// This is highly experimental. Placing them in the wrong place can
/// cause live pointers to be deallocated or reused. Wrong places
/// can include:
///
/// - global variables (including `#[thread_local]` ones)
/// - task-local storage
/// - both built-in allocating pointer types (`~` and `@`)
/// - both built-in allocating vector types (`~[]` and `@[]`)
/// - most library smart pointers, like `Rc`
#[no_send]
#[experimental]
#[managed]
pub struct Gc<T> {
    priv ptr: *T
}

impl<T: 'static + Trace> Gc<T> {
    /// Construct a new garbage-collected box
    #[experimental="not rooted by built-in pointer and vector types"]
    pub fn new(value: T) -> Gc<T> {
        let stack_top;
        let mut gc;
        {
            // we need the task-local GC, and some upper bound on the
            // top of the stack. The borrow is scoped so that we can
            // use task things like logging etc. inside .collect() and
            // (as much as possible) inside finalisers.
            let mut task_ = local::Local::borrow(None::<Task>);
            let task = task_.get();

            match task.stack_bounds() {
                (_, t) => stack_top = t,
            }

            // some contortions to put a *mut GC reference back into
            // the task if we're OK to go (i.e. not borrowed already)
            // but we may need to construct a new GC and failure is
            // not possible (task is borrowed) so... options.
            gc = match replace(&mut task.gc, GcUninit) {
                // we can't Gc::new while a collection is going on.
                GcBorrowed(_) => None,
                GcExists(gc) => Some(gc),
                GcUninit => Some(~GarbageCollector::new())
            };
            match gc {
                // `gc` is behind a ~ pointer, so it doesn't move and
                // this raw pointer will be valid until task death.
                Some(ref mut gc) => { task.gc = GcBorrowed(&mut **gc as *mut GarbageCollector) }
                None => {}
            }
        }

        let mut gc = match gc {
            // the task is unborrowed, so now we can fail!
            None => fail!("Gc::new: Gc already borrowed."),
            Some(gc) => gc,
        };

        let size = mem::size_of::<T>();
        let ptr;
        unsafe {
            gc.occasional_collection(stack_top);

            let finaliser = if needs_drop::<T>() {
                Some(pointer_run_dtor::<T>)
            } else {
                None
            };

            // if we don't contain anything that contains has a
            // #[managed] pointer unboxed, then we don't don't need to
            // scan, because there can never be a GC reference inside.
            // FIXME: we currently count ~Gc<int> as owning managed,
            // but it shouldn't (~, or equivalent) should root the Gc
            // itself.
            ptr = gc.alloc(size, Some(pointer_trace::<T>), finaliser) as *mut T;
            debug!("Gc::new: alloc'd {}", ptr);
            move_val_init(&mut *ptr, value);
        }

        // restore the garbage collector to the task.
        let mut task = local::Local::borrow(None::<Task>);
        task.get().gc = GcExists(gc);

        Gc { ptr: ptr as *T }
    }
}

impl<T: 'static + Freeze> Gc<T> {
    /// Borrow the value contained in the garbage-collected box.
    ///
    /// This is restricted to deeply immutable values, and so does not
    /// require a write-barrier because no writes are possible.
    ///
    /// Currently `unsafe` because `~` and `~[]` do not root a `Gc<T>`
    /// box, and so, if that is the only reference to one, then that
    /// `Gc<T>` may be deallocated or the memory reused.
    #[inline]
    pub unsafe fn borrow<'r>(&'r self) -> &'r T {
        &*self.ptr
    }
}

impl<T: 'static> Gc<T> {
    /// Borrow the value contained in the garbage-collected box, with
    /// a write-barrier.
    ///
    /// See `.borrow()` for the reason for `unsafe`.
    #[inline]
    pub unsafe fn borrow_write_barrier<'r>(&'r self) -> &'r T {
        // a completely conservative non-generational GC needs no
        // write barriers.
        &*self.ptr
    }

    /// Borrow the value contained in the garbage-collected box,
    /// without a write-barrier.
    ///
    /// Because this has no write barrier, any writes to the value
    /// must not write new references to other garbage-collected box.
    #[inline]
    pub unsafe fn borrow_no_write_barrier<'r>(&'r self) -> &'r T {
        // a completely conservative non-generational GC needs no
        // write barriers.
        &*self.ptr
    }
}

impl<T> Clone for Gc<T> {
    fn clone(&self) -> Gc<T> { *self }
}

/// The `Freeze` bound restricts this to acyclic graphs where it is well-defined.
///
/// A `Send` bound would also work, but `Send` *or* `Freeze` cannot be expressed.
impl<T: DeepClone + 'static + Trace> DeepClone for Gc<T> {
    #[inline]
    fn deep_clone(&self) -> Gc<T> {
        Gc::new(unsafe {self.borrow_write_barrier().deep_clone()})
    }
}

/// Stores the appropriate tools for interacting with the garbage
/// collector while tracing.
pub struct GcTracer<'a> {
    priv gc: &'a mut collector::GarbageCollector
}

impl<'a> GcTracer<'a> {
    /// Returns true on the first call for a given value of `ptr`,
    /// which is a "pointer" to some non-GC'd memory region that has
    /// been previously registered with the GC.
    pub fn pointer_first_trace(&mut self, ptr: *()) -> bool {
        self.gc.nongc_first_trace(ptr)
    }

    pub unsafe fn conservative_scan(&mut self, start: *uint, end: *uint) {
        self.gc.conservative_scan(start, end)
    }
}

/// Give information to the GC about precisely what piece of memory
/// contain `Gc<T>` pointers.
pub trait Trace {
    /// Trace the value, marking any reachable `Gc<T>` pointers.
    fn trace(&self, tracer: &mut GcTracer);
}

impl<T: Trace> Trace for Gc<T> {
    fn trace(&self, tracer: &mut GcTracer) {
        if tracer.gc.gc_first_trace(self.ptr as *()) {
            debug!("Gc.trace inner: {}", self.ptr);
            // only both tracing if we can get to a Gc pointer.
            unsafe {
                if reaches_new_managed::<T>() {
                    (*self.ptr).trace(tracer)
                }
            }
        }
    }
}

/// Things that definitely don't contain any GC'd pointers.
macro_rules! no_gc {
    ($($t:ty),*) => {
        $(
            impl Trace for $t { #[inline(always)] fn trace(&self, _: &mut GcTracer) {} }
            )*
    }
}

no_gc! {
    bool,
    int, i8, i16, i32, i64,
    uint, u8, u16, u32, u64,
    f32, f64,
    ~str
}
impl<'a> Trace for &'a str { #[inline(always)] fn trace(&self, _: &mut GcTracer) {} }


/// These should probably be compiler generated somehow...

// create tracing impls for tuples.
macro_rules! trace_tuple {
    () => { impl Trace for () { #[inline(always)] fn trace(&self, _: &mut GcTracer) {} } };

    // we "abuse" `ident`s, so we can reuse them as both the type
    // parameter and the variable name.
    ($head:ident $(, $rest:ident)*) => {
        // create an impl with all our arguments
        impl<$head: Trace $(, $rest: Trace )*> Trace for ($head, $($rest),*) {
            fn trace(&self, tracer: &mut GcTracer) {
                let (ref $head, $( ref $rest ),*) = *self;
                $head.trace(tracer);
                $( $rest.trace(tracer); )*
            }
        }

        // recursively generate the shorter tuple impls
        trace_tuple!($($rest),*)
    }
}
trace_tuple! {
     _1,  _2,  _3,  _4,  _5,  _6,  _7,  _8,  _9, _10,
    _11, _12, _13, _14, _15, _16, _17, _18, _19, _20,
    _21, _22, _23, _24, _25, _26, _27, _28, _29, _30,
    _31, _32, _33, _34, _35, _36, _37, _38, _39, _40
}

macro_rules! trace_fixed_vec {
    ($($e: expr),*) => {
        $(
            impl<T: Trace> Trace for [T, .. $e] {
                fn trace(&self, tracer: &mut GcTracer) {
                    for v in self.iter() {
                        v.trace(tracer)
                    }
                }
            }
            )*
    }
}
trace_fixed_vec! {
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10,
    100, 1000, 10000, 100000
}

#[cfg(test)]
mod tests {
    use super::*;
    use cell::RefCell;
    use iter::Iterator;
    use option::{Some, None};
    use vec::{ImmutableVector, MutableVector};

    #[test]
    fn test_clone() {
        let x = Gc::new(RefCell::new(5));
        let y = x.clone();
        unsafe {
            x.borrow_write_barrier().with_mut(|inner| {
                    *inner = 20;
                });
            assert_eq!(y.borrow_write_barrier().with(|x| *x), 20);
        }
    }

    #[test]
    fn test_deep_clone() {
        let x = Gc::new(RefCell::new(5));
        let y = x.deep_clone();
        unsafe {
            x.borrow_write_barrier().with_mut(|inner| {
                    *inner = 20;
                });
            assert_eq!(y.borrow_write_barrier().with(|x| *x), 5);
        }
    }

    #[test]
    fn test_simple() {
        let x = Gc::new(5);
        unsafe {
            assert_eq!(*x.borrow(), 5);
        }
    }

    #[test]
    fn test_simple_clone() {
        let x = Gc::new(5);
        let y = x.clone();
        unsafe {
            assert_eq!(*x.borrow(), 5);
            assert_eq!(*y.borrow(), 5);
        }
    }

    #[test]
    fn test_destructor() {
        let x = Gc::new(~5);
        unsafe {
            assert_eq!(**x.borrow(), 5);
        }
    }

    #[test]
    fn test_many_allocs() {
        // on the stack.
        let mut ptrs = [None::<Gc<uint>>, .. 10000];
        for (i, ptr) in ptrs.mut_iter().enumerate() {
            *ptr = Some(Gc::new(i))
        }

        for (i, ptr) in ptrs.iter().enumerate() {
            unsafe {
                let p = ptr.unwrap();
                assert_eq!(*p.borrow(), i);
            }
        }
    }
}

#[cfg(test)]
mod bench {
    use super::*;
    use iter::Iterator;
    use option::{Some, None};
    use vec::{ImmutableVector, MutableVector};
    use extra::test::BenchHarness;

    #[bench]
    fn many_allocs(bh: &mut BenchHarness) {
        bh.iter(|| {
                let mut ptrs = [None::<Gc<uint>>, .. 1000];
                for (i, ptr) in ptrs.mut_iter().enumerate() {
                    *ptr = Some(Gc::new(i))
                }
            })
    }
}
