use super::waitqueue::{WaitVariable, WaitQueue, SpinMutex, try_lock_or_false};

pub struct Mutex {
    inner: SpinMutex<WaitVariable<bool>>,
}

// Implementation according to “Operating Systems: Three Easy Pieces”, chapter 28
impl Mutex {
    pub const fn new() -> Mutex {
        Mutex { inner: SpinMutex::new(WaitVariable::new(false)) }
    }

    #[inline]
    pub unsafe fn init(&mut self) {}

    #[inline]
    pub unsafe fn lock(&self) {
        let mut guard = self.inner.lock();
        if *guard.lock_var() {
            // Another thread has the lock, wait
            WaitQueue::wait(guard)
            // Another thread has passed the lock to us
        } else {
            // We are just now obtaining the lock
            *guard.lock_var_mut() = true;
        }
    }

    #[inline]
    pub unsafe fn unlock(&self) {
        let guard = self.inner.lock();
        if let Err(mut guard) = WaitQueue::notify_one(guard) {
            // No other waiters, unlock
            *guard.lock_var_mut() = false;
        } else {
            // There was a thread waiting, just pass the lock
        }
    }

    #[inline]
    pub unsafe fn try_lock(&self) -> bool {
        let mut guard = try_lock_or_false!(self.inner);
        if *guard.lock_var() {
            // Another thread has the lock
            false
        } else {
            // We are just now obtaining the lock
            *guard.lock_var_mut() = true;
            true
        }
    }

    #[inline]
    pub unsafe fn destroy(&self) {}
}
