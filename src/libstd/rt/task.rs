// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Language-level runtime services that should reasonably expected
//! to be available 'everywhere'. Local heaps, GC, unwinding,
//! local storage, and logging. Even a 'freestanding' Rust would likely want
//! to implement this.

use borrow;
use cast::transmute;
use cleanup;
use libc::{c_void, uintptr_t};
use ptr;
use prelude::*;
use option::{Option, Some, None};
use rt::local::Local;
use rt::logging::StdErrLogger;
use super::local_heap::LocalHeap;
use rt::sched::{Scheduler, SchedHandle};
use rt::join_latch::JoinLatch;
use rt::stack::{StackSegment, StackPool};
use rt::context::Context;
use cell::Cell;

// The Task struct represents all state associated with a rust
// task. There are at this point two primary "subtypes" of task,
// however instead of using a subtype we just have a "task_type" field
// in the struct. This contains a pointer to another struct that holds
// the type-specific state.

pub struct Task {
    heap: LocalHeap,
    gc: GarbageCollector,
    storage: LocalStorage,
    logger: StdErrLogger,
    unwinder: Unwinder,
    join_latch: Option<~JoinLatch>,
    on_exit: Option<~fn(bool)>,
    destroyed: bool,
    coroutine: Option<~Coroutine>,
    sched: Option<~Scheduler>,
    task_type: TaskType
}

pub enum TaskType {
    GreenTask(Option<~SchedHome>),
    SchedTask
}

/// A coroutine is nothing more than a (register context, stack) pair.
pub struct Coroutine {
    /// The segment of stack on which the task is currently running or
    /// if the task is blocked, on which the task will resume
    /// execution.
    priv current_stack_segment: StackSegment,
    /// Always valid if the task is alive and not running.
    saved_context: Context
}

/// Some tasks have a deciated home scheduler that they must run on.
pub enum SchedHome {
    AnySched,
    Sched(SchedHandle)
}

pub struct GarbageCollector;
pub struct LocalStorage(*c_void, Option<extern "Rust" fn(*c_void)>);

pub struct Unwinder {
    unwinding: bool,
}

impl Task {

    // A helper to build a new task using the dynamically found
    // scheduler and task. Only works in GreenTask context.
    pub fn build_homed_child(f: ~fn(), home: SchedHome) -> ~Task {
        let f = Cell::new(f);
        let home = Cell::new(home);
        do Local::borrow::<Task, ~Task> |running_task| {
            let mut sched = running_task.sched.take_unwrap();
            let new_task = ~running_task.new_child_homed(&mut sched.stack_pool,
                                                         home.take(),
                                                         f.take());
            running_task.sched = Some(sched);
            new_task
        }
    }

    pub fn build_child(f: ~fn()) -> ~Task {
        Task::build_homed_child(f, AnySched)
    }

    pub fn build_homed_root(f: ~fn(), home: SchedHome) -> ~Task {
        let f = Cell::new(f);
        let home = Cell::new(home);
        do Local::borrow::<Task, ~Task> |running_task| {
            let mut sched = running_task.sched.take_unwrap();
            let new_task = ~Task::new_root_homed(&mut sched.stack_pool,
                                                    home.take(),
                                                    f.take());
            running_task.sched = Some(sched);
            new_task
        }
    }

    pub fn build_root(f: ~fn()) -> ~Task {
        Task::build_homed_root(f, AnySched)
    }

    pub fn new_sched_task() -> Task {
        Task {
            heap: LocalHeap::new(),
            gc: GarbageCollector,
            storage: LocalStorage(ptr::null(), None),
            logger: StdErrLogger,
            unwinder: Unwinder { unwinding: false },
            join_latch: Some(JoinLatch::new_root()),
            on_exit: None,
            destroyed: false,
            coroutine: Some(~Coroutine::empty()),
            sched: None,
            task_type: SchedTask
        }
    }

    pub fn new_root(stack_pool: &mut StackPool,
                    start: ~fn()) -> Task {
        Task::new_root_homed(stack_pool, AnySched, start)
    }

    pub fn new_child(&mut self,
                     stack_pool: &mut StackPool,
                     start: ~fn()) -> Task {
        self.new_child_homed(stack_pool, AnySched, start)
    }

    pub fn new_root_homed(stack_pool: &mut StackPool,
                          home: SchedHome,
                          start: ~fn()) -> Task {
        Task {
            heap: LocalHeap::new(),
            gc: GarbageCollector,
            storage: LocalStorage(ptr::null(), None),
            logger: StdErrLogger,
            unwinder: Unwinder { unwinding: false },
            join_latch: Some(JoinLatch::new_root()),
            on_exit: None,
            destroyed: false,
            coroutine: Some(~Coroutine::new(stack_pool, start)),
            sched: None,
            task_type: GreenTask(Some(~home))
        }
    }

    pub fn new_child_homed(&mut self,
                           stack_pool: &mut StackPool,
                           home: SchedHome,
                           start: ~fn()) -> Task {
        Task {
            heap: LocalHeap::new(),
            gc: GarbageCollector,
            storage: LocalStorage(ptr::null(), None),
            logger: StdErrLogger,
            unwinder: Unwinder { unwinding: false },
            join_latch: Some(self.join_latch.get_mut_ref().new_child()),
            on_exit: None,
            destroyed: false,
            coroutine: Some(~Coroutine::new(stack_pool, start)),
            sched: None,
            task_type: GreenTask(Some(~home))
        }
    }

    pub fn give_home(&mut self, new_home: SchedHome) {
        match self.task_type {
            GreenTask(ref mut home) => {
                *home = Some(~new_home);
            }
            SchedTask => {
                rtabort!("type error: used SchedTask as GreenTask");
            }
        }
    }

    pub fn swap_unwrap_home(&mut self) -> SchedHome {
        match self.task_type {
            GreenTask(ref mut home) => {
                let out = home.take_unwrap();
                return *out;
            }
            SchedTask => {
                rtabort!("type error: used SchedTask as GreenTask");
            }
        }
    }

    pub fn run(&mut self, f: &fn()) {

        self.unwinder.try(f);
        self.destroy();

        // Wait for children. Possibly report the exit status.
        let local_success = !self.unwinder.unwinding;
        let join_latch = self.join_latch.take_unwrap();
        match self.on_exit {
            Some(ref on_exit) => {
                let success = join_latch.wait(local_success);
                (*on_exit)(success);
            }
            None => {
                join_latch.release(local_success);
            }
        }
    }

    /// must be called manually before finalization to clean up
    /// thread-local resources. Some of the routines here expect
    /// Task to be available recursively so this must be
    /// called unsafely, without removing Task from
    /// thread-local-storage.
    fn destroy(&mut self) {

        rtdebug!("DESTROYING TASK: %u", borrow::to_uint(self));

        do Local::borrow::<Task, ()> |task| {
            assert!(borrow::ref_eq(task, self));
        }

        match self.storage {
            LocalStorage(ptr, Some(ref dtor)) => {
                (*dtor)(ptr)
            }
            _ => ()
        }

        // Destroy remaining boxes
        unsafe { cleanup::annihilate(); }

        self.destroyed = true;
    }

    // New utility functions for homes.

    pub fn is_home_no_tls(&self, sched: &~Scheduler) -> bool {
        match self.task_type {
            GreenTask(Some(~AnySched)) => { false }
            GreenTask(Some(~Sched(SchedHandle { sched_id: ref id, _}))) => {
                *id == sched.sched_id()
            }
            GreenTask(None) => {
                rtabort!("task without home");
            }
            SchedTask => {
                // Awe yea
                rtabort!("type error: expected: GreenTask, found: SchedTask");
            }
        }
    }

    pub fn homed(&self) -> bool {
        match self.task_type {
            GreenTask(Some(~AnySched)) => { false }
            GreenTask(Some(~Sched(SchedHandle { _ }))) => { true }
            GreenTask(None) => {
                rtabort!("task without home");
            }
            SchedTask => {
                rtabort!("type error: expected: GreenTask, found: SchedTask");
            }
        }
    }

    // Grab both the scheduler and the task from TLS and check if the
    // task is executing on an appropriate scheduler.
    pub fn on_appropriate_sched() -> bool {
        do Local::borrow::<Task,bool> |task| {
            let sched_id = task.sched.get_ref().sched_id();
            let sched_run_anything = task.sched.get_ref().run_anything;
            match task.task_type {
                GreenTask(Some(~AnySched)) => {
                    rtdebug!("anysched task in sched check ****");
                    sched_run_anything
                }
                GreenTask(Some(~Sched(SchedHandle { sched_id: ref id, _ }))) => {
                    rtdebug!("homed task in sched check ****");
                    *id == sched_id
                }
                GreenTask(None) => {
                    rtabort!("task without home");
                }
                SchedTask => {
                    rtabort!("type error: expected: GreenTask, found: SchedTask");
                }
            }
        }
    }

    // These utility functions related to home will need to change.
/*
    /// Check if *task* is currently home.
    pub fn is_home(&self) -> bool {
        do Local::borrow::<Scheduler,bool> |sched| {
            match self.home {
                Some(AnySched) => { false }
                Some(Sched(SchedHandle { sched_id: ref id, _ })) => {
                    *id == sched.sched_id()
                }
                None => { rtabort!("task home of None") }
            }
        }
    }

    pub fn is_home_no_tls(&self, sched: &~Scheduler) -> bool {
        match self.home {
            Some(AnySched) => { false }
            Some(Sched(SchedHandle { sched_id: ref id, _ })) => {
                *id == sched.sched_id()
            }
            None => {rtabort!("task home of None") }
        }
    }

    pub fn is_home_using_id(sched_id: uint) -> bool {
        do Local::borrow::<Task,bool> |task| {
            match task.home {
                Some(Sched(SchedHandle { sched_id: ref id, _ })) => {
                    *id == sched_id
                }
                Some(AnySched) => { false }
                None => { rtabort!("task home of None") }
            }
        }
    }

    /// Check if this *task* has a home.
    pub fn homed(&self) -> bool {
        match self.home {
            Some(AnySched) => { false }
            Some(Sched(_)) => { true }
            None => {
                rtabort!("task home of None")
            }
        }
    }

    /// On a special scheduler?
    pub fn on_special() -> bool {
        do Local::borrow::<Scheduler,bool> |sched| {
            !sched.run_anything
        }
    }
*/
}

impl Drop for Task {
    fn drop(&self) {
        rtdebug!("called drop for a task");
        assert!(self.destroyed)
    }
}

// Coroutines represent nothing more than a context and a stack
// segment.

impl Coroutine {

    pub fn new(stack_pool: &mut StackPool, start: ~fn()) -> Coroutine {
        static MIN_STACK_SIZE: uint = 100000; // XXX: Too much stack

        let start = Coroutine::build_start_wrapper(start);
        let mut stack = stack_pool.take_segment(MIN_STACK_SIZE);
        let initial_context = Context::new(start, &mut stack);
        Coroutine {
            current_stack_segment: stack,
            saved_context: initial_context
        }
    }

    pub fn empty() -> Coroutine {
        Coroutine {
            current_stack_segment: StackSegment::new(0),
            saved_context: Context::empty()
        }
    }

    fn build_start_wrapper(start: ~fn()) -> ~fn() {
        let start_cell = Cell::new(start);
        let wrapper: ~fn() = || {
            // First code after swap to this new context. Run our
            // cleanup job.
            unsafe {

                // Again - might work while safe, or it might not.
                do Local::borrow::<Scheduler,()> |sched| {
                    (sched).run_cleanup_job();
                }

                // To call the run method on a task we need a direct
                // reference to it. The task is in TLS, so we can
                // simply unsafe_borrow it to get this reference. We
                // need to still have the task in TLS though, so we
                // need to unsafe_borrow.
                let task = Local::unsafe_borrow::<Task>();

                do (*task).run {
                    // N.B. Removing `start` from the start wrapper
                    // closure by emptying a cell is critical for
                    // correctness. The ~Task pointer, and in turn the
                    // closure used to initialize the first call
                    // frame, is destroyed in the scheduler context,
                    // not task context. So any captured closures must
                    // not contain user-definable dtors that expect to
                    // be in task context. By moving `start` out of
                    // the closure, all the user code goes our of
                    // scope while the task is still running.
                    let start = start_cell.take();
                    start();
                };
            }

            // We remove the sched from the Task in TLS right now.
            let sched = Local::take::<Scheduler>();
            // ... allowing us to give it away when performing a
            // scheduling operation.
            sched.terminate_current_task()
        };
        return wrapper;
    }

    /// Destroy coroutine and try to reuse stack segment.
    pub fn recycle(~self, stack_pool: &mut StackPool) {
        match self {
            ~Coroutine { current_stack_segment, _ } => {
                stack_pool.give_segment(current_stack_segment);
            }
        }
    }

}


// Just a sanity check to make sure we are catching a Rust-thrown exception
static UNWIND_TOKEN: uintptr_t = 839147;

impl Unwinder {
    pub fn try(&mut self, f: &fn()) {
        use sys::Closure;

        unsafe {
            let closure: Closure = transmute(f);
            let code = transmute(closure.code);
            let env = transmute(closure.env);

            let token = rust_try(try_fn, code, env);
            assert!(token == 0 || token == UNWIND_TOKEN);
        }

        extern fn try_fn(code: *c_void, env: *c_void) {
            unsafe {
                let closure: Closure = Closure {
                    code: transmute(code),
                    env: transmute(env),
                };
                let closure: &fn() = transmute(closure);
                closure();
            }
        }

        extern {
            #[rust_stack]
            fn rust_try(f: *u8, code: *c_void, data: *c_void) -> uintptr_t;
        }
    }

    pub fn begin_unwind(&mut self) -> ! {
        self.unwinding = true;
        unsafe {
            rust_begin_unwind(UNWIND_TOKEN);
            return transmute(());
        }
        extern {
            fn rust_begin_unwind(token: uintptr_t);
        }
    }
}
/*
#[cfg(test)]
mod test {
    use rt::test::*;

    #[test]
    fn local_heap() {
        do run_in_newsched_task() {
            let a = @5;
            let b = a;
            assert!(*a == 5);
            assert!(*b == 5);
        }
    }

    #[test]
    fn tls() {
        use local_data;
        do run_in_newsched_task() {
            static key: local_data::Key<@~str> = &local_data::Key;
            local_data::set(key, @~"data");
            assert!(*local_data::get(key, |k| k.map(|&k| *k)).get() == ~"data");
            static key2: local_data::Key<@~str> = &local_data::Key;
            local_data::set(key2, @~"data");
            assert!(*local_data::get(key2, |k| k.map(|&k| *k)).get() == ~"data");
        }
    }

    #[test]
    fn unwind() {
        do run_in_newsched_task() {
            let result = spawntask_try(||());
            rtdebug!("trying first assert");
            assert!(result.is_ok());
            let result = spawntask_try(|| fail!());
            rtdebug!("trying second assert");
            assert!(result.is_err());
        }
    }

    #[test]
    fn rng() {
        do run_in_newsched_task() {
            use rand::{rng, Rng};
            let mut r = rng();
            let _ = r.next();
        }
    }

    #[test]
    fn logging() {
        do run_in_newsched_task() {
            info!("here i am. logging in a newsched task");
        }
    }

    #[test]
    fn comm_oneshot() {
        use comm::*;

        do run_in_newsched_task {
            let (port, chan) = oneshot();
            send_one(chan, 10);
            assert!(recv_one(port) == 10);
        }
    }

    #[test]
    fn comm_stream() {
        use comm::*;

        do run_in_newsched_task() {
            let (port, chan) = stream();
            chan.send(10);
            assert!(port.recv() == 10);
        }
    }

    #[test]
    fn comm_shared_chan() {
        use comm::*;

        do run_in_newsched_task() {
            let (port, chan) = stream();
            let chan = SharedChan::new(chan);
            chan.send(10);
            assert!(port.recv() == 10);
        }
    }

    #[test]
    fn linked_failure() {
        do run_in_newsched_task() {
            let res = do spawntask_try {
                spawntask_random(|| fail!());
            };
            assert!(res.is_err());
        }
    }

    #[test]
    fn heap_cycles() {
        use option::{Option, Some, None};

        do run_in_newsched_task {
            struct List {
                next: Option<@mut List>,
            }

            let a = @mut List { next: None };
            let b = @mut List { next: Some(a) };

            a.next = Some(b);
        }
    }

    // XXX: This is a copy of test_future_result in std::task.
    // It can be removed once the scheduler is turned on by default.
    #[test]
    fn future_result() {
        do run_in_newsched_task {
            use option::{Some, None};
            use task::*;

            let mut result = None;
            let mut builder = task();
            builder.future_result(|r| result = Some(r));
            do builder.spawn {}
            assert_eq!(result.unwrap().recv(), Success);

            result = None;
            let mut builder = task();
            builder.future_result(|r| result = Some(r));
            builder.unlinked();
            do builder.spawn {
                fail!();
            }
            assert_eq!(result.unwrap().recv(), Failure);
        }
    }
}
*/
