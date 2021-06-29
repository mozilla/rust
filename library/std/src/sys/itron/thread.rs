//! Thread implementation backed by μITRON tasks. Assumes `acre_tsk`,
//! `acre_dtq`, and `acre_flg` are available.
use super::{
    abi,
    error::{expect_success, expect_success_aborting, ItronError},
    task,
    time::dur2reltims,
};
use crate::{
    cell::UnsafeCell,
    convert::TryFrom,
    ffi::CStr,
    hint, io,
    mem::ManuallyDrop,
    sync::atomic::{AtomicUsize, Ordering},
    sys::thread_local_dtor::run_dtors,
    time::Duration,
};

pub struct Thread {
    inner: ManuallyDrop<Box<ThreadInner>>,

    /// The ID of the underlying task.
    task: abi::ID,
}

/// State data shared between a parent thread and child thread. It's dropped on
/// a transition to one of the final states.
struct ThreadInner {
    /// This field is used on thread creation to pass a closure from
    /// `Thread::new` to the created task.
    start: UnsafeCell<ManuallyDrop<Box<dyn FnOnce()>>>,

    /// A state machine. Each transition is annotated with `[...]` in the
    /// source code.
    ///
    /// ```text
    ///
    ///    <P>: parent, <C>: child, (?): don't-care
    ///
    ///       DETACHED (-1)  -------------------->  EXITED (?)
    ///                        <C>finish/exd_tsk
    ///          ^
    ///          |
    ///          | <P>detach
    ///          |
    ///
    ///       INIT (0)  ----------------------->  FINISHED (-1)
    ///                        <C>finish
    ///          |                                    |
    ///          | <P>join/slp_tsk                    | <P>join/del_tsk
    ///          |                                    | <P>detach/del_tsk
    ///          v                                    v
    ///
    ///       JOINING                              JOINED (?)
    ///     (parent_tid)
    ///                                            ^
    ///             \                             /
    ///              \  <C>finish/wup_tsk        / <P>slp_tsk-complete/ter_tsk
    ///               \                         /                      & del_tsk
    ///                \                       /
    ///                 '--> JOIN_FINALIZE ---'
    ///                          (-1)
    ///
    lifecycle: AtomicUsize,
}

// Safety: The only `!Sync` field, `ThreadInner::start`, is only touched by
//         the task represented by `ThreadInner`.
unsafe impl Sync for ThreadInner {}

const LIFECYCLE_INIT: usize = 0;
const LIFECYCLE_FINISHED: usize = usize::MAX;
const LIFECYCLE_DETACHED: usize = usize::MAX;
const LIFECYCLE_JOIN_FINALIZE: usize = usize::MAX;
const LIFECYCLE_DETACHED_OR_JOINED: usize = usize::MAX;
const LIFECYCLE_EXITED_OR_FINISHED_OR_JOIN_FINALIZE: usize = usize::MAX;
// there's no single value for `JOINING`

pub const DEFAULT_MIN_STACK_SIZE: usize = 1024 * crate::mem::size_of::<usize>();

impl Thread {
    /// # Safety
    ///
    /// See `thread::Builder::spawn_unchecked` for safety requirements.
    pub unsafe fn new(stack: usize, p: Box<dyn FnOnce()>) -> io::Result<Thread> {
        // Inherit the current task's priority
        let current_task = task::try_current_task_id().map_err(|e| e.as_io_error())?;
        let priority = task::try_task_priority(current_task).map_err(|e| e.as_io_error())?;

        // Initialize the task collector ahead-of-time instead of doing it
        // lazily and escalating any errors occurred to panics
        detached_task_collector::init()?;

        let inner = Box::new(ThreadInner {
            start: UnsafeCell::new(ManuallyDrop::new(p)),
            lifecycle: AtomicUsize::new(LIFECYCLE_INIT),
        });

        unsafe extern "C" fn trampoline(exinf: isize) {
            // Safety: `ThreadInner` is alive at this point
            let inner = unsafe { &*(exinf as *const ThreadInner) };

            // Safety: Since `trampoline` is called only once for each
            //         `ThreadInner` and only `trampoline` touches `start`,
            //         `start` contains contents and is safe to mutably borrow.
            let p = unsafe { ManuallyDrop::take(&mut *inner.start.get()) };
            p();

            // Fix the current thread's state just in case, so that the
            // destructors won't abort
            // Safety: Not really unsafe
            let _ = unsafe { abi::unl_cpu() };
            let _ = unsafe { abi::ena_dsp() };

            // Run TLS destructors now because they are not
            // called automatically for terminated tasks.
            unsafe { run_dtors() };

            let old_lifecycle = inner
                .lifecycle
                .swap(LIFECYCLE_EXITED_OR_FINISHED_OR_JOIN_FINALIZE, Ordering::Release);

            match old_lifecycle {
                LIFECYCLE_DETACHED => {
                    // [DETACHED → EXITED]
                    // No one will ever join, so we'll ask the collector task to
                    // delete the task.

                    // In this case, `inner`'s ownership has been moved to us,
                    // And we are responsible for dropping it. The acquire
                    // ordering is not necessary because the parent thread made
                    // no memory acccess needing synchronization since the call
                    // to `acre_tsk`.
                    // Safety: See above.
                    let _ = unsafe { Box::from_raw(inner as *const _ as *mut ThreadInner) };

                    let current_task = task::current_task_id_aborting();

                    // Safety: There are no pinned references to the stack
                    unsafe {
                        detached_task_collector::request_terminate_and_delete_task(current_task)
                    };
                }
                LIFECYCLE_INIT => {
                    // [INIT → FINISHED]
                    // The parent hasn't decided whether to join or detach this
                    // thread yet. Whichever option the parent chooses,
                    // it'll have to delete this task.
                    // Since the parent might drop `*inner` as soon as it sees
                    // `FINISHED`, the release ordering must be used in the
                    // above `swap` call.
                }
                parent_tid => {
                    // Since the parent might drop `*inner` and terminate us as
                    // soon as it sees `JOIN_FINALIZE`, the release ordering
                    // must be used in the above `swap` call.

                    // [JOINING → JOIN_FINALIZE]
                    // Wake up the parent task.
                    expect_success(
                        unsafe {
                            let mut er = abi::wup_tsk(parent_tid as _);
                            if er == abi::E_QOVR {
                                // `E_QOVR` indicates there's already
                                // a parking token
                                er = abi::E_OK;
                            }
                            er
                        },
                        &"wup_tsk",
                    );
                }
            }
        }

        let inner_ptr = (&*inner) as *const ThreadInner;

        let new_task = ItronError::err_if_negative(unsafe {
            abi::acre_tsk(&abi::T_CTSK {
                // Activate this task immediately
                tskatr: abi::TA_ACT,
                exinf: inner_ptr as abi::EXINF,
                // The entry point
                task: Some(trampoline),
                itskpri: priority,
                stksz: stack,
                // Let the kernel allocate the stack,
                stk: crate::ptr::null_mut(),
            })
        })
        .map_err(|e| e.as_io_error())?;

        Ok(Self { inner: ManuallyDrop::new(inner), task: new_task })
    }

    pub fn yield_now() {
        expect_success(unsafe { abi::rot_rdq(abi::TPRI_SELF) }, &"rot_rdq");
    }

    pub fn set_name(_name: &CStr) {
        // nope
    }

    pub fn sleep(dur: Duration) {
        for timeout in dur2reltims(dur) {
            expect_success(unsafe { abi::dly_tsk(timeout) }, &"dly_tsk");
        }
    }

    pub fn join(mut self) {
        let inner = &*self.inner;
        // Get the current task ID. Panicking here would cause a resource leak,
        // so just abort on failure.
        let current_task = task::current_task_id_aborting();
        debug_assert!(usize::try_from(current_task).is_ok());
        debug_assert_ne!(current_task as usize, LIFECYCLE_INIT);
        debug_assert_ne!(current_task as usize, LIFECYCLE_DETACHED);

        let current_task = current_task as usize;

        match inner.lifecycle.swap(current_task, Ordering::Acquire) {
            LIFECYCLE_INIT => {
                // [INIT → JOINING]
                // The child task will transition the state to `JOIN_FINALIZE`
                // and wake us up.
                loop {
                    expect_success_aborting(unsafe { abi::slp_tsk() }, &"slp_tsk");
                    // To synchronize with the child task's memory accesses to
                    // `inner` up to the point of the assignment of
                    // `JOIN_FINALIZE`, `Ordering::Acquire` must be used for the
                    // `load`.
                    if inner.lifecycle.load(Ordering::Acquire) == LIFECYCLE_JOIN_FINALIZE {
                        break;
                    }
                }

                // [JOIN_FINALIZE → JOINED]
            }
            LIFECYCLE_FINISHED => {
                // [FINISHED → JOINED]
                // To synchronize with the child task's memory accesses to
                // `inner` up to the point of the assignment of `FINISHED`,
                // `Ordering::Acquire` must be used for the above `swap` call`.
            }
            _ => unsafe { hint::unreachable_unchecked() },
        }

        // Terminate and delete the task
        // Safety: `self.task` still represents a task we own (because this
        //         method or `detach_inner` is called only once for each
        //         `Thread`). The task indicated that it's safe to delete by
        //         entering the `FINISHED` or `JOIN_FINALIZE` state.
        unsafe { terminate_and_delete_task(self.task) };

        // In either case, we are responsible for dropping `inner`.
        // Safety: The contents of `self.inner` will not be accessed hereafter
        let _inner = unsafe { ManuallyDrop::take(&mut self.inner) };

        // Skip the destructor (because it would attempt to detach the thread)
        crate::mem::forget(self);
    }
}

impl Drop for Thread {
    fn drop(&mut self) {
        // Detach the thread.
        match self.inner.lifecycle.swap(LIFECYCLE_DETACHED_OR_JOINED, Ordering::Acquire) {
            LIFECYCLE_INIT => {
                // [INIT → DETACHED]
                // When the time comes, the child will figure out that no
                // one will ever join it.
                // The ownership of `self.inner` is moved to the child thread.
                // However, the release ordering is not necessary because we
                // made no memory acccess needing synchronization since the call
                // to `acre_tsk`.
            }
            LIFECYCLE_FINISHED => {
                // [FINISHED → JOINED]
                // The task has already decided that we should delete the task.
                // To synchronize with the child task's memory accesses to
                // `inner` up to the point of the assignment of `FINISHED`,
                // the acquire ordering is required for the above `swap` call.

                // Terminate and delete the task
                // Safety: `self.task` still represents a task we own (because
                //         this method or `join_inner` is called only once for
                //         each `Thread`). The task  indicated that it's safe to
                //         delete by entering the `FINISHED` state.
                unsafe { terminate_and_delete_task(self.task) };

                // Wwe are responsible for dropping `inner`.
                // Safety: The contents of `self.inner` will not be accessed
                //         hereafter
                unsafe { ManuallyDrop::drop(&mut self.inner) };
            }
            _ => unsafe { hint::unreachable_unchecked() },
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
/// This function will abort if `deleted_task` refers to the calling task.
///
/// # Safety
///
/// The task must be safe to terminate. This is in general not true
/// because there might be pinned references to the task's stack.
unsafe fn terminate_and_delete_task(deleted_task: abi::ID) {
    // Terminate the task
    // Safety: Upheld by the caller
    match unsafe { abi::ter_tsk(deleted_task) } {
        // Indicates the task is already dormant, ignore it
        abi::E_OBJ => {}
        er => {
            expect_success_aborting(er, &"ter_tsk");
        }
    }

    // Delete the task
    // Safety: Upheld by the caller
    expect_success_aborting(unsafe { abi::del_tsk(deleted_task) }, &"del_tsk");
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
        expect_success_aborting(
            unsafe { abi::snd_dtq(deletion_queue, deleted_task as isize) },
            &"snd_dtq",
        );
    }

    extern "C" fn task_gc_task(exinf: isize) {
        let deletion_queue = exinf as abi::ID;

        loop {
            let payload = unsafe {
                let mut out = MaybeUninit::uninit();
                expect_success_aborting(abi::rcv_dtq(deletion_queue, out.as_mut_ptr()), "rcv_dtq");
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
        expect_success_aborting(unsafe { exd_tsk() }, &"exd_tsk");
    }
}
