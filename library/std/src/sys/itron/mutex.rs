//! Mutex implementation backed by μITRON mutexes. Assumes `acre_mtx` and
//! `TA_INHERIT` are available.
use super::{abi, error::ItronError, spin::SpinIdOnceCell};
use crate::cell::UnsafeCell;

pub struct Mutex {
    /// The ID of the underlying mutex object
    mtx: SpinIdOnceCell<()>,
}

pub type MovableMutex = Mutex;

fn new_mtx() -> abi::ID {
    ItronError::err_if_negative(unsafe {
        abi::acre_mtx(&abi::T_CMTX {
            // Priority inheritance mutex
            mtxatr: abi::TA_INHERIT,
            // Unused
            ceilpri: 0,
        })
    })
    .expect("acre_mtx failed")
}

impl Mutex {
    pub const fn new() -> Mutex {
        Mutex { mtx: SpinIdOnceCell::new() }
    }

    pub unsafe fn init(&mut self) {
        // Initialize `self.mtx` eagerly
        unsafe { self.mtx.set_unchecked((new_mtx(), ())) };
    }

    /// Get the inner mutex's ID, which is lazily created.
    fn raw(&self) -> abi::ID {
        let Ok((id, _)) = self.mtx.get_or_try_init(|| Ok::<_, !>((new_mtx(), ())));
        id
    }

    pub unsafe fn lock(&self) {
        let mtx = self.raw();
        ItronError::err_if_negative(unsafe { abi::loc_mtx(mtx) }).expect("loc_mtx failed");
    }

    pub unsafe fn unlock(&self) {
        let mtx = unsafe { self.mtx.get_unchecked().0 };
        ItronError::err_if_negative(unsafe { abi::unl_mtx(mtx) }).expect("unl_mtx failed");
    }

    pub unsafe fn try_lock(&self) -> bool {
        let mtx = self.raw();
        ItronError::err_if_negative(unsafe { abi::ploc_mtx(mtx) })
            .map(|_| true)
            .or_else(|e| if e.as_raw() != abi::E_TMOUT { Err(e) } else { Ok(false) })
            .expect("ploc_mtx failed")
    }

    pub unsafe fn destroy(&self) {
        if let Some(mtx) = self.mtx.get().map(|x| x.0) {
            ItronError::err_if_negative(unsafe { abi::del_mtx(mtx) }).expect("del_mtx failed");
        }
    }
}

pub(super) struct MutexGuard<'a>(&'a Mutex);

impl<'a> MutexGuard<'a> {
    #[inline]
    pub(super) fn lock(x: &'a Mutex) -> Self {
        unsafe { x.lock() };
        Self(x)
    }
}

impl Drop for MutexGuard<'_> {
    #[inline]
    fn drop(&mut self) {
        unsafe { self.0.unlock() };
    }
}

// All empty stubs because this platform does not yet support threads, so lock
// acquisition always succeeds.
pub struct ReentrantMutex {
    /// The ID of the underlying mutex object
    mtx: abi::ID,
    /// The lock count.
    count: UnsafeCell<usize>,
}

unsafe impl Send for ReentrantMutex {}
unsafe impl Sync for ReentrantMutex {}

impl ReentrantMutex {
    pub const unsafe fn uninitialized() -> ReentrantMutex {
        ReentrantMutex { mtx: 0, count: UnsafeCell::new(0) }
    }

    pub unsafe fn init(&mut self) {
        self.mtx = ItronError::err_if_negative(unsafe {
            abi::acre_mtx(&abi::T_CMTX {
                // Priority inheritance mutex
                mtxatr: abi::TA_INHERIT,
                // Unused
                ceilpri: 0,
            })
        })
        .expect("acre_mtx failed");
    }

    pub unsafe fn lock(&self) {
        let is_recursive = ItronError::err_if_negative(unsafe { abi::loc_mtx(self.mtx) })
            .map(|_| false)
            .or_else(|e| if e.as_raw() != abi::E_OBJ { Err(e) } else { Ok(true) })
            .expect("loc_mtx failed");

        if is_recursive {
            unsafe {
                let count = &mut *self.count.get();
                if let Some(new_count) = count.checked_add(1) {
                    *count = new_count;
                } else {
                    // counter overflow
                    crate::intrinsics::abort();
                }
            }
        }
    }

    pub unsafe fn unlock(&self) {
        unsafe {
            let count = &mut *self.count.get();
            if *count > 0 {
                *count -= 1;
                return;
            }
        }

        ItronError::err_if_negative(unsafe { abi::unl_mtx(self.mtx) }).expect("unl_mtx failed");
    }

    pub unsafe fn try_lock(&self) -> bool {
        let is_recursive = ItronError::err_if_negative(unsafe { abi::ploc_mtx(self.mtx) })
            .map(|_| Some(false))
            .or_else(|e| match e.as_raw() {
                abi::E_TMOUT => Ok(None),
                abi::E_OBJ => Ok(Some(true)),
                e => Err(e),
            })
            .expect("ploc_mtx failed");

        match is_recursive {
            // Locked by another thread
            None => false,

            // Recursive lock by the current thread
            Some(true) => unsafe {
                let count = &mut *self.count.get();
                if let Some(new_count) = count.checked_add(1) {
                    *count = new_count;
                } else {
                    // counter overflow
                    crate::intrinsics::abort();
                }
                true
            },

            // Top-level lock by the current thread
            Some(false) => true,
        }
    }

    pub unsafe fn destroy(&self) {
        ItronError::err_if_negative(unsafe { abi::del_mtx(self.mtx) }).expect("del_mtx failed");
    }
}
