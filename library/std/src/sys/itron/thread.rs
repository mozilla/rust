//! Thread implementation backed by μITRON tasks. Assumes `acre_tsk`,
//! `acre_dtq`, and `acre_flg` are available.
use super::{abi, error::ItronError, task, time::dur2reltims};
use crate::{
    cell::UnsafeCell,
    ffi::CStr,
    io,
    mem::{ManuallyDrop, MaybeUninit},
    process::abort,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    sys::thread_local_dtor::run_dtors,
    time::Duration,
};

pub struct Thread {
    inner: ManuallyDrop<Arc<ThreadInner>>,

    /// The ID of the underlying task.
    task: abi::ID,
}

struct ThreadInner {
    /// This field is used on thread creation to pass a closure from
    /// `Thread::new` to the created task.
    start: UnsafeCell<ManuallyDrop<Box<dyn FnOnce()>>>,

    /// A state machine. Each transition is annotated with `[...]` in the
    /// source code.
    ///
    /// ```text
    ///                         [DYING-ATTACHED]
    ///
    ///          LIFECYCLE_INIT   ----------->  LIFECYCLE_DYING
    ///
    ///                |                               |
    /// [DETACH-LIVE]  |                               |  [DETACH-DYING]
    ///                v                               v
    ///
    ///        LIFECYCLE_DETACHED  --------->  LIFECYCLE_DYING |
    ///                                       LIFECYCLE_DETACHED
    ///                         [DYING-DETACHED]
    /// ```
    lifecycle: AtomicUsize,

    /// The ID of the eventflag object. The eventflag object is set when the
    /// task's execution is complete and the task is safe to delete.
    death_flag: Eventflag,
}

// Safety: The only `!Sync` field, `ThreadInner::start`, is only touched by
//         the task represented by `ThreadInner`.
unsafe impl Sync for ThreadInner {}

const LIFECYCLE_INIT: usize = 0;
const LIFECYCLE_DYING: usize = 1;
const LIFECYCLE_DETACHED: usize = 2;

pub const DEFAULT_MIN_STACK_SIZE: usize = 1024 * crate::mem::size_of::<usize>();

impl Thread {
    /// # Safety
    ///
    /// See `thread::Builder::spawn_unchecked` for safety requirements.
    pub unsafe fn new(stack: usize, p: Box<dyn FnOnce()>) -> io::Result<Thread> {
        // Inherit the current task's priority
        let current_task = task::current_task_id().map_err(|e| e.as_io_error())?;
        let priority = task::task_priority(current_task).map_err(|e| e.as_io_error())?;

        // Initialize the task collector ahead-of-time instead of doing it
        // lazily and escalating any errors occurred to panics
        detached_task_collector::init()?;

        let death_flag = Eventflag(
            // Safety: The passed pointer is valid
            ItronError::err_if_negative(unsafe {
                abi::acre_flg(&abi::T_CFLG { flgatr: 0, iflgptn: 0 })
            })
            .map_err(|e| e.as_io_error())?,
        );

        let inner = Arc::new(ThreadInner {
            start: UnsafeCell::new(ManuallyDrop::new(p)),
            lifecycle: AtomicUsize::new(LIFECYCLE_INIT),
            death_flag,
        });

        unsafe extern "C" fn trampoline(exinf: isize) {
            // Safety: The ownership was transferred to us
            let inner = unsafe { Arc::from_raw(exinf as *const ThreadInner) };

            // Safety: Since `trampoline` is called only once for each
            //         `ThreadInner` and only `trampoline` touches `start`,
            //         `start` contains contents and is safe to mutably borrow.
            let p = unsafe { ManuallyDrop::take(&mut *inner.start.get()) };
            p();

            // Fix the current thread's state just in case
            // Safety: Not really unsafe
            let _ = unsafe { abi::unl_cpu() };

            // Run TLS destructors now because they are not
            // called automatically for terminated tasks.
            unsafe { run_dtors() };

            // Disable preemption throughout the following code section. This
            // prevents `Thread::drop` from getting stuck waiting for
            // `death_flag` to be set.
            // Safety: Not really unsafe
            let _ = unsafe { abi::dis_dsp() };

            if inner.lifecycle.fetch_add(LIFECYCLE_DYING, Ordering::Release) == LIFECYCLE_DETACHED {
                // [DYING-DETACHED]
                // No one will ever join, so ask the collector task to delete the task
                drop(inner);

                // Revert the effect of `dis_dsp` because `snd_dtq` would fail
                // with `E_CTX` otherwise.
                // Safety: Not really unsafe
                unsafe { abi::ena_dsp() };

                let current_task = task::current_task_id().unwrap_or_else(|_| abort());

                // Safety: There are no pinned references to the stack
                unsafe { detached_task_collector::request_terminate_and_delete_task(current_task) };
            } else {
                // [DYING-ATTACHED]
                // The task is still attached, so let the joiner delete this task.
                //
                // `death_flag` is guaranteed to exist until we set it because
                // the joiner is supposed to wait on `death_flag` before dropping
                // the `ThreadInner`.
                let death_flag = inner.death_flag.0;
                drop(inner);

                // Set `death_flag`
                ItronError::err_if_negative(unsafe { abi::set_flg(death_flag, 1) })
                    .unwrap_or_else(|_| abort());
            }

            // The last statement (`request_terminate_and_delete_task` or
            // `set_flg`) marks this task safe for deletion, so any code
            // that follows is not guaranteed to execute. This is why we dropped
            // `inner` earlier.
        }

        let inner_ptr = Arc::into_raw(Arc::clone(&inner));

        let new_task = ItronError::err_if_negative(unsafe {
            abi::acre_tsk(&abi::T_CTSK {
                // Activate this task immediately
                tskatr: abi::TA_ACT,
                // Move `inner_ptr` to this task
                exinf: inner_ptr as abi::EXINF,
                // The entry point
                task: Some(trampoline),
                itskpri: priority,
                stksz: stack,
                // Let the kernel allocate the stack,
                stk: crate::ptr::null_mut(),
            })
        })
        .map_err(|e| {
            // Safety: The task could not be created, so we still own `inner_ptr`
            let _ = unsafe { Arc::from_raw(inner_ptr) };
            e
        })
        .map_err(|e| e.as_io_error())?;

        Ok(Self { inner: ManuallyDrop::new(inner), task: new_task })
    }

    pub fn yield_now() {
        ItronError::err_if_negative(unsafe { abi::rot_rdq(abi::TPRI_SELF) })
            .expect("rot_rdq failed");
    }

    pub fn set_name(_name: &CStr) {
        // nope
    }

    pub fn sleep(dur: Duration) {
        for timeout in dur2reltims(dur) {
            ItronError::err_if_negative(unsafe { abi::dly_tsk(timeout) }).expect("dly_tsk failed");
        }
    }

    pub fn join(mut self) {
        // Safety: We haven't called `join_inner` before for this `Thread`
        unsafe { self.join_inner() };

        // Skip the destructor, but drop `inner` correctly
        // Safety: The contents of `self.inner` will not be accessed hereafter
        let inner = unsafe { ManuallyDrop::take(&mut self.inner) };
        crate::mem::forget(self);
        drop(inner);
    }

    /// Wait until `death_flag` is set and delete the task.
    ///
    /// # Safety
    ///
    /// This method can be called only once for each `Thread`.
    unsafe fn join_inner(&mut self) {
        ItronError::err_if_negative(unsafe {
            abi::wai_flg(
                self.inner.death_flag.0,
                1,
                abi::TWF_ORW,
                // unused out parameter
                MaybeUninit::uninit().as_mut_ptr(),
            )
        })
        .expect("wai_flg failed");

        // Terminate and delete the task
        // Safety: `self.task` still represents a task we own (because this
        //         method is called only once for each `Thread`). The task
        //         indicated that it's safe to delete by setting `death_flag`.
        unsafe { terminate_and_delete_task(self.task) };
    }
}

impl Drop for Thread {
    fn drop(&mut self) {
        if self.inner.lifecycle.fetch_add(LIFECYCLE_DETACHED, Ordering::Relaxed) == LIFECYCLE_DYING
        {
            // [DETACH-DYING]
            // The task has already decided that the joiner should
            // delete the task.

            // Safety: We haven't called `join_inner` before for this `Thread`
            unsafe { self.join_inner() };
        } else {
            // [DETACH-LIVE]
            // When the time comes, the task will figure out that no one will
            // ever join it
        }
    }
}

pub mod guard {
    pub type Guard = !;
    pub unsafe fn current() -> Option<Guard> {
        None
    }
    pub unsafe fn init() -> Option<Guard> {
        None
    }
}

/// Terminate and delete the specified task.
///
/// This function will panic if `id` is the calling task.
///
/// # Safety
///
/// The task must be safe to terminate. This is in general not true
/// because there might be pinned references to the task's stack.
unsafe fn terminate_and_delete_task(deleted_task: abi::ID) {
    // Terminate the task
    // Safety: Upheld by the caller
    ItronError::err_if_negative(unsafe { abi::ter_tsk(deleted_task) })
        .map(|_| ())
        .or_else(|e| {
            if e.as_raw() == abi::E_OBJ {
                // Indicates the task is already dormant, ignore it
                Ok(())
            } else {
                Err(e)
            }
        })
        .expect("ter_tsk failed");

    // Delete the task
    // Safety: Upheld by the caller
    ItronError::err_if_negative(unsafe { abi::del_tsk(deleted_task) }).expect("del_tsk failed");
}

/// Smart pointer for eventflag objects.
struct Eventflag(abi::ID);

impl Drop for Eventflag {
    fn drop(&mut self) {
        unsafe { abi::del_flg(self.0) };
    }
}

/// The task to clean up detached tasks
#[cfg(not(target_os = "solid-asp3"))]
mod detached_task_collector {
    use super::*;
    use std::lazy::SyncOnceCell;

    /// Smart pointer for data queue objects.
    struct Dataqueue(abi::ID);

    impl Drop for Dataqueue {
        fn drop(&mut self) {
            unsafe { abi::del_dtq(self.0) };
        }
    }

    static DELETION_QUEUE: SyncOnceCell<Dataqueue> = SyncOnceCell::new();

    /// Tentative value
    const TASK_GC_TASK_STACK_SIZE: usize = 2048;

    pub fn init() -> io::Result<()> {
        DELETION_QUEUE
            .get_or_try_init(|| {
                // Safety: Passed pointers are valid
                let deletion_queue = Dataqueue(
                    ItronError::err_if_negative(unsafe {
                        abi::acre_dtq(&abi::T_CDTQ {
                            // Prioritize higher-priority senders
                            dtqatr: abi::TA_TPRI,
                            // Up to one element in the queue
                            dtqcnt: 1,
                            // Let the kernel allocate the storage
                            dtqmb: crate::ptr::null_mut(),
                        })
                    })
                    .map_err(|e| e.as_io_error())?,
                );

                // Start the detached task collector task
                // Safety: Passed pointers are valid
                ItronError::err_if_negative(unsafe {
                    abi::acre_tsk(&abi::T_CTSK {
                        // Activate this task immediately
                        tskatr: abi::TA_ACT,
                        // Pass the deletion queue to this task
                        exinf: deletion_queue.0 as abi::EXINF,
                        // The entry point
                        task: Some(task_gc_task),
                        // Highest priority. This task spends most of the time in
                        // the kernel, so it will block other tasks anyway.
                        // Choosing a lower priority is actually harmful because it
                        // can lead to unbounded priority inversion and consequent
                        // resource starvation. (Dataqueues have no priority
                        // protection mechanisms.)
                        itskpri: 1,
                        stksz: TASK_GC_TASK_STACK_SIZE,
                        stk: crate::ptr::null_mut(),
                    })
                })
                .map_err(|e| e.as_io_error())?;

                Ok(deletion_queue)
            })
            .map(|_| {})
    }

    /// Request the task collector to terminate and delete the specified task,
    /// which must be the calling task.
    ///
    /// # Safety
    ///
    /// The task must be safe to terminate. This is in general not true
    /// because there might be pinned references to the task's stack.
    ///
    /// `deleted_task` must refer to the current task.
    pub unsafe fn request_terminate_and_delete_task(deleted_task: abi::ID) {
        let deletion_queue = DELETION_QUEUE.get().unwrap().0;

        // Send `deleted_task` to `queue`
        //
        // WARNING: This process must be atomic! To be precise, no other tasks
        // must be able to preempt this process. If there were such tasks,
        // they would be able to stall the GC of higher-priority tasks
        // (unbounded priority inversion), causing resource starvation.
        // Safety: Not really unsafe
        ItronError::err_if_negative(unsafe { abi::snd_dtq(deletion_queue, deleted_task as isize) })
            .unwrap_or_else(|_| abort());
    }

    extern "C" fn task_gc_task(exinf: isize) {
        let deletion_queue = exinf as abi::ID;

        loop {
            let payload = unsafe {
                let mut out = MaybeUninit::uninit();
                ItronError::err_if_negative(abi::rcv_dtq(deletion_queue, out.as_mut_ptr()))
                    .expect("rcv_dtq failed");
                out.assume_init()
            };
            let deleted_task = payload as abi::ID;

            // Terminate and delete the task
            // Safety: Upheld by the caller of `request_terminate_and_delete_task`
            unsafe { terminate_and_delete_task(deleted_task) };
        }
    }
}

#[cfg(target_os = "solid-asp3")]
mod detached_task_collector {
    use super::*;
    use crate::sys::solid::abi::exd_tsk;

    #[inline]
    pub fn init() -> io::Result<()> {
        Ok(())
    }

    /// Request the task collector to terminate and delete the specified task,
    /// which must be the calling task.
    ///
    /// # Safety
    ///
    /// The task must be safe to terminate. This is in general not true
    /// because there might be pinned references to the task's stack.
    ///
    /// `deleted_task` must refer to the current task.
    pub unsafe fn request_terminate_and_delete_task(_deleted_task: abi::ID) {
        ItronError::err_if_negative(unsafe { exd_tsk() }).unwrap_or_else(|_| abort());
    }
}
