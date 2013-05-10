// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use cast;
use libc;
use option::*;
use task;
use task::atomically;
use unstable::finally::Finally;
use unstable::intrinsics;
use ops::Drop;
use clone::Clone;
use kinds::Owned;

/****************************************************************************
 * Shared state & exclusive ARC
 ****************************************************************************/

struct ArcData<T> {
    count:     libc::intptr_t,
    // FIXME(#3224) should be able to make this non-option to save memory
    data:      Option<T>,
}

struct ArcDestruct<T> {
    data: *libc::c_void,
}

#[unsafe_destructor]
impl<T> Drop for ArcDestruct<T>{
    fn finalize(&self) {
        unsafe {
            do task::unkillable {
                let mut data: ~ArcData<T> = cast::transmute(self.data);
                let new_count =
                    intrinsics::atomic_xsub(&mut data.count, 1) - 1;
                assert!(new_count >= 0);
                if new_count == 0 {
                    // drop glue takes over.
                } else {
                    cast::forget(data);
                }
            }
        }
    }
}

fn ArcDestruct<T>(data: *libc::c_void) -> ArcDestruct<T> {
    ArcDestruct {
        data: data
    }
}

/**
 * COMPLETELY UNSAFE. Used as a primitive for the safe versions in std::arc.
 *
 * Data races between tasks can result in crashes and, with sufficient
 * cleverness, arbitrary type coercion.
 */
pub type SharedMutableState<T> = ArcDestruct<T>;

pub unsafe fn shared_mutable_state<T:Owned>(data: T) ->
        SharedMutableState<T> {
    let data = ~ArcData { count: 1, data: Some(data) };
    let ptr = cast::transmute(data);
    ArcDestruct(ptr)
}

#[inline(always)]
pub unsafe fn get_shared_mutable_state<T:Owned>(
    rc: *SharedMutableState<T>) -> *mut T
{
    let ptr: ~ArcData<T> = cast::transmute((*rc).data);
    assert!(ptr.count > 0);
    let r = cast::transmute(ptr.data.get_ref());
    cast::forget(ptr);
    return r;
}
#[inline(always)]
pub unsafe fn get_shared_immutable_state<'a,T:Owned>(
        rc: &'a SharedMutableState<T>) -> &'a T {
    let ptr: ~ArcData<T> = cast::transmute((*rc).data);
    assert!(ptr.count > 0);
    // Cast us back into the correct region
    let r = cast::transmute_region(ptr.data.get_ref());
    cast::forget(ptr);
    return r;
}

pub unsafe fn clone_shared_mutable_state<T:Owned>(rc: &SharedMutableState<T>)
        -> SharedMutableState<T> {
    let mut ptr: ~ArcData<T> = cast::transmute((*rc).data);
    let new_count = intrinsics::atomic_xadd(&mut ptr.count, 1) + 1;
    assert!(new_count >= 2);
    cast::forget(ptr);
    ArcDestruct((*rc).data)
}

impl<T:Owned> Clone for SharedMutableState<T> {
    fn clone(&self) -> SharedMutableState<T> {
        unsafe {
            clone_shared_mutable_state(self)
        }
    }
}

/****************************************************************************/

#[allow(non_camel_case_types)] // runtime type
pub type rust_little_lock = *libc::c_void;

struct LittleLock {
    l: rust_little_lock,
}

impl Drop for LittleLock {
    fn finalize(&self) {
        unsafe {
            rust_destroy_little_lock(self.l);
        }
    }
}

fn LittleLock() -> LittleLock {
    unsafe {
        LittleLock {
            l: rust_create_little_lock()
        }
    }
}

pub impl LittleLock {
    #[inline(always)]
    unsafe fn lock<T>(&self, f: &fn() -> T) -> T {
        do atomically {
            rust_lock_little_lock(self.l);
            do (|| {
                f()
            }).finally {
                rust_unlock_little_lock(self.l);
            }
        }
    }
}

struct ExData<T> {
    lock: LittleLock,
    failed: bool,
    data: T,
}

/**
 * An arc over mutable data that is protected by a lock. For library use only.
 */
pub struct Exclusive<T> {
    x: SharedMutableState<ExData<T>>
}

pub fn exclusive<T:Owned>(user_data: T) -> Exclusive<T> {
    let data = ExData {
        lock: LittleLock(),
        failed: false,
        data: user_data
    };
    Exclusive {
        x: unsafe {
            shared_mutable_state(data)
        }
    }
}

impl<T:Owned> Clone for Exclusive<T> {
    // Duplicate an exclusive ARC, as std::arc::clone.
    fn clone(&self) -> Exclusive<T> {
        Exclusive { x: unsafe { clone_shared_mutable_state(&self.x) } }
    }
}

pub impl<T:Owned> Exclusive<T> {
    // Exactly like std::arc::mutex_arc,access(), but with the little_lock
    // instead of a proper mutex. Same reason for being unsafe.
    //
    // Currently, scheduling operations (i.e., yielding, receiving on a pipe,
    // accessing the provided condition variable) are prohibited while inside
    // the exclusive. Supporting that is a work in progress.
    #[inline(always)]
    unsafe fn with<U>(&self, f: &fn(x: &mut T) -> U) -> U {
        let rec = get_shared_mutable_state(&self.x);
        do (*rec).lock.lock {
            if (*rec).failed {
                fail!(
                    ~"Poisoned exclusive - another task failed inside!");
            }
            (*rec).failed = true;
            let result = f(&mut (*rec).data);
            (*rec).failed = false;
            result
        }
    }

    #[inline(always)]
    unsafe fn with_imm<U>(&self, f: &fn(x: &T) -> U) -> U {
        do self.with |x| {
            f(cast::transmute_immut(x))
        }
    }
}

fn compare_and_swap(address: &mut int, oldval: int, newval: int) -> bool {
    unsafe {
        let old = intrinsics::atomic_cxchg(address, oldval, newval);
        old == oldval
    }
}

extern {
    fn rust_create_little_lock() -> rust_little_lock;
    fn rust_destroy_little_lock(lock: rust_little_lock);
    fn rust_lock_little_lock(lock: rust_little_lock);
    fn rust_unlock_little_lock(lock: rust_little_lock);
}

#[cfg(test)]
mod tests {
    use comm;
    use super::exclusive;
    use task;
    use uint;

    #[test]
    fn exclusive_arc() {
        let mut futures = ~[];

        let num_tasks = 10;
        let count = 10;

        let total = exclusive(~0);

        for uint::range(0, num_tasks) |_i| {
            let total = total.clone();
            let (port, chan) = comm::stream();
            futures.push(port);

            do task::spawn || {
                for uint::range(0, count) |_i| {
                    do total.with |count| {
                        **count += 1;
                    }
                }
                chan.send(());
            }
        };

        for futures.each |f| { f.recv() }

        do total.with |total| {
            assert!(**total == num_tasks * count)
        };
    }

    #[test] #[should_fail] #[ignore(cfg(windows))]
    fn exclusive_poison() {
        // Tests that if one task fails inside of an exclusive, subsequent
        // accesses will also fail.
        let x = exclusive(1);
        let x2 = x.clone();
        do task::try || {
            do x2.with |one| {
                assert!(*one == 2);
            }
        };
        do x.with |one| {
            assert!(*one == 1);
        }
    }
}
