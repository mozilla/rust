// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Generic hashing support.
//!
//! This module provides a generic way to compute the hash of a value. The
//! simplest way to make a type hashable is to use `#[derive(Hash)]`:
//!
//! # Examples
//!
//! ```rust
//! use std::hash::{hash, Hash, SipHasher};
//!
//! #[derive(Hash)]
//! struct Person {
//!     id: uint,
//!     name: String,
//!     phone: u64,
//! }
//!
//! let person1 = Person { id: 5, name: "Janet".to_string(), phone: 555_666_7777 };
//! let person2 = Person { id: 5, name: "Bob".to_string(), phone: 555_666_7777 };
//!
//! assert!(hash::<_, SipHasher>(&person1) != hash::<_, SipHasher>(&person2));
//! ```
//!
//! If you need more control over how a value is hashed, you need to implement
//! the trait `Hash`:
//!
//! ```rust
//! use std::hash::{hash, Hash, Hasher, SipHasher};
//!
//! struct Person {
//!     id: uint,
//!     name: String,
//!     phone: u64,
//! }
//!
//! impl Hash for Person {
//!     fn hash<H: Hasher>(&self, state: &mut H) {
//!         self.id.hash(state);
//!         self.phone.hash(state);
//!     }
//! }
//!
//! let person1 = Person { id: 5, name: "Janet".to_string(), phone: 555_666_7777 };
//! let person2 = Person { id: 5, name: "Bob".to_string(), phone: 555_666_7777 };
//!
//! assert_eq!(hash::<_, SipHasher>(&person1), hash::<_, SipHasher>(&person2));
//! ```

#![stable(feature = "rust1", since = "1.0.0")]

use prelude::*;

use default::Default;
use mem;

pub use self::sip::SipHasher;

mod sip;

/// A hashable type.
///
/// The `H` type parameter is an abstract hash state that is used by the `Hash`
/// to compute the hash. Specific implementations of this trait may specialize
/// for particular instances of `H` in order to be able to optimize the hashing
/// behavior.
#[cfg(not(stage0))]
#[stable(feature = "rust1", since = "1.0.0")]
pub trait Hash {
    /// Feeds this value into the state given, updating the hasher as necessary.
    #[stable(feature = "rust1", since = "1.0.0")]
    fn hash<H: Hasher>(&self, state: &mut H);

    /// Feeds a slice of this type into the state provided.
    #[unstable(feature = "hash", reason = "module was recently redesigned")]
    fn hash_slice<H: Hasher>(data: &[Self], state: &mut H) where Self: Sized {
        for piece in data {
            piece.hash(state);
        }
    }
}

/// A hashable type.
///
/// The `H` type parameter is an abstract hash state that is used by the `Hash`
/// to compute the hash. Specific implementations of this trait may specialize
/// for particular instances of `H` in order to be able to optimize the hashing
/// behavior.
#[cfg(stage0)]
pub trait Hash<H: Hasher> {
    /// Feeds this value into the state given, updating the hasher as necessary.
    fn hash(&self, state: &mut H);
}

/// A trait which represents the ability to hash an arbitrary stream of bytes.
#[stable(feature = "rust1", since = "1.0.0")]
pub trait Hasher {
    /// Result type of one run of hashing generated by this hasher.
    #[cfg(stage0)]
    type Output;

    /// Resets this hasher back to its initial state (as if it were just
    /// created).
    #[cfg(stage0)]
    fn reset(&mut self);

    /// Completes a round of hashing, producing the output hash generated.
    #[cfg(stage0)]
    fn finish(&self) -> Self::Output;

    /// Completes a round of hashing, producing the output hash generated.
    #[cfg(not(stage0))]
    #[unstable(feature = "hash", reason = "module was recently redesigned")]
    fn finish(&self) -> u64;

    /// Writes some data into this `Hasher`
    #[cfg(not(stage0))]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn write(&mut self, bytes: &[u8]);

    /// Write a single `u8` into this hasher
    #[cfg(not(stage0))]
    #[inline]
    #[unstable(feature = "hash", reason = "module was recently redesigned")]
    fn write_u8(&mut self, i: u8) { self.write(&[i]) }
    /// Write a single `u16` into this hasher.
    #[cfg(not(stage0))]
    #[inline]
    #[unstable(feature = "hash", reason = "module was recently redesigned")]
    fn write_u16(&mut self, i: u16) {
        self.write(&unsafe { mem::transmute::<_, [u8; 2]>(i) })
    }
    /// Write a single `u32` into this hasher.
    #[cfg(not(stage0))]
    #[inline]
    #[unstable(feature = "hash", reason = "module was recently redesigned")]
    fn write_u32(&mut self, i: u32) {
        self.write(&unsafe { mem::transmute::<_, [u8; 4]>(i) })
    }
    /// Write a single `u64` into this hasher.
    #[cfg(not(stage0))]
    #[inline]
    #[unstable(feature = "hash", reason = "module was recently redesigned")]
    fn write_u64(&mut self, i: u64) {
        self.write(&unsafe { mem::transmute::<_, [u8; 8]>(i) })
    }
    /// Write a single `usize` into this hasher.
    #[cfg(not(stage0))]
    #[inline]
    #[unstable(feature = "hash", reason = "module was recently redesigned")]
    fn write_usize(&mut self, i: usize) {
        if cfg!(target_pointer_size = "32") {
            self.write_u32(i as u32)
        } else {
            self.write_u64(i as u64)
        }
    }

    /// Write a single `i8` into this hasher.
    #[cfg(not(stage0))]
    #[inline]
    #[unstable(feature = "hash", reason = "module was recently redesigned")]
    fn write_i8(&mut self, i: i8) { self.write_u8(i as u8) }
    /// Write a single `i16` into this hasher.
    #[cfg(not(stage0))]
    #[inline]
    #[unstable(feature = "hash", reason = "module was recently redesigned")]
    fn write_i16(&mut self, i: i16) { self.write_u16(i as u16) }
    /// Write a single `i32` into this hasher.
    #[cfg(not(stage0))]
    #[inline]
    #[unstable(feature = "hash", reason = "module was recently redesigned")]
    fn write_i32(&mut self, i: i32) { self.write_u32(i as u32) }
    /// Write a single `i64` into this hasher.
    #[cfg(not(stage0))]
    #[inline]
    #[unstable(feature = "hash", reason = "module was recently redesigned")]
    fn write_i64(&mut self, i: i64) { self.write_u64(i as u64) }
    /// Write a single `isize` into this hasher.
    #[cfg(not(stage0))]
    #[inline]
    #[unstable(feature = "hash", reason = "module was recently redesigned")]
    fn write_isize(&mut self, i: isize) { self.write_usize(i as usize) }
}

/// A common bound on the `Hasher` parameter to `Hash` implementations in order
/// to generically hash an aggregate.
#[unstable(feature = "hash",
           reason = "this trait will likely be replaced by io::Writer")]
#[allow(missing_docs)]
#[cfg(stage0)]
pub trait Writer {
    fn write(&mut self, bytes: &[u8]);
}

/// Hash a value with the default SipHasher algorithm (two initial keys of 0).
///
/// The specified value will be hashed with this hasher and then the resulting
/// hash will be returned.
#[cfg(stage0)]
pub fn hash<T: Hash<H>, H: Hasher + Default>(value: &T) -> H::Output {
    let mut h: H = Default::default();
    value.hash(&mut h);
    h.finish()
}

/// Hash a value with the default SipHasher algorithm (two initial keys of 0).
///
/// The specified value will be hashed with this hasher and then the resulting
/// hash will be returned.
#[cfg(not(stage0))]
#[unstable(feature = "hash", reason = "module was recently redesigned")]
pub fn hash<T: Hash, H: Hasher + Default>(value: &T) -> u64 {
    let mut h: H = Default::default();
    value.hash(&mut h);
    h.finish()
}

//////////////////////////////////////////////////////////////////////////////

#[cfg(stage0)]
mod impls {
    use prelude::*;

    use mem;
    use num::Int;
    use super::*;

    macro_rules! impl_hash {
        ($ty:ident, $uty:ident) => {
            impl<S: Writer + Hasher> Hash<S> for $ty {
                #[inline]
                fn hash(&self, state: &mut S) {
                    let a: [u8; ::$ty::BYTES] = unsafe {
                        mem::transmute((*self as $uty).to_le() as $ty)
                    };
                    state.write(&a)
                }
            }
        }
    }

    impl_hash! { u8, u8 }
    impl_hash! { u16, u16 }
    impl_hash! { u32, u32 }
    impl_hash! { u64, u64 }
    impl_hash! { uint, uint }
    impl_hash! { i8, u8 }
    impl_hash! { i16, u16 }
    impl_hash! { i32, u32 }
    impl_hash! { i64, u64 }
    impl_hash! { int, uint }

    impl<S: Writer + Hasher> Hash<S> for bool {
        #[inline]
        fn hash(&self, state: &mut S) {
            (*self as u8).hash(state);
        }
    }

    impl<S: Writer + Hasher> Hash<S> for char {
        #[inline]
        fn hash(&self, state: &mut S) {
            (*self as u32).hash(state);
        }
    }

    impl<S: Writer + Hasher> Hash<S> for str {
        #[inline]
        fn hash(&self, state: &mut S) {
            state.write(self.as_bytes());
            0xffu8.hash(state)
        }
    }

    macro_rules! impl_hash_tuple {
        () => (
            impl<S: Hasher> Hash<S> for () {
                #[inline]
                fn hash(&self, _state: &mut S) {}
            }
        );

        ( $($name:ident)+) => (
            impl<S: Hasher, $($name: Hash<S>),*> Hash<S> for ($($name,)*) {
                #[inline]
                #[allow(non_snake_case)]
                fn hash(&self, state: &mut S) {
                    match *self {
                        ($(ref $name,)*) => {
                            $(
                                $name.hash(state);
                            )*
                        }
                    }
                }
            }
        );
    }

    impl_hash_tuple! {}
    impl_hash_tuple! { A }
    impl_hash_tuple! { A B }
    impl_hash_tuple! { A B C }
    impl_hash_tuple! { A B C D }
    impl_hash_tuple! { A B C D E }
    impl_hash_tuple! { A B C D E F }
    impl_hash_tuple! { A B C D E F G }
    impl_hash_tuple! { A B C D E F G H }
    impl_hash_tuple! { A B C D E F G H I }
    impl_hash_tuple! { A B C D E F G H I J }
    impl_hash_tuple! { A B C D E F G H I J K }
    impl_hash_tuple! { A B C D E F G H I J K L }

    impl<S: Writer + Hasher, T: Hash<S>> Hash<S> for [T] {
        #[inline]
        fn hash(&self, state: &mut S) {
            self.len().hash(state);
            for elt in self {
                elt.hash(state);
            }
        }
    }


    impl<'a, S: Hasher, T: ?Sized + Hash<S>> Hash<S> for &'a T {
        #[inline]
        fn hash(&self, state: &mut S) {
            (**self).hash(state);
        }
    }

    impl<'a, S: Hasher, T: ?Sized + Hash<S>> Hash<S> for &'a mut T {
        #[inline]
        fn hash(&self, state: &mut S) {
            (**self).hash(state);
        }
    }

    impl<S: Writer + Hasher, T> Hash<S> for *const T {
        #[inline]
        fn hash(&self, state: &mut S) {
            // NB: raw-pointer Hash does _not_ dereference
            // to the target; it just gives you the pointer-bytes.
            (*self as uint).hash(state);
        }
    }

    impl<S: Writer + Hasher, T> Hash<S> for *mut T {
        #[inline]
        fn hash(&self, state: &mut S) {
            // NB: raw-pointer Hash does _not_ dereference
            // to the target; it just gives you the pointer-bytes.
            (*self as uint).hash(state);
        }
    }
}

#[cfg(not(stage0))]
mod impls {
    use prelude::*;

    use slice;
    use super::*;

    macro_rules! impl_write {
        ($(($ty:ident, $meth:ident),)*) => {$(
            #[stable(feature = "rust1", since = "1.0.0")]
            impl Hash for $ty {
                fn hash<H: Hasher>(&self, state: &mut H) {
                    state.$meth(*self)
                }

                fn hash_slice<H: Hasher>(data: &[$ty], state: &mut H) {
                    let newlen = data.len() * ::$ty::BYTES;
                    let ptr = data.as_ptr() as *const u8;
                    state.write(unsafe { slice::from_raw_parts(ptr, newlen) })
                }
            }
        )*}
    }

    impl_write! {
        (u8, write_u8),
        (u16, write_u16),
        (u32, write_u32),
        (u64, write_u64),
        (usize, write_usize),
        (i8, write_i8),
        (i16, write_i16),
        (i32, write_i32),
        (i64, write_i64),
        (isize, write_isize),
    }

    #[stable(feature = "rust1", since = "1.0.0")]
    impl Hash for bool {
        fn hash<H: Hasher>(&self, state: &mut H) {
            state.write_u8(*self as u8)
        }
    }

    #[stable(feature = "rust1", since = "1.0.0")]
    impl Hash for char {
        fn hash<H: Hasher>(&self, state: &mut H) {
            state.write_u32(*self as u32)
        }
    }

    #[stable(feature = "rust1", since = "1.0.0")]
    impl Hash for str {
        fn hash<H: Hasher>(&self, state: &mut H) {
            state.write(self.as_bytes());
            state.write_u8(0xff)
        }
    }

    macro_rules! impl_hash_tuple {
        () => (
            #[stable(feature = "rust1", since = "1.0.0")]
            impl Hash for () {
                fn hash<H: Hasher>(&self, _state: &mut H) {}
            }
        );

        ( $($name:ident)+) => (
            #[stable(feature = "rust1", since = "1.0.0")]
            impl<$($name: Hash),*> Hash for ($($name,)*) {
                #[allow(non_snake_case)]
                fn hash<S: Hasher>(&self, state: &mut S) {
                    let ($(ref $name,)*) = *self;
                    $($name.hash(state);)*
                }
            }
        );
    }

    impl_hash_tuple! {}
    impl_hash_tuple! { A }
    impl_hash_tuple! { A B }
    impl_hash_tuple! { A B C }
    impl_hash_tuple! { A B C D }
    impl_hash_tuple! { A B C D E }
    impl_hash_tuple! { A B C D E F }
    impl_hash_tuple! { A B C D E F G }
    impl_hash_tuple! { A B C D E F G H }
    impl_hash_tuple! { A B C D E F G H I }
    impl_hash_tuple! { A B C D E F G H I J }
    impl_hash_tuple! { A B C D E F G H I J K }
    impl_hash_tuple! { A B C D E F G H I J K L }

    #[stable(feature = "rust1", since = "1.0.0")]
    impl<T: Hash> Hash for [T] {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.len().hash(state);
            Hash::hash_slice(self, state)
        }
    }


    #[stable(feature = "rust1", since = "1.0.0")]
    impl<'a, T: ?Sized + Hash> Hash for &'a T {
        fn hash<H: Hasher>(&self, state: &mut H) {
            (**self).hash(state);
        }
    }

    #[stable(feature = "rust1", since = "1.0.0")]
    impl<'a, T: ?Sized + Hash> Hash for &'a mut T {
        fn hash<H: Hasher>(&self, state: &mut H) {
            (**self).hash(state);
        }
    }

    #[stable(feature = "rust1", since = "1.0.0")]
    impl<T> Hash for *const T {
        fn hash<H: Hasher>(&self, state: &mut H) {
            state.write_usize(*self as usize)
        }
    }

    #[stable(feature = "rust1", since = "1.0.0")]
    impl<T> Hash for *mut T {
        fn hash<H: Hasher>(&self, state: &mut H) {
            state.write_usize(*self as usize)
        }
    }

    #[stable(feature = "rust1", since = "1.0.0")]
    impl<'a, T, B: ?Sized> Hash for Cow<'a, T, B>
        where B: Hash + ToOwned<T>
    {
        fn hash<H: Hasher>(&self, state: &mut H) {
            Hash::hash(&**self, state)
        }
    }
}
