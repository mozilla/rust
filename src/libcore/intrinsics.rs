//! Compiler intrinsics.
//!
//! The corresponding definitions are in `librustc_codegen_llvm/intrinsic.rs`.
//! The corresponding const implementations are in `librustc_mir/interpret/intrinsics.rs`
//!
//! # Const intrinsics
//!
//! Note: any changes to the constness of intrinsics should be discussed with the language team.
//! This includes changes in the stability of the constness.
//!
//! In order to make an intrinsic usable at compile-time, one needs to copy the implementation
//! from https://github.com/rust-lang/miri/blob/master/src/shims/intrinsics.rs to
//! `librustc_mir/interpret/intrinsics.rs` and add a
//! `#[rustc_const_unstable(feature = "foo", issue = "01234")]` to the intrinsic.
//!
//! If an intrinsic is supposed to be used from a `const fn` with a `rustc_const_stable` attribute,
//! the intrinsic's attribute must be `rustc_const_stable`, too. Such a change should not be done
//! without T-lang consulation, because it bakes a feature into the language that cannot be
//! replicated in user code without compiler support.
//!
//! # Volatiles
//!
//! The volatile intrinsics provide operations intended to act on I/O
//! memory, which are guaranteed to not be reordered by the compiler
//! across other volatile intrinsics. See the LLVM documentation on
//! [[volatile]].
//!
//! [volatile]: http://llvm.org/docs/LangRef.html#volatile-memory-accesses
//!
//! # Atomics
//!
//! The atomic intrinsics provide common atomic operations on machine
//! words, with multiple possible memory orderings. They obey the same
//! semantics as C++11. See the LLVM documentation on [[atomics]].
//!
//! [atomics]: http://llvm.org/docs/Atomics.html
//!
//! A quick refresher on memory ordering:
//!
//! * Acquire - a barrier for acquiring a lock. Subsequent reads and writes
//!   take place after the barrier.
//! * Release - a barrier for releasing a lock. Preceding reads and writes
//!   take place before the barrier.
//! * Sequentially consistent - sequentially consistent operations are
//!   guaranteed to happen in order. This is the standard mode for working
//!   with atomic types and is equivalent to Java's `volatile`.

#![unstable(
    feature = "core_intrinsics",
    reason = "intrinsics are unlikely to ever be stabilized, instead \
                      they should be used through stabilized interfaces \
                      in the rest of the standard library",
    issue = "none"
)]
#![allow(missing_docs)]

use crate::mem;

#[stable(feature = "drop_in_place", since = "1.8.0")]
#[rustc_deprecated(
    reason = "no longer an intrinsic - use `ptr::drop_in_place` directly",
    since = "1.18.0"
)]
pub use crate::ptr::drop_in_place;

extern "rust-intrinsic" {
    // N.B., these intrinsics take raw pointers because they mutate aliased
    // memory, which is not valid for either `&` or `&mut`.

    /// Stores a value if the current value is the same as the `old` value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `compare_exchange` method by passing
    /// [`Ordering::SeqCst`](../../std/sync/atomic/enum.Ordering.html)
    /// as both the `success` and `failure` parameters. For example,
    /// [`AtomicBool::compare_exchange`][compare_exchange].
    ///
    /// [compare_exchange]: ../../std/sync/atomic/struct.AtomicBool.html#method.compare_exchange
    pub fn atomic_cxchg<T>(dst: *mut T, old: T, src: T) -> (T, bool);
    /// Stores a value if the current value is the same as the `old` value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `compare_exchange` method by passing
    /// [`Ordering::Acquire`](../../std/sync/atomic/enum.Ordering.html)
    /// as both the `success` and `failure` parameters. For example,
    /// [`AtomicBool::compare_exchange`][compare_exchange].
    ///
    /// [compare_exchange]: ../../std/sync/atomic/struct.AtomicBool.html#method.compare_exchange
    pub fn atomic_cxchg_acq<T>(dst: *mut T, old: T, src: T) -> (T, bool);
    /// Stores a value if the current value is the same as the `old` value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `compare_exchange` method by passing
    /// [`Ordering::Release`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `success` and
    /// [`Ordering::Relaxed`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `failure` parameters. For example,
    /// [`AtomicBool::compare_exchange`][compare_exchange].
    ///
    /// [compare_exchange]: ../../std/sync/atomic/struct.AtomicBool.html#method.compare_exchange
    pub fn atomic_cxchg_rel<T>(dst: *mut T, old: T, src: T) -> (T, bool);
    /// Stores a value if the current value is the same as the `old` value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `compare_exchange` method by passing
    /// [`Ordering::AcqRel`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `success` and
    /// [`Ordering::Acquire`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `failure` parameters. For example,
    /// [`AtomicBool::compare_exchange`][compare_exchange].
    ///
    /// [compare_exchange]: ../../std/sync/atomic/struct.AtomicBool.html#method.compare_exchange
    pub fn atomic_cxchg_acqrel<T>(dst: *mut T, old: T, src: T) -> (T, bool);
    /// Stores a value if the current value is the same as the `old` value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `compare_exchange` method by passing
    /// [`Ordering::Relaxed`](../../std/sync/atomic/enum.Ordering.html)
    /// as both the `success` and `failure` parameters. For example,
    /// [`AtomicBool::compare_exchange`][compare_exchange].
    ///
    /// [compare_exchange]: ../../std/sync/atomic/struct.AtomicBool.html#method.compare_exchange
    pub fn atomic_cxchg_relaxed<T>(dst: *mut T, old: T, src: T) -> (T, bool);
    /// Stores a value if the current value is the same as the `old` value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `compare_exchange` method by passing
    /// [`Ordering::SeqCst`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `success` and
    /// [`Ordering::Relaxed`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `failure` parameters. For example,
    /// [`AtomicBool::compare_exchange`][compare_exchange].
    ///
    /// [compare_exchange]: ../../std/sync/atomic/struct.AtomicBool.html#method.compare_exchange
    pub fn atomic_cxchg_failrelaxed<T>(dst: *mut T, old: T, src: T) -> (T, bool);
    /// Stores a value if the current value is the same as the `old` value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `compare_exchange` method by passing
    /// [`Ordering::SeqCst`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `success` and
    /// [`Ordering::Acquire`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `failure` parameters. For example,
    /// [`AtomicBool::compare_exchange`][compare_exchange].
    ///
    /// [compare_exchange]: ../../std/sync/atomic/struct.AtomicBool.html#method.compare_exchange
    pub fn atomic_cxchg_failacq<T>(dst: *mut T, old: T, src: T) -> (T, bool);
    /// Stores a value if the current value is the same as the `old` value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `compare_exchange` method by passing
    /// [`Ordering::Acquire`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `success` and
    /// [`Ordering::Relaxed`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `failure` parameters. For example,
    /// [`AtomicBool::compare_exchange`][compare_exchange].
    ///
    /// [compare_exchange]: ../../std/sync/atomic/struct.AtomicBool.html#method.compare_exchange
    pub fn atomic_cxchg_acq_failrelaxed<T>(dst: *mut T, old: T, src: T) -> (T, bool);
    /// Stores a value if the current value is the same as the `old` value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `compare_exchange` method by passing
    /// [`Ordering::AcqRel`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `success` and
    /// [`Ordering::Relaxed`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `failure` parameters. For example,
    /// [`AtomicBool::compare_exchange`][compare_exchange].
    ///
    /// [compare_exchange]: ../../std/sync/atomic/struct.AtomicBool.html#method.compare_exchange
    pub fn atomic_cxchg_acqrel_failrelaxed<T>(dst: *mut T, old: T, src: T) -> (T, bool);

    /// Stores a value if the current value is the same as the `old` value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `compare_exchange_weak` method by passing
    /// [`Ordering::SeqCst`](../../std/sync/atomic/enum.Ordering.html)
    /// as both the `success` and `failure` parameters. For example,
    /// [`AtomicBool::compare_exchange_weak`][cew].
    ///
    /// [cew]: ../../std/sync/atomic/struct.AtomicBool.html#method.compare_exchange_weak
    pub fn atomic_cxchgweak<T>(dst: *mut T, old: T, src: T) -> (T, bool);
    /// Stores a value if the current value is the same as the `old` value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `compare_exchange_weak` method by passing
    /// [`Ordering::Acquire`](../../std/sync/atomic/enum.Ordering.html)
    /// as both the `success` and `failure` parameters. For example,
    /// [`AtomicBool::compare_exchange_weak`][cew].
    ///
    /// [cew]: ../../std/sync/atomic/struct.AtomicBool.html#method.compare_exchange_weak
    pub fn atomic_cxchgweak_acq<T>(dst: *mut T, old: T, src: T) -> (T, bool);
    /// Stores a value if the current value is the same as the `old` value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `compare_exchange_weak` method by passing
    /// [`Ordering::Release`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `success` and
    /// [`Ordering::Relaxed`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `failure` parameters. For example,
    /// [`AtomicBool::compare_exchange_weak`][cew].
    ///
    /// [cew]: ../../std/sync/atomic/struct.AtomicBool.html#method.compare_exchange_weak
    pub fn atomic_cxchgweak_rel<T>(dst: *mut T, old: T, src: T) -> (T, bool);
    /// Stores a value if the current value is the same as the `old` value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `compare_exchange_weak` method by passing
    /// [`Ordering::AcqRel`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `success` and
    /// [`Ordering::Acquire`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `failure` parameters. For example,
    /// [`AtomicBool::compare_exchange_weak`][cew].
    ///
    /// [cew]: ../../std/sync/atomic/struct.AtomicBool.html#method.compare_exchange_weak
    pub fn atomic_cxchgweak_acqrel<T>(dst: *mut T, old: T, src: T) -> (T, bool);
    /// Stores a value if the current value is the same as the `old` value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `compare_exchange_weak` method by passing
    /// [`Ordering::Relaxed`](../../std/sync/atomic/enum.Ordering.html)
    /// as both the `success` and `failure` parameters. For example,
    /// [`AtomicBool::compare_exchange_weak`][cew].
    ///
    /// [cew]: ../../std/sync/atomic/struct.AtomicBool.html#method.compare_exchange_weak
    pub fn atomic_cxchgweak_relaxed<T>(dst: *mut T, old: T, src: T) -> (T, bool);
    /// Stores a value if the current value is the same as the `old` value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `compare_exchange_weak` method by passing
    /// [`Ordering::SeqCst`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `success` and
    /// [`Ordering::Relaxed`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `failure` parameters. For example,
    /// [`AtomicBool::compare_exchange_weak`][cew].
    ///
    /// [cew]: ../../std/sync/atomic/struct.AtomicBool.html#method.compare_exchange_weak
    pub fn atomic_cxchgweak_failrelaxed<T>(dst: *mut T, old: T, src: T) -> (T, bool);
    /// Stores a value if the current value is the same as the `old` value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `compare_exchange_weak` method by passing
    /// [`Ordering::SeqCst`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `success` and
    /// [`Ordering::Acquire`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `failure` parameters. For example,
    /// [`AtomicBool::compare_exchange_weak`][cew].
    ///
    /// [cew]: ../../std/sync/atomic/struct.AtomicBool.html#method.compare_exchange_weak
    pub fn atomic_cxchgweak_failacq<T>(dst: *mut T, old: T, src: T) -> (T, bool);
    /// Stores a value if the current value is the same as the `old` value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `compare_exchange_weak` method by passing
    /// [`Ordering::Acquire`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `success` and
    /// [`Ordering::Relaxed`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `failure` parameters. For example,
    /// [`AtomicBool::compare_exchange_weak`][cew].
    ///
    /// [cew]: ../../std/sync/atomic/struct.AtomicBool.html#method.compare_exchange_weak
    pub fn atomic_cxchgweak_acq_failrelaxed<T>(dst: *mut T, old: T, src: T) -> (T, bool);
    /// Stores a value if the current value is the same as the `old` value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `compare_exchange_weak` method by passing
    /// [`Ordering::AcqRel`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `success` and
    /// [`Ordering::Relaxed`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `failure` parameters. For example,
    /// [`AtomicBool::compare_exchange_weak`][cew].
    ///
    /// [cew]: ../../std/sync/atomic/struct.AtomicBool.html#method.compare_exchange_weak
    pub fn atomic_cxchgweak_acqrel_failrelaxed<T>(dst: *mut T, old: T, src: T) -> (T, bool);

    /// Loads the current value of the pointer.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `load` method by passing
    /// [`Ordering::SeqCst`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicBool::load`](../../std/sync/atomic/struct.AtomicBool.html#method.load).
    pub fn atomic_load<T>(src: *const T) -> T;
    /// Loads the current value of the pointer.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `load` method by passing
    /// [`Ordering::Acquire`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicBool::load`](../../std/sync/atomic/struct.AtomicBool.html#method.load).
    pub fn atomic_load_acq<T>(src: *const T) -> T;
    /// Loads the current value of the pointer.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `load` method by passing
    /// [`Ordering::Relaxed`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicBool::load`](../../std/sync/atomic/struct.AtomicBool.html#method.load).
    pub fn atomic_load_relaxed<T>(src: *const T) -> T;
    pub fn atomic_load_unordered<T>(src: *const T) -> T;

    /// Stores the value at the specified memory location.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `store` method by passing
    /// [`Ordering::SeqCst`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicBool::store`](../../std/sync/atomic/struct.AtomicBool.html#method.store).
    pub fn atomic_store<T>(dst: *mut T, val: T);
    /// Stores the value at the specified memory location.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `store` method by passing
    /// [`Ordering::Release`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicBool::store`](../../std/sync/atomic/struct.AtomicBool.html#method.store).
    pub fn atomic_store_rel<T>(dst: *mut T, val: T);
    /// Stores the value at the specified memory location.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `store` method by passing
    /// [`Ordering::Relaxed`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicBool::store`](../../std/sync/atomic/struct.AtomicBool.html#method.store).
    pub fn atomic_store_relaxed<T>(dst: *mut T, val: T);
    pub fn atomic_store_unordered<T>(dst: *mut T, val: T);

    /// Stores the value at the specified memory location, returning the old value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `swap` method by passing
    /// [`Ordering::SeqCst`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicBool::swap`](../../std/sync/atomic/struct.AtomicBool.html#method.swap).
    pub fn atomic_xchg<T>(dst: *mut T, src: T) -> T;
    /// Stores the value at the specified memory location, returning the old value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `swap` method by passing
    /// [`Ordering::Acquire`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicBool::swap`](../../std/sync/atomic/struct.AtomicBool.html#method.swap).
    pub fn atomic_xchg_acq<T>(dst: *mut T, src: T) -> T;
    /// Stores the value at the specified memory location, returning the old value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `swap` method by passing
    /// [`Ordering::Release`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicBool::swap`](../../std/sync/atomic/struct.AtomicBool.html#method.swap).
    pub fn atomic_xchg_rel<T>(dst: *mut T, src: T) -> T;
    /// Stores the value at the specified memory location, returning the old value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `swap` method by passing
    /// [`Ordering::AcqRel`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicBool::swap`](../../std/sync/atomic/struct.AtomicBool.html#method.swap).
    pub fn atomic_xchg_acqrel<T>(dst: *mut T, src: T) -> T;
    /// Stores the value at the specified memory location, returning the old value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `swap` method by passing
    /// [`Ordering::Relaxed`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicBool::swap`](../../std/sync/atomic/struct.AtomicBool.html#method.swap).
    pub fn atomic_xchg_relaxed<T>(dst: *mut T, src: T) -> T;

    /// Adds to the current value, returning the previous value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `fetch_add` method by passing
    /// [`Ordering::SeqCst`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicIsize::fetch_add`](../../std/sync/atomic/struct.AtomicIsize.html#method.fetch_add).
    pub fn atomic_xadd<T>(dst: *mut T, src: T) -> T;
    /// Adds to the current value, returning the previous value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `fetch_add` method by passing
    /// [`Ordering::Acquire`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicIsize::fetch_add`](../../std/sync/atomic/struct.AtomicIsize.html#method.fetch_add).
    pub fn atomic_xadd_acq<T>(dst: *mut T, src: T) -> T;
    /// Adds to the current value, returning the previous value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `fetch_add` method by passing
    /// [`Ordering::Release`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicIsize::fetch_add`](../../std/sync/atomic/struct.AtomicIsize.html#method.fetch_add).
    pub fn atomic_xadd_rel<T>(dst: *mut T, src: T) -> T;
    /// Adds to the current value, returning the previous value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `fetch_add` method by passing
    /// [`Ordering::AcqRel`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicIsize::fetch_add`](../../std/sync/atomic/struct.AtomicIsize.html#method.fetch_add).
    pub fn atomic_xadd_acqrel<T>(dst: *mut T, src: T) -> T;
    /// Adds to the current value, returning the previous value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `fetch_add` method by passing
    /// [`Ordering::Relaxed`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicIsize::fetch_add`](../../std/sync/atomic/struct.AtomicIsize.html#method.fetch_add).
    pub fn atomic_xadd_relaxed<T>(dst: *mut T, src: T) -> T;

    /// Subtract from the current value, returning the previous value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `fetch_sub` method by passing
    /// [`Ordering::SeqCst`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicIsize::fetch_sub`](../../std/sync/atomic/struct.AtomicIsize.html#method.fetch_sub).
    pub fn atomic_xsub<T>(dst: *mut T, src: T) -> T;
    /// Subtract from the current value, returning the previous value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `fetch_sub` method by passing
    /// [`Ordering::Acquire`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicIsize::fetch_sub`](../../std/sync/atomic/struct.AtomicIsize.html#method.fetch_sub).
    pub fn atomic_xsub_acq<T>(dst: *mut T, src: T) -> T;
    /// Subtract from the current value, returning the previous value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `fetch_sub` method by passing
    /// [`Ordering::Release`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicIsize::fetch_sub`](../../std/sync/atomic/struct.AtomicIsize.html#method.fetch_sub).
    pub fn atomic_xsub_rel<T>(dst: *mut T, src: T) -> T;
    /// Subtract from the current value, returning the previous value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `fetch_sub` method by passing
    /// [`Ordering::AcqRel`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicIsize::fetch_sub`](../../std/sync/atomic/struct.AtomicIsize.html#method.fetch_sub).
    pub fn atomic_xsub_acqrel<T>(dst: *mut T, src: T) -> T;
    /// Subtract from the current value, returning the previous value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `fetch_sub` method by passing
    /// [`Ordering::Relaxed`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicIsize::fetch_sub`](../../std/sync/atomic/struct.AtomicIsize.html#method.fetch_sub).
    pub fn atomic_xsub_relaxed<T>(dst: *mut T, src: T) -> T;

    /// Bitwise and with the current value, returning the previous value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `fetch_and` method by passing
    /// [`Ordering::SeqCst`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicBool::fetch_and`](../../std/sync/atomic/struct.AtomicBool.html#method.fetch_and).
    pub fn atomic_and<T>(dst: *mut T, src: T) -> T;
    /// Bitwise and with the current value, returning the previous value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `fetch_and` method by passing
    /// [`Ordering::Acquire`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicBool::fetch_and`](../../std/sync/atomic/struct.AtomicBool.html#method.fetch_and).
    pub fn atomic_and_acq<T>(dst: *mut T, src: T) -> T;
    /// Bitwise and with the current value, returning the previous value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `fetch_and` method by passing
    /// [`Ordering::Release`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicBool::fetch_and`](../../std/sync/atomic/struct.AtomicBool.html#method.fetch_and).
    pub fn atomic_and_rel<T>(dst: *mut T, src: T) -> T;
    /// Bitwise and with the current value, returning the previous value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `fetch_and` method by passing
    /// [`Ordering::AcqRel`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicBool::fetch_and`](../../std/sync/atomic/struct.AtomicBool.html#method.fetch_and).
    pub fn atomic_and_acqrel<T>(dst: *mut T, src: T) -> T;
    /// Bitwise and with the current value, returning the previous value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `fetch_and` method by passing
    /// [`Ordering::Relaxed`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicBool::fetch_and`](../../std/sync/atomic/struct.AtomicBool.html#method.fetch_and).
    pub fn atomic_and_relaxed<T>(dst: *mut T, src: T) -> T;

    /// Bitwise nand with the current value, returning the previous value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic::AtomicBool` type via the `fetch_nand` method by passing
    /// [`Ordering::SeqCst`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicBool::fetch_nand`](../../std/sync/atomic/struct.AtomicBool.html#method.fetch_nand).
    pub fn atomic_nand<T>(dst: *mut T, src: T) -> T;
    /// Bitwise nand with the current value, returning the previous value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic::AtomicBool` type via the `fetch_nand` method by passing
    /// [`Ordering::Acquire`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicBool::fetch_nand`](../../std/sync/atomic/struct.AtomicBool.html#method.fetch_nand).
    pub fn atomic_nand_acq<T>(dst: *mut T, src: T) -> T;
    /// Bitwise nand with the current value, returning the previous value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic::AtomicBool` type via the `fetch_nand` method by passing
    /// [`Ordering::Release`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicBool::fetch_nand`](../../std/sync/atomic/struct.AtomicBool.html#method.fetch_nand).
    pub fn atomic_nand_rel<T>(dst: *mut T, src: T) -> T;
    /// Bitwise nand with the current value, returning the previous value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic::AtomicBool` type via the `fetch_nand` method by passing
    /// [`Ordering::AcqRel`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicBool::fetch_nand`](../../std/sync/atomic/struct.AtomicBool.html#method.fetch_nand).
    pub fn atomic_nand_acqrel<T>(dst: *mut T, src: T) -> T;
    /// Bitwise nand with the current value, returning the previous value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic::AtomicBool` type via the `fetch_nand` method by passing
    /// [`Ordering::Relaxed`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicBool::fetch_nand`](../../std/sync/atomic/struct.AtomicBool.html#method.fetch_nand).
    pub fn atomic_nand_relaxed<T>(dst: *mut T, src: T) -> T;

    /// Bitwise or with the current value, returning the previous value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `fetch_or` method by passing
    /// [`Ordering::SeqCst`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicBool::fetch_or`](../../std/sync/atomic/struct.AtomicBool.html#method.fetch_or).
    pub fn atomic_or<T>(dst: *mut T, src: T) -> T;
    /// Bitwise or with the current value, returning the previous value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `fetch_or` method by passing
    /// [`Ordering::Acquire`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicBool::fetch_or`](../../std/sync/atomic/struct.AtomicBool.html#method.fetch_or).
    pub fn atomic_or_acq<T>(dst: *mut T, src: T) -> T;
    /// Bitwise or with the current value, returning the previous value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `fetch_or` method by passing
    /// [`Ordering::Release`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicBool::fetch_or`](../../std/sync/atomic/struct.AtomicBool.html#method.fetch_or).
    pub fn atomic_or_rel<T>(dst: *mut T, src: T) -> T;
    /// Bitwise or with the current value, returning the previous value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `fetch_or` method by passing
    /// [`Ordering::AcqRel`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicBool::fetch_or`](../../std/sync/atomic/struct.AtomicBool.html#method.fetch_or).
    pub fn atomic_or_acqrel<T>(dst: *mut T, src: T) -> T;
    /// Bitwise or with the current value, returning the previous value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `fetch_or` method by passing
    /// [`Ordering::Relaxed`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicBool::fetch_or`](../../std/sync/atomic/struct.AtomicBool.html#method.fetch_or).
    pub fn atomic_or_relaxed<T>(dst: *mut T, src: T) -> T;

    /// Bitwise xor with the current value, returning the previous value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `fetch_xor` method by passing
    /// [`Ordering::SeqCst`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicBool::fetch_xor`](../../std/sync/atomic/struct.AtomicBool.html#method.fetch_xor).
    pub fn atomic_xor<T>(dst: *mut T, src: T) -> T;
    /// Bitwise xor with the current value, returning the previous value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `fetch_xor` method by passing
    /// [`Ordering::Acquire`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicBool::fetch_xor`](../../std/sync/atomic/struct.AtomicBool.html#method.fetch_xor).
    pub fn atomic_xor_acq<T>(dst: *mut T, src: T) -> T;
    /// Bitwise xor with the current value, returning the previous value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `fetch_xor` method by passing
    /// [`Ordering::Release`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicBool::fetch_xor`](../../std/sync/atomic/struct.AtomicBool.html#method.fetch_xor).
    pub fn atomic_xor_rel<T>(dst: *mut T, src: T) -> T;
    /// Bitwise xor with the current value, returning the previous value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `fetch_xor` method by passing
    /// [`Ordering::AcqRel`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicBool::fetch_xor`](../../std/sync/atomic/struct.AtomicBool.html#method.fetch_xor).
    pub fn atomic_xor_acqrel<T>(dst: *mut T, src: T) -> T;
    /// Bitwise xor with the current value, returning the previous value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` types via the `fetch_xor` method by passing
    /// [`Ordering::Relaxed`](../../std/sync/atomic/enum.Ordering.html)
    /// as the `order`. For example,
    /// [`AtomicBool::fetch_xor`](../../std/sync/atomic/struct.AtomicBool.html#method.fetch_xor).
    pub fn atomic_xor_relaxed<T>(dst: *mut T, src: T) -> T;

    /// Maximum with the current value using a signed comparison.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` signed integer types via the `fetch_max` method by passing
    /// [`Ordering::SeqCst`](../../std/sync/atomic/enum.Ordering.html#variant.SeqCst)
    /// as the `order`. For example,
    /// [`AtomicI32::fetch_max`](../../std/sync/atomic/struct.AtomicI32.html#method.fetch_max).
    pub fn atomic_max<T>(dst: *mut T, src: T) -> T;
    /// Maximum with the current value using a signed comparison.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` signed integer types via the `fetch_max` method by passing
    /// [`Ordering::Acquire`](../../std/sync/atomic/enum.Ordering.html#variant.Acquire)
    /// as the `order`. For example,
    /// [`AtomicI32::fetch_max`](../../std/sync/atomic/struct.AtomicI32.html#method.fetch_max).
    pub fn atomic_max_acq<T>(dst: *mut T, src: T) -> T;
    /// Maximum with the current value using a signed comparison.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` signed integer types via the `fetch_max` method by passing
    /// [`Ordering::Release`](../../std/sync/atomic/enum.Ordering.html#variant.Release)
    /// as the `order`. For example,
    /// [`AtomicI32::fetch_max`](../../std/sync/atomic/struct.AtomicI32.html#method.fetch_max).
    pub fn atomic_max_rel<T>(dst: *mut T, src: T) -> T;
    /// Maximum with the current value using a signed comparison.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` signed integer types via the `fetch_max` method by passing
    /// [`Ordering::AcqRel`](../../std/sync/atomic/enum.Ordering.html#variant.AcqRel)
    /// as the `order`. For example,
    /// [`AtomicI32::fetch_max`](../../std/sync/atomic/struct.AtomicI32.html#method.fetch_max).
    pub fn atomic_max_acqrel<T>(dst: *mut T, src: T) -> T;
    /// Maximum with the current value.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` signed integer types via the `fetch_max` method by passing
    /// [`Ordering::Relaxed`](../../std/sync/atomic/enum.Ordering.html#variant.Relaxed)
    /// as the `order`. For example,
    /// [`AtomicI32::fetch_max`](../../std/sync/atomic/struct.AtomicI32.html#method.fetch_max).
    pub fn atomic_max_relaxed<T>(dst: *mut T, src: T) -> T;

    /// Minimum with the current value using a signed comparison.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` signed integer types via the `fetch_min` method by passing
    /// [`Ordering::SeqCst`](../../std/sync/atomic/enum.Ordering.html#variant.SeqCst)
    /// as the `order`. For example,
    /// [`AtomicI32::fetch_min`](../../std/sync/atomic/struct.AtomicI32.html#method.fetch_min).
    pub fn atomic_min<T>(dst: *mut T, src: T) -> T;
    /// Minimum with the current value using a signed comparison.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` signed integer types via the `fetch_min` method by passing
    /// [`Ordering::Acquire`](../../std/sync/atomic/enum.Ordering.html#variant.Acquire)
    /// as the `order`. For example,
    /// [`AtomicI32::fetch_min`](../../std/sync/atomic/struct.AtomicI32.html#method.fetch_min).
    pub fn atomic_min_acq<T>(dst: *mut T, src: T) -> T;
    /// Minimum with the current value using a signed comparison.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` signed integer types via the `fetch_min` method by passing
    /// [`Ordering::Release`](../../std/sync/atomic/enum.Ordering.html#variant.Release)
    /// as the `order`. For example,
    /// [`AtomicI32::fetch_min`](../../std/sync/atomic/struct.AtomicI32.html#method.fetch_min).
    pub fn atomic_min_rel<T>(dst: *mut T, src: T) -> T;
    /// Minimum with the current value using a signed comparison.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` signed integer types via the `fetch_min` method by passing
    /// [`Ordering::AcqRel`](../../std/sync/atomic/enum.Ordering.html#variant.AcqRel)
    /// as the `order`. For example,
    /// [`AtomicI32::fetch_min`](../../std/sync/atomic/struct.AtomicI32.html#method.fetch_min).
    pub fn atomic_min_acqrel<T>(dst: *mut T, src: T) -> T;
    /// Minimum with the current value using a signed comparison.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` signed integer types via the `fetch_min` method by passing
    /// [`Ordering::Relaxed`](../../std/sync/atomic/enum.Ordering.html#variant.Relaxed)
    /// as the `order`. For example,
    /// [`AtomicI32::fetch_min`](../../std/sync/atomic/struct.AtomicI32.html#method.fetch_min).
    pub fn atomic_min_relaxed<T>(dst: *mut T, src: T) -> T;

    /// Minimum with the current value using an unsigned comparison.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` unsigned integer types via the `fetch_min` method by passing
    /// [`Ordering::SeqCst`](../../std/sync/atomic/enum.Ordering.html#variant.SeqCst)
    /// as the `order`. For example,
    /// [`AtomicU32::fetch_min`](../../std/sync/atomic/struct.AtomicU32.html#method.fetch_min).
    pub fn atomic_umin<T>(dst: *mut T, src: T) -> T;
    /// Minimum with the current value using an unsigned comparison.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` unsigned integer types via the `fetch_min` method by passing
    /// [`Ordering::Acquire`](../../std/sync/atomic/enum.Ordering.html#variant.Acquire)
    /// as the `order`. For example,
    /// [`AtomicU32::fetch_min`](../../std/sync/atomic/struct.AtomicU32.html#method.fetch_min).
    pub fn atomic_umin_acq<T>(dst: *mut T, src: T) -> T;
    /// Minimum with the current value using an unsigned comparison.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` unsigned integer types via the `fetch_min` method by passing
    /// [`Ordering::Release`](../../std/sync/atomic/enum.Ordering.html#variant.Release)
    /// as the `order`. For example,
    /// [`AtomicU32::fetch_min`](../../std/sync/atomic/struct.AtomicU32.html#method.fetch_min).
    pub fn atomic_umin_rel<T>(dst: *mut T, src: T) -> T;
    /// Minimum with the current value using an unsigned comparison.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` unsigned integer types via the `fetch_min` method by passing
    /// [`Ordering::AcqRel`](../../std/sync/atomic/enum.Ordering.html#variant.AcqRel)
    /// as the `order`. For example,
    /// [`AtomicU32::fetch_min`](../../std/sync/atomic/struct.AtomicU32.html#method.fetch_min).
    pub fn atomic_umin_acqrel<T>(dst: *mut T, src: T) -> T;
    /// Minimum with the current value using an unsigned comparison.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` unsigned integer types via the `fetch_min` method by passing
    /// [`Ordering::Relaxed`](../../std/sync/atomic/enum.Ordering.html#variant.Relaxed)
    /// as the `order`. For example,
    /// [`AtomicU32::fetch_min`](../../std/sync/atomic/struct.AtomicU32.html#method.fetch_min).
    pub fn atomic_umin_relaxed<T>(dst: *mut T, src: T) -> T;

    /// Maximum with the current value using an unsigned comparison.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` unsigned integer types via the `fetch_max` method by passing
    /// [`Ordering::SeqCst`](../../std/sync/atomic/enum.Ordering.html#variant.SeqCst)
    /// as the `order`. For example,
    /// [`AtomicU32::fetch_max`](../../std/sync/atomic/struct.AtomicU32.html#method.fetch_max).
    pub fn atomic_umax<T>(dst: *mut T, src: T) -> T;
    /// Maximum with the current value using an unsigned comparison.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` unsigned integer types via the `fetch_max` method by passing
    /// [`Ordering::Acquire`](../../std/sync/atomic/enum.Ordering.html#variant.Acquire)
    /// as the `order`. For example,
    /// [`AtomicU32::fetch_max`](../../std/sync/atomic/struct.AtomicU32.html#method.fetch_max).
    pub fn atomic_umax_acq<T>(dst: *mut T, src: T) -> T;
    /// Maximum with the current value using an unsigned comparison.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` unsigned integer types via the `fetch_max` method by passing
    /// [`Ordering::Release`](../../std/sync/atomic/enum.Ordering.html#variant.Release)
    /// as the `order`. For example,
    /// [`AtomicU32::fetch_max`](../../std/sync/atomic/struct.AtomicU32.html#method.fetch_max).
    pub fn atomic_umax_rel<T>(dst: *mut T, src: T) -> T;
    /// Maximum with the current value using an unsigned comparison.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` unsigned integer types via the `fetch_max` method by passing
    /// [`Ordering::AcqRel`](../../std/sync/atomic/enum.Ordering.html#variant.AcqRel)
    /// as the `order`. For example,
    /// [`AtomicU32::fetch_max`](../../std/sync/atomic/struct.AtomicU32.html#method.fetch_max).
    pub fn atomic_umax_acqrel<T>(dst: *mut T, src: T) -> T;
    /// Maximum with the current value using an unsigned comparison.
    ///
    /// The stabilized version of this intrinsic is available on the
    /// `std::sync::atomic` unsigned integer types via the `fetch_max` method by passing
    /// [`Ordering::Relaxed`](../../std/sync/atomic/enum.Ordering.html#variant.Relaxed)
    /// as the `order`. For example,
    /// [`AtomicU32::fetch_max`](../../std/sync/atomic/struct.AtomicU32.html#method.fetch_max).
    pub fn atomic_umax_relaxed<T>(dst: *mut T, src: T) -> T;

    /// The `prefetch` intrinsic is a hint to the code generator to insert a prefetch instruction
    /// if supported; otherwise, it is a no-op.
    /// Prefetches have no effect on the behavior of the program but can change its performance
    /// characteristics.
    ///
    /// The `locality` argument must be a constant integer and is a temporal locality specifier
    /// ranging from (0) - no locality, to (3) - extremely local keep in cache
    pub fn prefetch_read_data<T>(data: *const T, locality: i32);
    /// The `prefetch` intrinsic is a hint to the code generator to insert a prefetch instruction
    /// if supported; otherwise, it is a no-op.
    /// Prefetches have no effect on the behavior of the program but can change its performance
    /// characteristics.
    ///
    /// The `locality` argument must be a constant integer and is a temporal locality specifier
    /// ranging from (0) - no locality, to (3) - extremely local keep in cache
    pub fn prefetch_write_data<T>(data: *const T, locality: i32);
    /// The `prefetch` intrinsic is a hint to the code generator to insert a prefetch instruction
    /// if supported; otherwise, it is a no-op.
    /// Prefetches have no effect on the behavior of the program but can change its performance
    /// characteristics.
    ///
    /// The `locality` argument must be a constant integer and is a temporal locality specifier
    /// ranging from (0) - no locality, to (3) - extremely local keep in cache
    pub fn prefetch_read_instruction<T>(data: *const T, locality: i32);
    /// The `prefetch` intrinsic is a hint to the code generator to insert a prefetch instruction
    /// if supported; otherwise, it is a no-op.
    /// Prefetches have no effect on the behavior of the program but can change its performance
    /// characteristics.
    ///
    /// The `locality` argument must be a constant integer and is a temporal locality specifier
    /// ranging from (0) - no locality, to (3) - extremely local keep in cache
    pub fn prefetch_write_instruction<T>(data: *const T, locality: i32);
}

extern "rust-intrinsic" {

    /// An atomic fence.
    ///
    /// The stabilized version of this intrinsic is available in
    /// [`std::sync::atomic::fence`](../../std/sync/atomic/fn.fence.html)
    /// by passing
    /// [`Ordering::SeqCst`](../../std/sync/atomic/enum.Ordering.html#variant.SeqCst)
    /// as the `order`.
    pub fn atomic_fence();
    /// An atomic fence.
    ///
    /// The stabilized version of this intrinsic is available in
    /// [`std::sync::atomic::fence`](../../std/sync/atomic/fn.fence.html)
    /// by passing
    /// [`Ordering::Acquire`](../../std/sync/atomic/enum.Ordering.html#variant.Acquire)
    /// as the `order`.
    pub fn atomic_fence_acq();
    /// An atomic fence.
    ///
    /// The stabilized version of this intrinsic is available in
    /// [`std::sync::atomic::fence`](../../std/sync/atomic/fn.fence.html)
    /// by passing
    /// [`Ordering::Release`](../../std/sync/atomic/enum.Ordering.html#variant.Release)
    /// as the `order`.
    pub fn atomic_fence_rel();
    /// An atomic fence.
    ///
    /// The stabilized version of this intrinsic is available in
    /// [`std::sync::atomic::fence`](../../std/sync/atomic/fn.fence.html)
    /// by passing
    /// [`Ordering::AcqRel`](../../std/sync/atomic/enum.Ordering.html#variant.AcqRel)
    /// as the `order`.
    pub fn atomic_fence_acqrel();

    /// A compiler-only memory barrier.
    ///
    /// Memory accesses will never be reordered across this barrier by the
    /// compiler, but no instructions will be emitted for it. This is
    /// appropriate for operations on the same thread that may be preempted,
    /// such as when interacting with signal handlers.
    ///
    /// The stabilized version of this intrinsic is available in
    /// [`std::sync::atomic::compiler_fence`](../../std/sync/atomic/fn.compiler_fence.html)
    /// by passing
    /// [`Ordering::SeqCst`](../../std/sync/atomic/enum.Ordering.html#variant.SeqCst)
    /// as the `order`.
    pub fn atomic_singlethreadfence();
    /// A compiler-only memory barrier.
    ///
    /// Memory accesses will never be reordered across this barrier by the
    /// compiler, but no instructions will be emitted for it. This is
    /// appropriate for operations on the same thread that may be preempted,
    /// such as when interacting with signal handlers.
    ///
    /// The stabilized version of this intrinsic is available in
    /// [`std::sync::atomic::compiler_fence`](../../std/sync/atomic/fn.compiler_fence.html)
    /// by passing
    /// [`Ordering::Acquire`](../../std/sync/atomic/enum.Ordering.html#variant.Acquire)
    /// as the `order`.
    pub fn atomic_singlethreadfence_acq();
    /// A compiler-only memory barrier.
    ///
    /// Memory accesses will never be reordered across this barrier by the
    /// compiler, but no instructions will be emitted for it. This is
    /// appropriate for operations on the same thread that may be preempted,
    /// such as when interacting with signal handlers.
    ///
    /// The stabilized version of this intrinsic is available in
    /// [`std::sync::atomic::compiler_fence`](../../std/sync/atomic/fn.compiler_fence.html)
    /// by passing
    /// [`Ordering::Release`](../../std/sync/atomic/enum.Ordering.html#variant.Release)
    /// as the `order`.
    pub fn atomic_singlethreadfence_rel();
    /// A compiler-only memory barrier.
    ///
    /// Memory accesses will never be reordered across this barrier by the
    /// compiler, but no instructions will be emitted for it. This is
    /// appropriate for operations on the same thread that may be preempted,
    /// such as when interacting with signal handlers.
    ///
    /// The stabilized version of this intrinsic is available in
    /// [`std::sync::atomic::compiler_fence`](../../std/sync/atomic/fn.compiler_fence.html)
    /// by passing
    /// [`Ordering::AcqRel`](../../std/sync/atomic/enum.Ordering.html#variant.AcqRel)
    /// as the `order`.
    pub fn atomic_singlethreadfence_acqrel();

    /// Magic intrinsic that derives its meaning from attributes
    /// attached to the function.
    ///
    /// For example, dataflow uses this to inject static assertions so
    /// that `rustc_peek(potentially_uninitialized)` would actually
    /// double-check that dataflow did indeed compute that it is
    /// uninitialized at that point in the control flow.
    pub fn rustc_peek<T>(_: T) -> T;

    /// Aborts the execution of the process.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::process::abort`](../../std/process/fn.abort.html)
    pub fn abort() -> !;

    /// Tells LLVM that this point in the code is not reachable, enabling
    /// further optimizations.
    ///
    /// N.B., this is very different from the `unreachable!()` macro: Unlike the
    /// macro, which panics when it is executed, it is *undefined behavior* to
    /// reach code marked with this function.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::hint::unreachable_unchecked`](../../std/hint/fn.unreachable_unchecked.html).
    pub fn unreachable() -> !;

    /// Informs the optimizer that a condition is always true.
    /// If the condition is false, the behavior is undefined.
    ///
    /// No code is generated for this intrinsic, but the optimizer will try
    /// to preserve it (and its condition) between passes, which may interfere
    /// with optimization of surrounding code and reduce performance. It should
    /// not be used if the invariant can be discovered by the optimizer on its
    /// own, or if it does not enable any significant optimizations.
    pub fn assume(b: bool);

    /// Hints to the compiler that branch condition is likely to be true.
    /// Returns the value passed to it.
    ///
    /// Any use other than with `if` statements will probably not have an effect.
    pub fn likely(b: bool) -> bool;

    /// Hints to the compiler that branch condition is likely to be false.
    /// Returns the value passed to it.
    ///
    /// Any use other than with `if` statements will probably not have an effect.
    pub fn unlikely(b: bool) -> bool;

    /// Executes a breakpoint trap, for inspection by a debugger.
    pub fn breakpoint();

    /// The size of a type in bytes.
    ///
    /// More specifically, this is the offset in bytes between successive
    /// items of the same type, including alignment padding.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::mem::size_of`](../../std/mem/fn.size_of.html).
    #[rustc_const_stable(feature = "const_size_of", since = "1.40.0")]
    pub fn size_of<T>() -> usize;

    /// Moves a value to an uninitialized memory location.
    ///
    /// Drop glue is not run on the destination.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::ptr::write`](../../std/ptr/fn.write.html).
    pub fn move_val_init<T>(dst: *mut T, src: T);

    /// The minimum alignment of a type.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::mem::align_of`](../../std/mem/fn.align_of.html).
    #[rustc_const_stable(feature = "const_min_align_of", since = "1.40.0")]
    pub fn min_align_of<T>() -> usize;
    #[rustc_const_unstable(feature = "const_pref_align_of", issue = "none")]
    pub fn pref_align_of<T>() -> usize;

    /// The size of the referenced value in bytes.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::mem::size_of_val`](../../std/mem/fn.size_of_val.html).
    #[cfg(bootstrap)]
    pub fn size_of_val<T: ?Sized>(_: &T) -> usize;
    /// The minimum alignment of the type of the value that `val` points to.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::mem::min_align_of_val`](../../std/mem/fn.min_align_of_val.html).
    #[cfg(bootstrap)]
    pub fn min_align_of_val<T: ?Sized>(_: &T) -> usize;

    /// The size of the referenced value in bytes.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::mem::size_of_val`](../../std/mem/fn.size_of_val.html).
    #[cfg(not(bootstrap))]
    pub fn size_of_val<T: ?Sized>(_: *const T) -> usize;
    #[cfg(not(bootstrap))]
    pub fn min_align_of_val<T: ?Sized>(_: *const T) -> usize;

    /// Gets a static string slice containing the name of a type.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::any::type_name`](../../std/any/fn.type_name.html)
    #[rustc_const_unstable(feature = "const_type_name", issue = "none")]
    pub fn type_name<T: ?Sized>() -> &'static str;

    /// Gets an identifier which is globally unique to the specified type. This
    /// function will return the same value for a type regardless of whichever
    /// crate it is invoked in.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::any::TypeId::of`](../../std/any/struct.TypeId.html#method.of)
    #[rustc_const_unstable(feature = "const_type_id", issue = "none")]
    pub fn type_id<T: ?Sized + 'static>() -> u64;

    /// A guard for unsafe functions that cannot ever be executed if `T` is uninhabited:
    /// This will statically either panic, or do nothing.
    #[cfg(bootstrap)]
    pub fn panic_if_uninhabited<T>();

    /// A guard for unsafe functions that cannot ever be executed if `T` is uninhabited:
    /// This will statically either panic, or do nothing.
    #[cfg(not(bootstrap))]
    pub fn assert_inhabited<T>();

    /// A guard for unsafe functions that cannot ever be executed if `T` does not permit
    /// zero-initialization: This will statically either panic, or do nothing.
    #[cfg(not(bootstrap))]
    pub fn assert_zero_valid<T>();

    /// A guard for unsafe functions that cannot ever be executed if `T` has invalid
    /// bit patterns: This will statically either panic, or do nothing.
    #[cfg(not(bootstrap))]
    pub fn assert_uninit_valid<T>();

    /// Gets a reference to a static `Location` indicating where it was called.
    #[rustc_const_unstable(feature = "const_caller_location", issue = "47809")]
    pub fn caller_location() -> &'static crate::panic::Location<'static>;

    /// Moves a value out of scope without running drop glue.
    /// This exists solely for `mem::forget_unsized`; normal `forget` uses `ManuallyDrop` instead.
    pub fn forget<T: ?Sized>(_: T);

    /// Reinterprets the bits of a value of one type as another type.
    ///
    /// Both types must have the same size. Neither the original, nor the result,
    /// may be an [invalid value](../../nomicon/what-unsafe-does.html).
    ///
    /// `transmute` is semantically equivalent to a bitwise move of one type
    /// into another. It copies the bits from the source value into the
    /// destination value, then forgets the original. It's equivalent to C's
    /// `memcpy` under the hood, just like `transmute_copy`.
    ///
    /// `transmute` is **incredibly** unsafe. There are a vast number of ways to
    /// cause [undefined behavior][ub] with this function. `transmute` should be
    /// the absolute last resort.
    ///
    /// The [nomicon](../../nomicon/transmutes.html) has additional
    /// documentation.
    ///
    /// [ub]: ../../reference/behavior-considered-undefined.html
    ///
    /// # Examples
    ///
    /// There are a few things that `transmute` is really useful for.
    ///
    /// Turning a pointer into a function pointer. This is *not* portable to
    /// machines where function pointers and data pointers have different sizes.
    ///
    /// ```
    /// fn foo() -> i32 {
    ///     0
    /// }
    /// let pointer = foo as *const ();
    /// let function = unsafe {
    ///     std::mem::transmute::<*const (), fn() -> i32>(pointer)
    /// };
    /// assert_eq!(function(), 0);
    /// ```
    ///
    /// Extending a lifetime, or shortening an invariant lifetime. This is
    /// advanced, very unsafe Rust!
    ///
    /// ```
    /// struct R<'a>(&'a i32);
    /// unsafe fn extend_lifetime<'b>(r: R<'b>) -> R<'static> {
    ///     std::mem::transmute::<R<'b>, R<'static>>(r)
    /// }
    ///
    /// unsafe fn shorten_invariant_lifetime<'b, 'c>(r: &'b mut R<'static>)
    ///                                              -> &'b mut R<'c> {
    ///     std::mem::transmute::<&'b mut R<'static>, &'b mut R<'c>>(r)
    /// }
    /// ```
    ///
    /// # Alternatives
    ///
    /// Don't despair: many uses of `transmute` can be achieved through other means.
    /// Below are common applications of `transmute` which can be replaced with safer
    /// constructs.
    ///
    /// Turning a pointer into a `usize`:
    ///
    /// ```
    /// let ptr = &0;
    /// let ptr_num_transmute = unsafe {
    ///     std::mem::transmute::<&i32, usize>(ptr)
    /// };
    ///
    /// // Use an `as` cast instead
    /// let ptr_num_cast = ptr as *const i32 as usize;
    /// ```
    ///
    /// Turning a `*mut T` into an `&mut T`:
    ///
    /// ```
    /// let ptr: *mut i32 = &mut 0;
    /// let ref_transmuted = unsafe {
    ///     std::mem::transmute::<*mut i32, &mut i32>(ptr)
    /// };
    ///
    /// // Use a reborrow instead
    /// let ref_casted = unsafe { &mut *ptr };
    /// ```
    ///
    /// Turning an `&mut T` into an `&mut U`:
    ///
    /// ```
    /// let ptr = &mut 0;
    /// let val_transmuted = unsafe {
    ///     std::mem::transmute::<&mut i32, &mut u32>(ptr)
    /// };
    ///
    /// // Now, put together `as` and reborrowing - note the chaining of `as`
    /// // `as` is not transitive
    /// let val_casts = unsafe { &mut *(ptr as *mut i32 as *mut u32) };
    /// ```
    ///
    /// Turning an `&str` into an `&[u8]`:
    ///
    /// ```
    /// // this is not a good way to do this.
    /// let slice = unsafe { std::mem::transmute::<&str, &[u8]>("Rust") };
    /// assert_eq!(slice, &[82, 117, 115, 116]);
    ///
    /// // You could use `str::as_bytes`
    /// let slice = "Rust".as_bytes();
    /// assert_eq!(slice, &[82, 117, 115, 116]);
    ///
    /// // Or, just use a byte string, if you have control over the string
    /// // literal
    /// assert_eq!(b"Rust", &[82, 117, 115, 116]);
    /// ```
    ///
    /// Turning a `Vec<&T>` into a `Vec<Option<&T>>`:
    ///
    /// ```
    /// let store = [0, 1, 2, 3];
    /// let v_orig = store.iter().collect::<Vec<&i32>>();
    ///
    /// // clone the vector as we will reuse them later
    /// let v_clone = v_orig.clone();
    ///
    /// // Using transmute: this relies on the unspecified data layout of `Vec`, which is a
    /// // bad idea and could cause Undefined Behavior.
    /// // However, it is no-copy.
    /// let v_transmuted = unsafe {
    ///     std::mem::transmute::<Vec<&i32>, Vec<Option<&i32>>>(v_clone)
    /// };
    ///
    /// let v_clone = v_orig.clone();
    ///
    /// // This is the suggested, safe way.
    /// // It does copy the entire vector, though, into a new array.
    /// let v_collected = v_clone.into_iter()
    ///                          .map(Some)
    ///                          .collect::<Vec<Option<&i32>>>();
    ///
    /// let v_clone = v_orig.clone();
    ///
    /// // The no-copy, unsafe way, still using transmute, but not relying on the data layout.
    /// // Like the first approach, this reuses the `Vec` internals.
    /// // Therefore, the new inner type must have the
    /// // exact same size, *and the same alignment*, as the old type.
    /// // The same caveats exist for this method as transmute, for
    /// // the original inner type (`&i32`) to the converted inner type
    /// // (`Option<&i32>`), so read the nomicon pages linked above and also
    /// // consult the [`from_raw_parts`] documentation.
    /// let v_from_raw = unsafe {
    // FIXME Update this when vec_into_raw_parts is stabilized
    ///     // Ensure the original vector is not dropped.
    ///     let mut v_clone = std::mem::ManuallyDrop::new(v_clone);
    ///     Vec::from_raw_parts(v_clone.as_mut_ptr() as *mut Option<&i32>,
    ///                         v_clone.len(),
    ///                         v_clone.capacity())
    /// };
    /// ```
    ///
    /// [`from_raw_parts`]: ../../std/vec/struct.Vec.html#method.from_raw_parts
    ///
    /// Implementing `split_at_mut`:
    ///
    /// ```
    /// use std::{slice, mem};
    ///
    /// // There are multiple ways to do this, and there are multiple problems
    /// // with the following (transmute) way.
    /// fn split_at_mut_transmute<T>(slice: &mut [T], mid: usize)
    ///                              -> (&mut [T], &mut [T]) {
    ///     let len = slice.len();
    ///     assert!(mid <= len);
    ///     unsafe {
    ///         let slice2 = mem::transmute::<&mut [T], &mut [T]>(slice);
    ///         // first: transmute is not typesafe; all it checks is that T and
    ///         // U are of the same size. Second, right here, you have two
    ///         // mutable references pointing to the same memory.
    ///         (&mut slice[0..mid], &mut slice2[mid..len])
    ///     }
    /// }
    ///
    /// // This gets rid of the typesafety problems; `&mut *` will *only* give
    /// // you an `&mut T` from an `&mut T` or `*mut T`.
    /// fn split_at_mut_casts<T>(slice: &mut [T], mid: usize)
    ///                          -> (&mut [T], &mut [T]) {
    ///     let len = slice.len();
    ///     assert!(mid <= len);
    ///     unsafe {
    ///         let slice2 = &mut *(slice as *mut [T]);
    ///         // however, you still have two mutable references pointing to
    ///         // the same memory.
    ///         (&mut slice[0..mid], &mut slice2[mid..len])
    ///     }
    /// }
    ///
    /// // This is how the standard library does it. This is the best method, if
    /// // you need to do something like this
    /// fn split_at_stdlib<T>(slice: &mut [T], mid: usize)
    ///                       -> (&mut [T], &mut [T]) {
    ///     let len = slice.len();
    ///     assert!(mid <= len);
    ///     unsafe {
    ///         let ptr = slice.as_mut_ptr();
    ///         // This now has three mutable references pointing at the same
    ///         // memory. `slice`, the rvalue ret.0, and the rvalue ret.1.
    ///         // `slice` is never used after `let ptr = ...`, and so one can
    ///         // treat it as "dead", and therefore, you only have two real
    ///         // mutable slices.
    ///         (slice::from_raw_parts_mut(ptr, mid),
    ///          slice::from_raw_parts_mut(ptr.add(mid), len - mid))
    ///     }
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_transmute", issue = "53605")]
    pub fn transmute<T, U>(e: T) -> U;

    /// Returns `true` if the actual type given as `T` requires drop
    /// glue; returns `false` if the actual type provided for `T`
    /// implements `Copy`.
    ///
    /// If the actual type neither requires drop glue nor implements
    /// `Copy`, then may return `true` or `false`.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::mem::needs_drop`](../../std/mem/fn.needs_drop.html).
    #[rustc_const_stable(feature = "const_needs_drop", since = "1.40.0")]
    pub fn needs_drop<T>() -> bool;

    /// Calculates the offset from a pointer.
    ///
    /// This is implemented as an intrinsic to avoid converting to and from an
    /// integer, since the conversion would throw away aliasing information.
    ///
    /// # Safety
    ///
    /// Both the starting and resulting pointer must be either in bounds or one
    /// byte past the end of an allocated object. If either pointer is out of
    /// bounds or arithmetic overflow occurs then any further use of the
    /// returned value will result in undefined behavior.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::pointer::offset`](../../std/primitive.pointer.html#method.offset).
    pub fn offset<T>(dst: *const T, offset: isize) -> *const T;

    /// Calculates the offset from a pointer, potentially wrapping.
    ///
    /// This is implemented as an intrinsic to avoid converting to and from an
    /// integer, since the conversion inhibits certain optimizations.
    ///
    /// # Safety
    ///
    /// Unlike the `offset` intrinsic, this intrinsic does not restrict the
    /// resulting pointer to point into or one byte past the end of an allocated
    /// object, and it wraps with two's complement arithmetic. The resulting
    /// value is not necessarily valid to be used to actually access memory.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::pointer::wrapping_offset`](../../std/primitive.pointer.html#method.wrapping_offset).
    pub fn arith_offset<T>(dst: *const T, offset: isize) -> *const T;

    /// Equivalent to the appropriate `llvm.memcpy.p0i8.0i8.*` intrinsic, with
    /// a size of `count` * `size_of::<T>()` and an alignment of
    /// `min_align_of::<T>()`
    ///
    /// The volatile parameter is set to `true`, so it will not be optimized out
    /// unless size is equal to zero.
    pub fn volatile_copy_nonoverlapping_memory<T>(dst: *mut T, src: *const T, count: usize);
    /// Equivalent to the appropriate `llvm.memmove.p0i8.0i8.*` intrinsic, with
    /// a size of `count` * `size_of::<T>()` and an alignment of
    /// `min_align_of::<T>()`
    ///
    /// The volatile parameter is set to `true`, so it will not be optimized out
    /// unless size is equal to zero.
    pub fn volatile_copy_memory<T>(dst: *mut T, src: *const T, count: usize);
    /// Equivalent to the appropriate `llvm.memset.p0i8.*` intrinsic, with a
    /// size of `count` * `size_of::<T>()` and an alignment of
    /// `min_align_of::<T>()`.
    ///
    /// The volatile parameter is set to `true`, so it will not be optimized out
    /// unless size is equal to zero.
    pub fn volatile_set_memory<T>(dst: *mut T, val: u8, count: usize);

    /// Performs a volatile load from the `src` pointer.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::ptr::read_volatile`](../../std/ptr/fn.read_volatile.html).
    pub fn volatile_load<T>(src: *const T) -> T;
    /// Performs a volatile store to the `dst` pointer.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::ptr::write_volatile`](../../std/ptr/fn.write_volatile.html).
    pub fn volatile_store<T>(dst: *mut T, val: T);

    /// Performs a volatile load from the `src` pointer
    /// The pointer is not required to be aligned.
    pub fn unaligned_volatile_load<T>(src: *const T) -> T;
    /// Performs a volatile store to the `dst` pointer.
    /// The pointer is not required to be aligned.
    pub fn unaligned_volatile_store<T>(dst: *mut T, val: T);

    /// Returns the square root of an `f32`
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f32::sqrt`](../../std/primitive.f32.html#method.sqrt)
    pub fn sqrtf32(x: f32) -> f32;
    /// Returns the square root of an `f64`
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f64::sqrt`](../../std/primitive.f64.html#method.sqrt)
    pub fn sqrtf64(x: f64) -> f64;

    /// Raises an `f32` to an integer power.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f32::powi`](../../std/primitive.f32.html#method.powi)
    pub fn powif32(a: f32, x: i32) -> f32;
    /// Raises an `f64` to an integer power.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f64::powi`](../../std/primitive.f64.html#method.powi)
    pub fn powif64(a: f64, x: i32) -> f64;

    /// Returns the sine of an `f32`.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f32::sin`](../../std/primitive.f32.html#method.sin)
    pub fn sinf32(x: f32) -> f32;
    /// Returns the sine of an `f64`.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f64::sin`](../../std/primitive.f64.html#method.sin)
    pub fn sinf64(x: f64) -> f64;

    /// Returns the cosine of an `f32`.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f32::cos`](../../std/primitive.f32.html#method.cos)
    pub fn cosf32(x: f32) -> f32;
    /// Returns the cosine of an `f64`.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f64::cos`](../../std/primitive.f64.html#method.cos)
    pub fn cosf64(x: f64) -> f64;

    /// Raises an `f32` to an `f32` power.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f32::powf`](../../std/primitive.f32.html#method.powf)
    pub fn powf32(a: f32, x: f32) -> f32;
    /// Raises an `f64` to an `f64` power.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f64::powf`](../../std/primitive.f64.html#method.powf)
    pub fn powf64(a: f64, x: f64) -> f64;

    /// Returns the exponential of an `f32`.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f32::exp`](../../std/primitive.f32.html#method.exp)
    pub fn expf32(x: f32) -> f32;
    /// Returns the exponential of an `f64`.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f64::exp`](../../std/primitive.f64.html#method.exp)
    pub fn expf64(x: f64) -> f64;

    /// Returns 2 raised to the power of an `f32`.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f32::exp2`](../../std/primitive.f32.html#method.exp2)
    pub fn exp2f32(x: f32) -> f32;
    /// Returns 2 raised to the power of an `f64`.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f64::exp2`](../../std/primitive.f64.html#method.exp2)
    pub fn exp2f64(x: f64) -> f64;

    /// Returns the natural logarithm of an `f32`.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f32::ln`](../../std/primitive.f32.html#method.ln)
    pub fn logf32(x: f32) -> f32;
    /// Returns the natural logarithm of an `f64`.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f64::ln`](../../std/primitive.f64.html#method.ln)
    pub fn logf64(x: f64) -> f64;

    /// Returns the base 10 logarithm of an `f32`.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f32::log10`](../../std/primitive.f32.html#method.log10)
    pub fn log10f32(x: f32) -> f32;
    /// Returns the base 10 logarithm of an `f64`.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f64::log10`](../../std/primitive.f64.html#method.log10)
    pub fn log10f64(x: f64) -> f64;

    /// Returns the base 2 logarithm of an `f32`.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f32::log2`](../../std/primitive.f32.html#method.log2)
    pub fn log2f32(x: f32) -> f32;
    /// Returns the base 2 logarithm of an `f64`.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f64::log2`](../../std/primitive.f64.html#method.log2)
    pub fn log2f64(x: f64) -> f64;

    /// Returns `a * b + c` for `f32` values.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f32::mul_add`](../../std/primitive.f32.html#method.mul_add)
    pub fn fmaf32(a: f32, b: f32, c: f32) -> f32;
    /// Returns `a * b + c` for `f64` values.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f64::mul_add`](../../std/primitive.f64.html#method.mul_add)
    pub fn fmaf64(a: f64, b: f64, c: f64) -> f64;

    /// Returns the absolute value of an `f32`.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f32::abs`](../../std/primitive.f32.html#method.abs)
    pub fn fabsf32(x: f32) -> f32;
    /// Returns the absolute value of an `f64`.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f64::abs`](../../std/primitive.f64.html#method.abs)
    pub fn fabsf64(x: f64) -> f64;

    /// Returns the minimum of two `f32` values.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f32::min`](../../std/primitive.f32.html#method.min)
    pub fn minnumf32(x: f32, y: f32) -> f32;
    /// Returns the minimum of two `f64` values.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f64::min`](../../std/primitive.f64.html#method.min)
    pub fn minnumf64(x: f64, y: f64) -> f64;
    /// Returns the maximum of two `f32` values.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f32::max`](../../std/primitive.f32.html#method.max)
    pub fn maxnumf32(x: f32, y: f32) -> f32;
    /// Returns the maximum of two `f64` values.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f64::max`](../../std/primitive.f64.html#method.max)
    pub fn maxnumf64(x: f64, y: f64) -> f64;

    /// Copies the sign from `y` to `x` for `f32` values.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f32::copysign`](../../std/primitive.f32.html#method.copysign)
    pub fn copysignf32(x: f32, y: f32) -> f32;
    /// Copies the sign from `y` to `x` for `f64` values.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f64::copysign`](../../std/primitive.f64.html#method.copysign)
    pub fn copysignf64(x: f64, y: f64) -> f64;

    /// Returns the largest integer less than or equal to an `f32`.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f32::floor`](../../std/primitive.f32.html#method.floor)
    pub fn floorf32(x: f32) -> f32;
    /// Returns the largest integer less than or equal to an `f64`.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f64::floor`](../../std/primitive.f64.html#method.floor)
    pub fn floorf64(x: f64) -> f64;

    /// Returns the smallest integer greater than or equal to an `f32`.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f32::ceil`](../../std/primitive.f32.html#method.ceil)
    pub fn ceilf32(x: f32) -> f32;
    /// Returns the smallest integer greater than or equal to an `f64`.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f64::ceil`](../../std/primitive.f64.html#method.ceil)
    pub fn ceilf64(x: f64) -> f64;

    /// Returns the integer part of an `f32`.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f32::trunc`](../../std/primitive.f32.html#method.trunc)
    pub fn truncf32(x: f32) -> f32;
    /// Returns the integer part of an `f64`.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f64::trunc`](../../std/primitive.f64.html#method.trunc)
    pub fn truncf64(x: f64) -> f64;

    /// Returns the nearest integer to an `f32`. May raise an inexact floating-point exception
    /// if the argument is not an integer.
    pub fn rintf32(x: f32) -> f32;
    /// Returns the nearest integer to an `f64`. May raise an inexact floating-point exception
    /// if the argument is not an integer.
    pub fn rintf64(x: f64) -> f64;

    /// Returns the nearest integer to an `f32`.
    pub fn nearbyintf32(x: f32) -> f32;
    /// Returns the nearest integer to an `f64`.
    pub fn nearbyintf64(x: f64) -> f64;

    /// Returns the nearest integer to an `f32`. Rounds half-way cases away from zero.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f32::round`](../../std/primitive.f32.html#method.round)
    pub fn roundf32(x: f32) -> f32;
    /// Returns the nearest integer to an `f64`. Rounds half-way cases away from zero.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::f64::round`](../../std/primitive.f64.html#method.round)
    pub fn roundf64(x: f64) -> f64;

    /// Float addition that allows optimizations based on algebraic rules.
    /// May assume inputs are finite.
    pub fn fadd_fast<T>(a: T, b: T) -> T;

    /// Float subtraction that allows optimizations based on algebraic rules.
    /// May assume inputs are finite.
    pub fn fsub_fast<T>(a: T, b: T) -> T;

    /// Float multiplication that allows optimizations based on algebraic rules.
    /// May assume inputs are finite.
    pub fn fmul_fast<T>(a: T, b: T) -> T;

    /// Float division that allows optimizations based on algebraic rules.
    /// May assume inputs are finite.
    pub fn fdiv_fast<T>(a: T, b: T) -> T;

    /// Float remainder that allows optimizations based on algebraic rules.
    /// May assume inputs are finite.
    pub fn frem_fast<T>(a: T, b: T) -> T;

    /// Convert with LLVM’s fptoui/fptosi, which may return undef for values out of range
    /// (<https://github.com/rust-lang/rust/issues/10184>)
    /// This is under stabilization at <https://github.com/rust-lang/rust/issues/67058>
    pub fn float_to_int_approx_unchecked<Float, Int>(value: Float) -> Int;

    /// Returns the number of bits set in an integer type `T`
    ///
    /// The stabilized versions of this intrinsic are available on the integer
    /// primitives via the `count_ones` method. For example,
    /// [`std::u32::count_ones`](../../std/primitive.u32.html#method.count_ones)
    #[rustc_const_stable(feature = "const_ctpop", since = "1.40.0")]
    pub fn ctpop<T>(x: T) -> T;

    /// Returns the number of leading unset bits (zeroes) in an integer type `T`.
    ///
    /// The stabilized versions of this intrinsic are available on the integer
    /// primitives via the `leading_zeros` method. For example,
    /// [`std::u32::leading_zeros`](../../std/primitive.u32.html#method.leading_zeros)
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(core_intrinsics)]
    ///
    /// use std::intrinsics::ctlz;
    ///
    /// let x = 0b0001_1100_u8;
    /// let num_leading = ctlz(x);
    /// assert_eq!(num_leading, 3);
    /// ```
    ///
    /// An `x` with value `0` will return the bit width of `T`.
    ///
    /// ```
    /// #![feature(core_intrinsics)]
    ///
    /// use std::intrinsics::ctlz;
    ///
    /// let x = 0u16;
    /// let num_leading = ctlz(x);
    /// assert_eq!(num_leading, 16);
    /// ```
    #[rustc_const_stable(feature = "const_ctlz", since = "1.40.0")]
    pub fn ctlz<T>(x: T) -> T;

    /// Like `ctlz`, but extra-unsafe as it returns `undef` when
    /// given an `x` with value `0`.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(core_intrinsics)]
    ///
    /// use std::intrinsics::ctlz_nonzero;
    ///
    /// let x = 0b0001_1100_u8;
    /// let num_leading = unsafe { ctlz_nonzero(x) };
    /// assert_eq!(num_leading, 3);
    /// ```
    #[rustc_const_unstable(feature = "constctlz", issue = "none")]
    pub fn ctlz_nonzero<T>(x: T) -> T;

    /// Returns the number of trailing unset bits (zeroes) in an integer type `T`.
    ///
    /// The stabilized versions of this intrinsic are available on the integer
    /// primitives via the `trailing_zeros` method. For example,
    /// [`std::u32::trailing_zeros`](../../std/primitive.u32.html#method.trailing_zeros)
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(core_intrinsics)]
    ///
    /// use std::intrinsics::cttz;
    ///
    /// let x = 0b0011_1000_u8;
    /// let num_trailing = cttz(x);
    /// assert_eq!(num_trailing, 3);
    /// ```
    ///
    /// An `x` with value `0` will return the bit width of `T`:
    ///
    /// ```
    /// #![feature(core_intrinsics)]
    ///
    /// use std::intrinsics::cttz;
    ///
    /// let x = 0u16;
    /// let num_trailing = cttz(x);
    /// assert_eq!(num_trailing, 16);
    /// ```
    #[rustc_const_stable(feature = "const_cttz", since = "1.40.0")]
    pub fn cttz<T>(x: T) -> T;

    /// Like `cttz`, but extra-unsafe as it returns `undef` when
    /// given an `x` with value `0`.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(core_intrinsics)]
    ///
    /// use std::intrinsics::cttz_nonzero;
    ///
    /// let x = 0b0011_1000_u8;
    /// let num_trailing = unsafe { cttz_nonzero(x) };
    /// assert_eq!(num_trailing, 3);
    /// ```
    #[rustc_const_unstable(feature = "const_cttz", issue = "none")]
    pub fn cttz_nonzero<T>(x: T) -> T;

    /// Reverses the bytes in an integer type `T`.
    ///
    /// The stabilized versions of this intrinsic are available on the integer
    /// primitives via the `swap_bytes` method. For example,
    /// [`std::u32::swap_bytes`](../../std/primitive.u32.html#method.swap_bytes)
    #[rustc_const_stable(feature = "const_bswap", since = "1.40.0")]
    pub fn bswap<T>(x: T) -> T;

    /// Reverses the bits in an integer type `T`.
    ///
    /// The stabilized versions of this intrinsic are available on the integer
    /// primitives via the `reverse_bits` method. For example,
    /// [`std::u32::reverse_bits`](../../std/primitive.u32.html#method.reverse_bits)
    #[rustc_const_stable(feature = "const_bitreverse", since = "1.40.0")]
    pub fn bitreverse<T>(x: T) -> T;

    /// Performs checked integer addition.
    ///
    /// The stabilized versions of this intrinsic are available on the integer
    /// primitives via the `overflowing_add` method. For example,
    /// [`std::u32::overflowing_add`](../../std/primitive.u32.html#method.overflowing_add)
    #[rustc_const_stable(feature = "const_int_overflow", since = "1.40.0")]
    pub fn add_with_overflow<T>(x: T, y: T) -> (T, bool);

    /// Performs checked integer subtraction
    ///
    /// The stabilized versions of this intrinsic are available on the integer
    /// primitives via the `overflowing_sub` method. For example,
    /// [`std::u32::overflowing_sub`](../../std/primitive.u32.html#method.overflowing_sub)
    #[rustc_const_stable(feature = "const_int_overflow", since = "1.40.0")]
    pub fn sub_with_overflow<T>(x: T, y: T) -> (T, bool);

    /// Performs checked integer multiplication
    ///
    /// The stabilized versions of this intrinsic are available on the integer
    /// primitives via the `overflowing_mul` method. For example,
    /// [`std::u32::overflowing_mul`](../../std/primitive.u32.html#method.overflowing_mul)
    #[rustc_const_stable(feature = "const_int_overflow", since = "1.40.0")]
    pub fn mul_with_overflow<T>(x: T, y: T) -> (T, bool);

    /// Performs an exact division, resulting in undefined behavior where
    /// `x % y != 0` or `y == 0` or `x == T::min_value() && y == -1`
    pub fn exact_div<T>(x: T, y: T) -> T;

    /// Performs an unchecked division, resulting in undefined behavior
    /// where y = 0 or x = `T::min_value()` and y = -1
    ///
    /// The stabilized versions of this intrinsic are available on the integer
    /// primitives via the `checked_div` method. For example,
    /// [`std::u32::checked_div`](../../std/primitive.u32.html#method.checked_div)
    #[rustc_const_unstable(feature = "const_int_unchecked_arith", issue = "none")]
    pub fn unchecked_div<T>(x: T, y: T) -> T;
    /// Returns the remainder of an unchecked division, resulting in
    /// undefined behavior where y = 0 or x = `T::min_value()` and y = -1
    ///
    /// The stabilized versions of this intrinsic are available on the integer
    /// primitives via the `checked_rem` method. For example,
    /// [`std::u32::checked_rem`](../../std/primitive.u32.html#method.checked_rem)
    #[rustc_const_unstable(feature = "const_int_unchecked_arith", issue = "none")]
    pub fn unchecked_rem<T>(x: T, y: T) -> T;

    /// Performs an unchecked left shift, resulting in undefined behavior when
    /// y < 0 or y >= N, where N is the width of T in bits.
    ///
    /// The stabilized versions of this intrinsic are available on the integer
    /// primitives via the `checked_shl` method. For example,
    /// [`std::u32::checked_shl`](../../std/primitive.u32.html#method.checked_shl)
    #[rustc_const_stable(feature = "const_int_unchecked", since = "1.40.0")]
    pub fn unchecked_shl<T>(x: T, y: T) -> T;
    /// Performs an unchecked right shift, resulting in undefined behavior when
    /// y < 0 or y >= N, where N is the width of T in bits.
    ///
    /// The stabilized versions of this intrinsic are available on the integer
    /// primitives via the `checked_shr` method. For example,
    /// [`std::u32::checked_shr`](../../std/primitive.u32.html#method.checked_shr)
    #[rustc_const_stable(feature = "const_int_unchecked", since = "1.40.0")]
    pub fn unchecked_shr<T>(x: T, y: T) -> T;

    /// Returns the result of an unchecked addition, resulting in
    /// undefined behavior when `x + y > T::max_value()` or `x + y < T::min_value()`.
    #[rustc_const_unstable(feature = "const_int_unchecked_arith", issue = "none")]
    pub fn unchecked_add<T>(x: T, y: T) -> T;

    /// Returns the result of an unchecked subtraction, resulting in
    /// undefined behavior when `x - y > T::max_value()` or `x - y < T::min_value()`.
    #[rustc_const_unstable(feature = "const_int_unchecked_arith", issue = "none")]
    pub fn unchecked_sub<T>(x: T, y: T) -> T;

    /// Returns the result of an unchecked multiplication, resulting in
    /// undefined behavior when `x * y > T::max_value()` or `x * y < T::min_value()`.
    #[rustc_const_unstable(feature = "const_int_unchecked_arith", issue = "none")]
    pub fn unchecked_mul<T>(x: T, y: T) -> T;

    /// Performs rotate left.
    ///
    /// The stabilized versions of this intrinsic are available on the integer
    /// primitives via the `rotate_left` method. For example,
    /// [`std::u32::rotate_left`](../../std/primitive.u32.html#method.rotate_left)
    #[rustc_const_stable(feature = "const_int_rotate", since = "1.40.0")]
    pub fn rotate_left<T>(x: T, y: T) -> T;

    /// Performs rotate right.
    ///
    /// The stabilized versions of this intrinsic are available on the integer
    /// primitives via the `rotate_right` method. For example,
    /// [`std::u32::rotate_right`](../../std/primitive.u32.html#method.rotate_right)
    #[rustc_const_stable(feature = "const_int_rotate", since = "1.40.0")]
    pub fn rotate_right<T>(x: T, y: T) -> T;

    /// Returns (a + b) mod 2<sup>N</sup>, where N is the width of T in bits.
    ///
    /// The stabilized versions of this intrinsic are available on the integer
    /// primitives via the `checked_add` method. For example,
    /// [`std::u32::checked_add`](../../std/primitive.u32.html#method.checked_add)
    #[rustc_const_stable(feature = "const_int_wrapping", since = "1.40.0")]
    pub fn wrapping_add<T>(a: T, b: T) -> T;
    /// Returns (a - b) mod 2<sup>N</sup>, where N is the width of T in bits.
    ///
    /// The stabilized versions of this intrinsic are available on the integer
    /// primitives via the `checked_sub` method. For example,
    /// [`std::u32::checked_sub`](../../std/primitive.u32.html#method.checked_sub)
    #[rustc_const_stable(feature = "const_int_wrapping", since = "1.40.0")]
    pub fn wrapping_sub<T>(a: T, b: T) -> T;
    /// Returns (a * b) mod 2<sup>N</sup>, where N is the width of T in bits.
    ///
    /// The stabilized versions of this intrinsic are available on the integer
    /// primitives via the `checked_mul` method. For example,
    /// [`std::u32::checked_mul`](../../std/primitive.u32.html#method.checked_mul)
    #[rustc_const_stable(feature = "const_int_wrapping", since = "1.40.0")]
    pub fn wrapping_mul<T>(a: T, b: T) -> T;

    /// Computes `a + b`, while saturating at numeric bounds.
    ///
    /// The stabilized versions of this intrinsic are available on the integer
    /// primitives via the `saturating_add` method. For example,
    /// [`std::u32::saturating_add`](../../std/primitive.u32.html#method.saturating_add)
    #[rustc_const_stable(feature = "const_int_saturating", since = "1.40.0")]
    pub fn saturating_add<T>(a: T, b: T) -> T;
    /// Computes `a - b`, while saturating at numeric bounds.
    ///
    /// The stabilized versions of this intrinsic are available on the integer
    /// primitives via the `saturating_sub` method. For example,
    /// [`std::u32::saturating_sub`](../../std/primitive.u32.html#method.saturating_sub)
    #[rustc_const_stable(feature = "const_int_saturating", since = "1.40.0")]
    pub fn saturating_sub<T>(a: T, b: T) -> T;

    /// Returns the value of the discriminant for the variant in 'v',
    /// cast to a `u64`; if `T` has no discriminant, returns 0.
    ///
    /// The stabilized version of this intrinsic is
    /// [`std::mem::discriminant`](../../std/mem/fn.discriminant.html)
    #[rustc_const_unstable(feature = "const_discriminant", issue = "69821")]
    pub fn discriminant_value<T>(v: &T) -> u64;

    /// Rust's "try catch" construct which invokes the function pointer `try_fn`
    /// with the data pointer `data`.
    ///
    /// The third argument is a function called if a panic occurs. This function
    /// takes the data pointer and a pointer to the target-specific exception
    /// object that was caught. For more information see the compiler's
    /// source as well as std's catch implementation.
    #[cfg(not(bootstrap))]
    pub fn r#try(try_fn: fn(*mut u8), data: *mut u8, catch_fn: fn(*mut u8, *mut u8)) -> i32;
    #[cfg(bootstrap)]
    pub fn r#try(f: fn(*mut u8), data: *mut u8, local_ptr: *mut u8) -> i32;

    /// Emits a `!nontemporal` store according to LLVM (see their docs).
    /// Probably will never become stable.
    pub fn nontemporal_store<T>(ptr: *mut T, val: T);

    /// See documentation of `<*const T>::offset_from` for details.
    #[rustc_const_unstable(feature = "const_ptr_offset_from", issue = "none")]
    pub fn ptr_offset_from<T>(ptr: *const T, base: *const T) -> isize;

    /// Internal hook used by Miri to implement unwinding.
    /// ICEs when encountered during non-Miri codegen.
    ///
    /// The `payload` ptr here will be exactly the one `do_catch` gets passed by `try`.
    ///
    /// Perma-unstable: do not use.
    pub fn miri_start_panic(payload: *mut u8) -> !;
}

// Some functions are defined here because they accidentally got made
// available in this module on stable. See <https://github.com/rust-lang/rust/issues/15702>.
// (`transmute` also falls into this category, but it cannot be wrapped due to the
// check that `T` and `U` have the same size.)

/// Checks whether `ptr` is properly aligned with respect to
/// `align_of::<T>()`.
pub(crate) fn is_aligned_and_not_null<T>(ptr: *const T) -> bool {
    !ptr.is_null() && ptr as usize % mem::align_of::<T>() == 0
}

/// Checks whether the regions of memory starting at `src` and `dst` of size
/// `count * size_of::<T>()` do *not* overlap.
pub(crate) fn is_nonoverlapping<T>(src: *const T, dst: *const T, count: usize) -> bool {
    let src_usize = src as usize;
    let dst_usize = dst as usize;
    let size = mem::size_of::<T>().checked_mul(count).unwrap();
    let diff = if src_usize > dst_usize { src_usize - dst_usize } else { dst_usize - src_usize };
    // If the absolute distance between the ptrs is at least as big as the size of the buffer,
    // they do not overlap.
    diff >= size
}

/// Copies `count * size_of::<T>()` bytes from `src` to `dst`. The source
/// and destination must *not* overlap.
///
/// For regions of memory which might overlap, use [`copy`] instead.
///
/// `copy_nonoverlapping` is semantically equivalent to C's [`memcpy`], but
/// with the argument order swapped.
///
/// [`copy`]: ./fn.copy.html
/// [`memcpy`]: https://en.cppreference.com/w/c/string/byte/memcpy
///
/// # Safety
///
/// Behavior is undefined if any of the following conditions are violated:
///
/// * `src` must be [valid] for reads of `count * size_of::<T>()` bytes.
///
/// * `dst` must be [valid] for writes of `count * size_of::<T>()` bytes.
///
/// * Both `src` and `dst` must be properly aligned.
///
/// * The region of memory beginning at `src` with a size of `count *
///   size_of::<T>()` bytes must *not* overlap with the region of memory
///   beginning at `dst` with the same size.
///
/// Like [`read`], `copy_nonoverlapping` creates a bitwise copy of `T`, regardless of
/// whether `T` is [`Copy`]. If `T` is not [`Copy`], using *both* the values
/// in the region beginning at `*src` and the region beginning at `*dst` can
/// [violate memory safety][read-ownership].
///
/// Note that even if the effectively copied size (`count * size_of::<T>()`) is
/// `0`, the pointers must be non-NULL and properly aligned.
///
/// [`Copy`]: ../marker/trait.Copy.html
/// [`read`]: ../ptr/fn.read.html
/// [read-ownership]: ../ptr/fn.read.html#ownership-of-the-returned-value
/// [valid]: ../ptr/index.html#safety
///
/// # Examples
///
/// Manually implement [`Vec::append`]:
///
/// ```
/// use std::ptr;
///
/// /// Moves all the elements of `src` into `dst`, leaving `src` empty.
/// fn append<T>(dst: &mut Vec<T>, src: &mut Vec<T>) {
///     let src_len = src.len();
///     let dst_len = dst.len();
///
///     // Ensure that `dst` has enough capacity to hold all of `src`.
///     dst.reserve(src_len);
///
///     unsafe {
///         // The call to offset is always safe because `Vec` will never
///         // allocate more than `isize::MAX` bytes.
///         let dst_ptr = dst.as_mut_ptr().offset(dst_len as isize);
///         let src_ptr = src.as_ptr();
///
///         // Truncate `src` without dropping its contents. We do this first,
///         // to avoid problems in case something further down panics.
///         src.set_len(0);
///
///         // The two regions cannot overlap because mutable references do
///         // not alias, and two different vectors cannot own the same
///         // memory.
///         ptr::copy_nonoverlapping(src_ptr, dst_ptr, src_len);
///
///         // Notify `dst` that it now holds the contents of `src`.
///         dst.set_len(dst_len + src_len);
///     }
/// }
///
/// let mut a = vec!['r'];
/// let mut b = vec!['u', 's', 't'];
///
/// append(&mut a, &mut b);
///
/// assert_eq!(a, &['r', 'u', 's', 't']);
/// assert!(b.is_empty());
/// ```
///
/// [`Vec::append`]: ../../std/vec/struct.Vec.html#method.append
#[doc(alias = "memcpy")]
#[stable(feature = "rust1", since = "1.0.0")]
#[inline]
pub unsafe fn copy_nonoverlapping<T>(src: *const T, dst: *mut T, count: usize) {
    extern "rust-intrinsic" {
        fn copy_nonoverlapping<T>(src: *const T, dst: *mut T, count: usize);
    }

    debug_assert!(is_aligned_and_not_null(src), "attempt to copy from unaligned or null pointer");
    debug_assert!(is_aligned_and_not_null(dst), "attempt to copy to unaligned or null pointer");
    debug_assert!(is_nonoverlapping(src, dst, count), "attempt to copy to overlapping memory");
    copy_nonoverlapping(src, dst, count)
}

/// Copies `count * size_of::<T>()` bytes from `src` to `dst`. The source
/// and destination may overlap.
///
/// If the source and destination will *never* overlap,
/// [`copy_nonoverlapping`] can be used instead.
///
/// `copy` is semantically equivalent to C's [`memmove`], but with the argument
/// order swapped. Copying takes place as if the bytes were copied from `src`
/// to a temporary array and then copied from the array to `dst`.
///
/// [`copy_nonoverlapping`]: ./fn.copy_nonoverlapping.html
/// [`memmove`]: https://en.cppreference.com/w/c/string/byte/memmove
///
/// # Safety
///
/// Behavior is undefined if any of the following conditions are violated:
///
/// * `src` must be [valid] for reads of `count * size_of::<T>()` bytes.
///
/// * `dst` must be [valid] for writes of `count * size_of::<T>()` bytes.
///
/// * Both `src` and `dst` must be properly aligned.
///
/// Like [`read`], `copy` creates a bitwise copy of `T`, regardless of
/// whether `T` is [`Copy`]. If `T` is not [`Copy`], using both the values
/// in the region beginning at `*src` and the region beginning at `*dst` can
/// [violate memory safety][read-ownership].
///
/// Note that even if the effectively copied size (`count * size_of::<T>()`) is
/// `0`, the pointers must be non-NULL and properly aligned.
///
/// [`Copy`]: ../marker/trait.Copy.html
/// [`read`]: ../ptr/fn.read.html
/// [read-ownership]: ../ptr/fn.read.html#ownership-of-the-returned-value
/// [valid]: ../ptr/index.html#safety
///
/// # Examples
///
/// Efficiently create a Rust vector from an unsafe buffer:
///
/// ```
/// use std::ptr;
///
/// # #[allow(dead_code)]
/// unsafe fn from_buf_raw<T>(ptr: *const T, elts: usize) -> Vec<T> {
///     let mut dst = Vec::with_capacity(elts);
///     dst.set_len(elts);
///     ptr::copy(ptr, dst.as_mut_ptr(), elts);
///     dst
/// }
/// ```
#[doc(alias = "memmove")]
#[stable(feature = "rust1", since = "1.0.0")]
#[inline]
pub unsafe fn copy<T>(src: *const T, dst: *mut T, count: usize) {
    extern "rust-intrinsic" {
        fn copy<T>(src: *const T, dst: *mut T, count: usize);
    }

    debug_assert!(is_aligned_and_not_null(src), "attempt to copy from unaligned or null pointer");
    debug_assert!(is_aligned_and_not_null(dst), "attempt to copy to unaligned or null pointer");
    copy(src, dst, count)
}

/// Sets `count * size_of::<T>()` bytes of memory starting at `dst` to
/// `val`.
///
/// `write_bytes` is similar to C's [`memset`], but sets `count *
/// size_of::<T>()` bytes to `val`.
///
/// [`memset`]: https://en.cppreference.com/w/c/string/byte/memset
///
/// # Safety
///
/// Behavior is undefined if any of the following conditions are violated:
///
/// * `dst` must be [valid] for writes of `count * size_of::<T>()` bytes.
///
/// * `dst` must be properly aligned.
///
/// Additionally, the caller must ensure that writing `count *
/// size_of::<T>()` bytes to the given region of memory results in a valid
/// value of `T`. Using a region of memory typed as a `T` that contains an
/// invalid value of `T` is undefined behavior.
///
/// Note that even if the effectively copied size (`count * size_of::<T>()`) is
/// `0`, the pointer must be non-NULL and properly aligned.
///
/// [valid]: ../ptr/index.html#safety
///
/// # Examples
///
/// Basic usage:
///
/// ```
/// use std::ptr;
///
/// let mut vec = vec![0u32; 4];
/// unsafe {
///     let vec_ptr = vec.as_mut_ptr();
///     ptr::write_bytes(vec_ptr, 0xfe, 2);
/// }
/// assert_eq!(vec, [0xfefefefe, 0xfefefefe, 0, 0]);
/// ```
///
/// Creating an invalid value:
///
/// ```
/// use std::ptr;
///
/// let mut v = Box::new(0i32);
///
/// unsafe {
///     // Leaks the previously held value by overwriting the `Box<T>` with
///     // a null pointer.
///     ptr::write_bytes(&mut v as *mut Box<i32>, 0, 1);
/// }
///
/// // At this point, using or dropping `v` results in undefined behavior.
/// // drop(v); // ERROR
///
/// // Even leaking `v` "uses" it, and hence is undefined behavior.
/// // mem::forget(v); // ERROR
///
/// // In fact, `v` is invalid according to basic type layout invariants, so *any*
/// // operation touching it is undefined behavior.
/// // let v2 = v; // ERROR
///
/// unsafe {
///     // Let us instead put in a valid value
///     ptr::write(&mut v as *mut Box<i32>, Box::new(42i32));
/// }
///
/// // Now the box is fine
/// assert_eq!(*v, 42);
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[inline]
pub unsafe fn write_bytes<T>(dst: *mut T, val: u8, count: usize) {
    extern "rust-intrinsic" {
        fn write_bytes<T>(dst: *mut T, val: u8, count: usize);
    }

    debug_assert!(is_aligned_and_not_null(dst), "attempt to write to unaligned or null pointer");
    write_bytes(dst, val, count)
}
