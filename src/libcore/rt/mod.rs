// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/*! The Rust Runtime, including the task scheduler and I/O

The `rt` module provides the private runtime infrastructure necessary
to support core language features like the exchange and local heap,
the garbage collector, logging, local data and unwinding. It also
implements the default task scheduler and task model. Initialization
routines are provided for setting up runtime resources in common
configurations, including that used by `rustc` when generating
executables.

It is intended that the features provided by `rt` can be factored in a
way such that the core library can be built with different 'profiles'
for different use cases, e.g. excluding the task scheduler. A number
of runtime features though are critical to the functioning of the
language and an implementation must be provided regardless of the
execution environment.

Of foremost importance is the global exchange heap, in the module
`global_heap`. Very little practical Rust code can be written without
access to the global heap. Unlike most of `rt` the global heap is
truly a global resource and generally operates independently of the
rest of the runtime.

All other runtime features are 'local', either thread-local or
task-local.  Those critical to the functioning of the language are
defined in the module `local_services`. Local services are those which
are expected to be available to Rust code generally but rely on
thread- or task-local state. These currently include the local heap,
the garbage collector, local storage, logging and the stack unwinder.
Local services are primarily implemented for tasks, but may also
be implemented for use outside of tasks.

The relationship between `rt` and the rest of the core library is
not entirely clear yet and some modules will be moving into or
out of `rt` as development proceeds.

Several modules in `core` are clients of `rt`:

* `core::task` - The user-facing interface to the Rust task model.
* `core::task::local_data` - The interface to local data.
* `core::gc` - The garbage collector.
* `core::unstable::lang` - Miscellaneous lang items, some of which rely on `core::rt`.
* `core::condition` - Uses local data.
* `core::cleanup` - Local heap destruction.
* `core::io` - In the future `core::io` will use an `rt` implementation.
* `core::logging`
* `core::pipes`
* `core::comm`
* `core::stackwalk`

*/

#[doc(hidden)];

use libc::c_char;
use ptr::Ptr;

/// The global (exchange) heap.
pub mod global_heap;

/// The Scheduler and Task types.
mod sched;

/// Thread-local access to the current Scheduler.
pub mod local_sched;

/// Synchronous I/O.
#[path = "io/mod.rs"]
pub mod io;

/// Thread-local implementations of language-critical runtime features like @.
pub mod local_services;

/// The EventLoop and internal synchronous I/O interface.
mod rtio;

/// libuv and default rtio implementation.
#[path = "uv/mod.rs"]
pub mod uv;

// FIXME #5248: The import in `sched` doesn't resolve unless this is pub!
/// Bindings to pthread/windows thread-local storage.
pub mod thread_local_storage;

/// A parallel work-stealing dequeue.
mod work_queue;

/// Stack segments and caching.
mod stack;

/// CPU context swapping.
mod context;

/// Bindings to system threading libraries.
mod thread;

/// The runtime configuration, read from environment variables
pub mod env;

/// The local, managed heap
mod local_heap;

/// The Logger trait and implementations
pub mod logging;

/// Tools for testing the runtime
#[cfg(test)]
pub mod test;

/// Reference counting
pub mod rc;

/// A simple single-threaded channel type for passing buffered data between
/// scheduler and task context
pub mod tube;

/// Set up a default runtime configuration, given compiler-supplied arguments.
///
/// This is invoked by the `start` _language item_ (unstable::lang) to
/// run a Rust executable.
///
/// # Arguments
///
/// * `main` - A C-abi function that takes no arguments and returns `c_void`.
///   It is a wrapper around the user-defined `main` function, and will be run
///   in a task.
/// * `argc` & `argv` - The argument vector. On Unix this information is used
///   by os::args.
/// * `crate_map` - Runtime information about the executing crate, mostly for logging
///
/// # Return value
///
/// The return value is used as the process return code. 0 on success, 101 on error.
pub fn start(main: *u8, _argc: int, _argv: **c_char, _crate_map: *u8) -> int {

    use self::sched::{Scheduler, Task};
    use self::uv::uvio::UvEventLoop;
    use sys::Closure;
    use ptr;
    use cast;

    let loop_ = ~UvEventLoop::new();
    let mut sched = ~Scheduler::new(loop_);

    let main_task = ~do Task::new(&mut sched.stack_pool) {

        unsafe {
            // `main` is an `fn() -> ()` that doesn't take an environment
            // XXX: Could also call this as an `extern "Rust" fn` once they work
            let main = Closure {
                code: main as *(),
                env: ptr::null(),
            };
            let mainfn: &fn() = cast::transmute(main);

            mainfn();
        }
    };

    sched.task_queue.push_back(main_task);
    sched.run();

    return 0;
}

/// Possible contexts in which Rust code may be executing.
/// Different runtime services are available depending on context.
/// Mostly used for determining if we're using the new scheduler
/// or the old scheduler.
#[deriving(Eq)]
pub enum RuntimeContext {
    // Only the exchange heap is available
    GlobalContext,
    // The scheduler may be accessed
    SchedulerContext,
    // Full task services, e.g. local heap, unwinding
    TaskContext,
    // Running in an old-style task
    OldTaskContext
}

/// Determine the current RuntimeContext
pub fn context() -> RuntimeContext {

    use task::rt::rust_task;
    use self::sched::local_sched;

    // XXX: Hitting TLS twice to check if the scheduler exists
    // then to check for the task is not good for perf
    if unsafe { rust_try_get_task().is_not_null() } {
        return OldTaskContext;
    } else {
        if local_sched::exists() {
            let context = ::cell::empty_cell();
            do local_sched::borrow |sched| {
                if sched.in_task_context() {
                    context.put_back(TaskContext);
                } else {
                    context.put_back(SchedulerContext);
                }
            }
            return context.take();
        } else {
            return GlobalContext;
        }
    }

    pub extern {
        #[rust_stack]
        fn rust_try_get_task() -> *rust_task;
    }
}

#[test]
fn test_context() {
    use unstable::run_in_bare_thread;
    use self::sched::{local_sched, Task};
    use rt::uv::uvio::UvEventLoop;
    use cell::Cell;

    assert!(context() == OldTaskContext);
    do run_in_bare_thread {
        assert!(context() == GlobalContext);
        let mut sched = ~UvEventLoop::new_scheduler();
        let task = ~do Task::new(&mut sched.stack_pool) {
            assert!(context() == TaskContext);
            let sched = local_sched::take();
            do sched.deschedule_running_task_and_then() |task| {
                assert!(context() == SchedulerContext);
                let task = Cell(task);
                do local_sched::borrow |sched| {
                    sched.task_queue.push_back(task.take());
                }
            }
        };
        sched.task_queue.push_back(task);
        sched.run();
    }
}
