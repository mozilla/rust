// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/*! Task-local reference-counted boxes (`Rc` type)

The `Rc` type provides shared ownership of an immutable value. Destruction is deterministic, and
will occur as soon as the last owner is gone. It is marked as non-sendable because it avoids the
overhead of atomic reference counting.

The `downgrade` method can be used to create a non-owning `Weak` pointer to the box. A `Weak`
pointer can be upgraded to an `Rc` pointer, but will return `None` if the value has already been
freed.

For example, a tree with parent pointers can be represented by putting the nodes behind `Strong`
pointers, and then storing the parent pointers as `Weak` pointers.

*/

use cast::transmute;
use ops::Drop;
use cmp::{Eq, Ord};
use clone::{Clone, DeepClone};
use ptr::read_ptr;
use option::{Option, Some, None};

struct RcBox<T> {
    value: T,
    strong: uint,
    weak: uint
}

/// Immutable reference counted pointer type
#[unsafe_no_drop_flag]
#[no_send]
pub struct Rc<T> {
    priv ptr: *mut RcBox<T>
}

impl<T> Rc<T> {
    /// Construct a new reference-counted box
    pub fn new(value: T) -> Rc<T> {
        unsafe {
            Rc { ptr: transmute(~RcBox { value: value, strong: 1, weak: 0 }) }
        }
    }
}

impl<T> Rc<T> {
    /// Borrow the value contained in the reference-counted box
    #[inline(always)]
    pub fn borrow<'a>(&'a self) -> &'a T {
        &self.inner().value
    }

    /// Downgrade the reference-counted pointer to a weak reference
    pub fn downgrade(&self) -> Weak<T> {
        self.inner().weak += 1;
        Weak { ptr: self.ptr }
    }

    fn inner<'a>(&'a self) -> &'a mut RcBox<T> {
        // Note that we need the indrection of &~RcBox<T> because we can't
        // transmute *RcBox to &RcBox (the actual pointer layout is different if
        // T contains managed pointers at this time)
        let ptr: &mut ~RcBox<T> = unsafe { transmute(&self.ptr) };
        &mut **ptr
    }
}

#[unsafe_destructor]
impl<T> Drop for Rc<T> {
    fn drop(&mut self) {
        if self.ptr == 0 as *mut RcBox<T> { return }

        let inner = self.inner();
        inner.strong -= 1;
        if inner.strong == 0 {
            // If we've run out of strong pointers, we need to be sure to run
            // the destructor *now*, but we can't free the value just yet (weak
            // pointers may still be active).
            unsafe { read_ptr(&inner.value); } // destroy the contained object
            if inner.weak == 0 {
                free(self.ptr);
            }
        }
    }
}

impl<T> Clone for Rc<T> {
    #[inline]
    fn clone(&self) -> Rc<T> {
        self.inner().strong += 1;
        Rc { ptr: self.ptr }
    }
}

impl<T: DeepClone> DeepClone for Rc<T> {
    #[inline]
    fn deep_clone(&self) -> Rc<T> {
        Rc::new(self.borrow().deep_clone())
    }
}

impl<T: Eq> Eq for Rc<T> {
    #[inline(always)]
    fn eq(&self, other: &Rc<T>) -> bool { *self.borrow() == *other.borrow() }

    #[inline(always)]
    fn ne(&self, other: &Rc<T>) -> bool { *self.borrow() != *other.borrow() }
}

impl<T: Ord> Ord for Rc<T> {
    #[inline(always)]
    fn lt(&self, other: &Rc<T>) -> bool { *self.borrow() < *other.borrow() }

    #[inline(always)]
    fn le(&self, other: &Rc<T>) -> bool { *self.borrow() <= *other.borrow() }

    #[inline(always)]
    fn gt(&self, other: &Rc<T>) -> bool { *self.borrow() > *other.borrow() }

    #[inline(always)]
    fn ge(&self, other: &Rc<T>) -> bool { *self.borrow() >= *other.borrow() }
}

/// Weak reference to a reference-counted box
#[unsafe_no_drop_flag]
#[no_send]
pub struct Weak<T> {
    priv ptr: *mut RcBox<T>
}

impl<T> Weak<T> {
    /// Upgrade a weak reference to a strong reference
    pub fn upgrade(&self) -> Option<Rc<T>> {
        if self.inner().strong == 0 {
            None
        } else {
            self.inner().strong += 1;
            Some(Rc { ptr: self.ptr })
        }
    }

    fn inner<'a>(&'a self) -> &'a mut RcBox<T> {
        // see above version
        let ptr: &mut ~RcBox<T> = unsafe { transmute(&self.ptr) };
        &mut **ptr
    }
}

#[unsafe_destructor]
impl<T> Drop for Weak<T> {
    fn drop(&mut self) {
        if self.ptr as uint == 0 { return }

        let inner = self.inner();
        inner.weak -= 1;
        if inner.weak == 0 && inner.strong == 0 {
            free(self.ptr);
        }
    }
}

impl<T> Clone for Weak<T> {
    #[inline]
    fn clone(&self) -> Weak<T> {
        self.inner().weak += 1;
        Weak { ptr: self.ptr }
    }
}

// We need to be very careful when dropping this inner box pointer. We don't
// necessarily know the right free function to call because it's different
// depending on whether T contains managed pointers or not. The other sticky
// part is that we can't run the destructor for T because it's already been run.
//
// To get around this, we transmute the pointer to an owned pointer with a type
// that has size 0. This preserves the managed-ness of the type along with
// preventing any destructors from being run. Note that this assumes that the GC
// doesn't need to know the real size of the pointer (it's just malloc right
// now), so this works for now but may need to change in the future.
fn free<T>(ptr: *mut RcBox<T>) {
    let _: ~RcBox<[T, ..0]> = unsafe { transmute(ptr) };
}

#[cfg(test)]
mod tests {
    use prelude::*;
    use super::*;
    use cell::RefCell;

    #[test]
    fn test_clone() {
        let x = Rc::new(RefCell::new(5));
        let y = x.clone();
        x.borrow().with_mut(|inner| {
            *inner = 20;
        });
        assert_eq!(y.borrow().with(|v| *v), 20);
    }

    #[test]
    fn test_deep_clone() {
        let x = Rc::new(RefCell::new(5));
        let y = x.deep_clone();
        x.borrow().with_mut(|inner| {
            *inner = 20;
        });
        assert_eq!(y.borrow().with(|v| *v), 5);
    }

    #[test]
    fn test_simple() {
        let x = Rc::new(5);
        assert_eq!(*x.borrow(), 5);
    }

    #[test]
    fn test_simple_clone() {
        let x = Rc::new(5);
        let y = x.clone();
        assert_eq!(*x.borrow(), 5);
        assert_eq!(*y.borrow(), 5);
    }

    #[test]
    fn test_destructor() {
        let x = Rc::new(~5);
        assert_eq!(**x.borrow(), 5);
    }

    #[test]
    fn test_live() {
        let x = Rc::new(5);
        let y = x.downgrade();
        assert!(y.upgrade().is_some());
    }

    #[test]
    fn test_dead() {
        let x = Rc::new(5);
        let y = x.downgrade();
        drop(x);
        assert!(y.upgrade().is_none());
    }

    #[test]
    fn gc_inside() {
        // see issue #11532
        use {cell, gc};
        let a = Rc::new(cell::RefCell::new(gc::Gc::new(1)));
        assert!(a.borrow().try_borrow_mut().is_some());
    }
}
