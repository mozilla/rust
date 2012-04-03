
#include "rust_globals.h"
#include "rust_scheduler.h"
#include "rust_task.h"
#include "rust_util.h"
#include "rust_sched_launcher.h"

rust_scheduler::rust_scheduler(rust_kernel *kernel,
                               size_t num_threads,
                               rust_sched_id id) :
    kernel(kernel),
    live_threads(num_threads),
    live_tasks(0),
    num_threads(num_threads),
    cur_thread(0),
    id(id)
{
    create_task_threads();
}

rust_scheduler::~rust_scheduler() {
    destroy_task_threads();
}

rust_sched_launcher *
rust_scheduler::create_task_thread(int id) {
    rust_sched_launcher *thread =
        new (kernel, "rust_thread_sched_launcher")
        rust_thread_sched_launcher(this, id);
    KLOG(kernel, kern, "created task thread: " PTR ", id: %d",
          thread, id);
    return thread;
}

void
rust_scheduler::destroy_task_thread(rust_sched_launcher *thread) {
    KLOG(kernel, kern, "deleting task thread: " PTR, thread);
    delete thread;
}

void
rust_scheduler::create_task_threads() {
    KLOG(kernel, kern, "Using %d scheduler threads.", num_threads);

    for(size_t i = 0; i < num_threads; ++i) {
        threads.push(create_task_thread(i));
    }
}

void
rust_scheduler::destroy_task_threads() {
    for(size_t i = 0; i < num_threads; ++i) {
        destroy_task_thread(threads[i]);
    }
}

void
rust_scheduler::start_task_threads()
{
    for(size_t i = 0; i < num_threads; ++i) {
        rust_sched_launcher *thread = threads[i];
        thread->start();
    }
}

void
rust_scheduler::join_task_threads()
{
    for(size_t i = 0; i < num_threads; ++i) {
        rust_sched_launcher *thread = threads[i];
        thread->join();
    }
}

void
rust_scheduler::kill_all_tasks() {
    for(size_t i = 0; i < num_threads; ++i) {
        rust_sched_launcher *thread = threads[i];
        thread->get_loop()->kill_all_tasks();
    }
}

rust_task *
rust_scheduler::create_task(rust_task *spawner, const char *name) {
    size_t thread_no;
    {
        scoped_lock with(lock);
        live_tasks++;
        thread_no = cur_thread++;
        if (cur_thread >= num_threads)
            cur_thread = 0;
    }
    rust_sched_launcher *thread = threads[thread_no];
    return thread->get_loop()->create_task(spawner, name);
}

void
rust_scheduler::release_task() {
    bool need_exit = false;
    {
        scoped_lock with(lock);
        live_tasks--;
        if (live_tasks == 0) {
            need_exit = true;
        }
    }
    if (need_exit) {
        // There are no more tasks on this scheduler. Time to leave
        exit();
    }
}

void
rust_scheduler::exit() {
    // Take a copy of num_threads. After the last thread exits this
    // scheduler will get destroyed, and our fields will cease to exist.
    size_t current_num_threads = num_threads;
    for(size_t i = 0; i < current_num_threads; ++i) {
        threads[i]->get_loop()->exit();
    }
}

size_t
rust_scheduler::number_of_threads() {
    return num_threads;
}

void
rust_scheduler::release_task_thread() {
    uintptr_t new_live_threads;
    {
        scoped_lock with(lock);
        new_live_threads = --live_threads;
    }
    if (new_live_threads == 0) {
        kernel->release_scheduler_id(id);
    }
}
