
#include <stdarg.h>
#include <cassert>
#include <pthread.h>
#include <vector>
#include "rust_internal.h"
#include "rust_util.h"
#include "globals.h"
#include "rust_scheduler.h"

#ifndef _WIN32
pthread_key_t rust_task_thread::task_key;
#else
DWORD rust_task_thread::task_key;
#endif

const size_t SCHED_STACK_SIZE = 1024*100;
const size_t C_STACK_SIZE = 1024*1024;

bool rust_task_thread::tls_initialized = false;

rust_task_thread::rust_task_thread(rust_scheduler *sched,
                                   rust_srv *srv,
                                   int id) :
    rust_thread(SCHED_STACK_SIZE),
    _log(srv, this),
    cache(this),
    id(id),
    should_exit(false),
    cached_c_stack(NULL),
    kernel(sched->kernel),
    sched(sched),
    srv(srv),
    newborn_tasks(this, "newborn"),
    running_tasks(this, "running"),
    blocked_tasks(this, "blocked"),
    dead_tasks(this, "dead"),
    log_lvl(log_debug),
    min_stack_size(kernel->env->min_stack_size),
    env(kernel->env),
    // TODO: calculate a per scheduler name.
    name("main")
{
    LOGPTR(this, "new dom", (uintptr_t)this);
    isaac_init(kernel, &rctx);
#ifndef __WIN32__
    pthread_attr_init(&attr);
    pthread_attr_setstacksize(&attr, 1024 * 1024);
    pthread_attr_setdetachstate(&attr, true);
#endif

    if (!tls_initialized)
        init_tls();
}

rust_task_thread::~rust_task_thread() {
    DLOG(this, dom, "~rust_task_thread %s @0x%" PRIxPTR, name, (uintptr_t)this);

    newborn_tasks.delete_all();
    running_tasks.delete_all();
    blocked_tasks.delete_all();
    dead_tasks.delete_all();
#ifndef __WIN32__
    pthread_attr_destroy(&attr);
#endif
}

void
rust_task_thread::activate(rust_task *task) {
    task->ctx.next = &c_context;
    DLOG(this, task, "descheduling...");
    lock.unlock();
    prepare_c_stack(task);
    task->ctx.swap(c_context);
    unprepare_c_stack();
    lock.lock();
    DLOG(this, task, "task has returned");
}

void
rust_task_thread::log(rust_task* task, uint32_t level, char const *fmt, ...) {
    char buf[BUF_BYTES];
    va_list args;
    va_start(args, fmt);
    vsnprintf(buf, sizeof(buf), fmt, args);
    _log.trace_ln(task, level, buf);
    va_end(args);
}

void
rust_task_thread::fail() {
    log(NULL, log_err, "domain %s @0x%" PRIxPTR " root task failed",
        name, this);
    kernel->fail();
}

void
rust_task_thread::kill_all_tasks() {
    std::vector<rust_task*> all_tasks;

    {
        scoped_lock with(lock);

        for (size_t i = 0; i < running_tasks.length(); i++) {
            all_tasks.push_back(running_tasks[i]);
        }

        for (size_t i = 0; i < blocked_tasks.length(); i++) {
            all_tasks.push_back(blocked_tasks[i]);
        }
    }

    while (!all_tasks.empty()) {
        rust_task *task = all_tasks.back();
        all_tasks.pop_back();
        // We don't want the failure of these tasks to propagate back
        // to the kernel again since we're already failing everything
        task->unsupervise();
        task->kill();
    }
}

size_t
rust_task_thread::number_of_live_tasks() {
    return running_tasks.length() + blocked_tasks.length();
}

/**
 * Delete any dead tasks.
 */
void
rust_task_thread::reap_dead_tasks() {
    I(this, lock.lock_held_by_current_thread());

    if (dead_tasks.length() == 0) {
        return;
    }

    A(this, dead_tasks.length() == 1,
      "Only one task should die during a single turn of the event loop");

    // First make a copy of the dead_task list with the lock held
    rust_task *dead_task = dead_tasks.pop_value();

    // Dereferencing the task will probably cause it to be released
    // from the scheduler, which may end up trying to take this lock
    lock.unlock();

    // Release the task from the kernel so nobody else can get at it
    kernel->release_task_id(dead_task->id);
    dead_task->delete_all_stacks();
    // Deref the task, which may cause it to request us to release it
    dead_task->deref();

    lock.lock();
}

void
rust_task_thread::release_task(rust_task *task) {
    // Nobody should have a ref to the task at this point
    I(this, task->get_ref_count() == 0);
    // Kernel should not know about the task any more
    I(this, kernel->get_task_by_id(task->id) == NULL);
    // Now delete the task, which will require using this thread's
    // memory region.
    delete task;
    // Now release the task from the scheduler, which may trigger this
    // thread to exit
    sched->release_task();
}

/**
 * Schedules a running task for execution. Only running tasks can be
 * activated.  Blocked tasks have to be unblocked before they can be
 * activated.
 *
 * Returns NULL if no tasks can be scheduled.
 */
rust_task *
rust_task_thread::schedule_task() {
    I(this, this);
    // FIXME: in the face of failing tasks, this is not always right.
    // I(this, n_live_tasks() > 0);
    if (running_tasks.length() > 0) {
        size_t k = isaac_rand(&rctx);
        // Look around for a runnable task, starting at k.
        for(size_t j = 0; j < running_tasks.length(); ++j) {
            size_t  i = (j + k) % running_tasks.length();
            return (rust_task *)running_tasks[i];
        }
    }
    return NULL;
}

void
rust_task_thread::log_state() {
    if (log_rt_task < log_debug) return;

    if (!running_tasks.is_empty()) {
        log(NULL, log_debug, "running tasks:");
        for (size_t i = 0; i < running_tasks.length(); i++) {
            log(NULL, log_debug, "\t task: %s @0x%" PRIxPTR,
                running_tasks[i]->name,
                running_tasks[i]);
        }
    }

    if (!blocked_tasks.is_empty()) {
        log(NULL, log_debug, "blocked tasks:");
        for (size_t i = 0; i < blocked_tasks.length(); i++) {
            log(NULL, log_debug, "\t task: %s @0x%" PRIxPTR ", blocked on: 0x%"
                PRIxPTR " '%s'",
                blocked_tasks[i]->name, blocked_tasks[i],
                blocked_tasks[i]->get_cond(),
                blocked_tasks[i]->get_cond_name());
        }
    }

    if (!dead_tasks.is_empty()) {
        log(NULL, log_debug, "dead tasks:");
        for (size_t i = 0; i < dead_tasks.length(); i++) {
            log(NULL, log_debug, "\t task: %s 0x%" PRIxPTR,
                dead_tasks[i]->name, dead_tasks[i]);
        }
    }
}
/**
 * Starts the main scheduler loop which performs task scheduling for this
 * domain.
 *
 * Returns once no more tasks can be scheduled and all task ref_counts
 * drop to zero.
 */
void
rust_task_thread::start_main_loop() {
    lock.lock();

    DLOG(this, dom, "started domain loop %d", id);

    while (!should_exit) {
        DLOG(this, dom, "worker %d, number_of_live_tasks = %d",
             id, number_of_live_tasks());

        rust_task *scheduled_task = schedule_task();

        if (scheduled_task == NULL) {
            log_state();
            DLOG(this, task,
                 "all tasks are blocked, scheduler id %d yielding ...",
                 id);
            lock.wait();
            A(this, dead_tasks.length() == 0,
              "Tasks should only die after running");
            DLOG(this, task,
                 "scheduler %d resuming ...", id);
            continue;
        }

        I(this, scheduled_task->running());

        DLOG(this, task,
             "activating task %s 0x%" PRIxPTR
             ", state: %s",
             scheduled_task->name,
             (uintptr_t)scheduled_task,
             scheduled_task->get_state()->name);

        place_task_in_tls(scheduled_task);

        DLOG(this, task,
             "Running task %p on worker %d",
             scheduled_task, id);
        activate(scheduled_task);

        DLOG(this, task,
             "returned from task %s @0x%" PRIxPTR
             " in state '%s', worker id=%d" PRIxPTR,
             scheduled_task->name,
             (uintptr_t)scheduled_task,
             scheduled_task->get_state()->name,
             id);

        reap_dead_tasks();
    }

    A(this, newborn_tasks.is_empty(), "Should have no newborn tasks");
    A(this, running_tasks.is_empty(), "Should have no running tasks");
    A(this, blocked_tasks.is_empty(), "Should have no blocked tasks");
    A(this, dead_tasks.is_empty(), "Should have no dead tasks");

    DLOG(this, dom, "finished main-loop %d", id);

    lock.unlock();

    I(this, !extra_c_stack);
    if (cached_c_stack) {
        destroy_stack(kernel->region(), cached_c_stack);
        cached_c_stack = NULL;
    }
}

rust_crate_cache *
rust_task_thread::get_cache() {
    return &cache;
}

rust_task_id
rust_task_thread::create_task(rust_task *spawner, const char *name,
                            size_t init_stack_sz) {
    rust_task *task =
        new (this->kernel, "rust_task")
        rust_task (this, &newborn_tasks, spawner, name, init_stack_sz);
    DLOG(this, task, "created task: " PTR ", spawner: %s, name: %s",
                        task, spawner ? spawner->name : "null", name);

    {
        scoped_lock with(lock);
        newborn_tasks.append(task);
    }

    kernel->register_task(task);
    return task->id;
}

void 
rust_task_thread::transition(rust_task *task,
                             rust_task_list *src, rust_task_list *dst,
                             rust_cond *cond, const char* cond_name) {
    bool unlock = false;
    if(!lock.lock_held_by_current_thread()) {
        unlock = true;
        lock.lock();
    }
    DLOG(this, task,
         "task %s " PTR " state change '%s' -> '%s' while in '%s'",
         name, (uintptr_t)this, src->name, dst->name,
         task->get_state()->name);
    I(this, task->get_state() == src);
    src->remove(task);
    dst->append(task);
    task->set_state(dst, cond, cond_name);

    lock.signal();
    if(unlock)
        lock.unlock();
}

void rust_task_thread::run() {
    this->start_main_loop();
    sched->release_task_thread();
}

#ifndef _WIN32
void
rust_task_thread::init_tls() {
    int result = pthread_key_create(&task_key, NULL);
    assert(!result && "Couldn't create the TLS key!");
    tls_initialized = true;
}

void
rust_task_thread::place_task_in_tls(rust_task *task) {
    int result = pthread_setspecific(task_key, task);
    assert(!result && "Couldn't place the task in TLS!");
    task->record_stack_limit();
}
#else
void
rust_task_thread::init_tls() {
    task_key = TlsAlloc();
    assert(task_key != TLS_OUT_OF_INDEXES && "Couldn't create the TLS key!");
    tls_initialized = true;
}

void
rust_task_thread::place_task_in_tls(rust_task *task) {
    BOOL result = TlsSetValue(task_key, task);
    assert(result && "Couldn't place the task in TLS!");
    task->record_stack_limit();
}
#endif

void
rust_task_thread::exit() {
    A(this, !lock.lock_held_by_current_thread(), "Shouldn't have lock");
    scoped_lock with(lock);
    should_exit = true;
    lock.signal();
}

// Before activating each task, make sure we have a C stack available.
// It needs to be allocated ahead of time (while we're on our own
// stack), because once we're on the Rust stack we won't have enough
// room to do the allocation
void
rust_task_thread::prepare_c_stack(rust_task *task) {
    I(this, !extra_c_stack);
    if (!cached_c_stack && !task->have_c_stack()) {
        cached_c_stack = create_stack(kernel->region(), C_STACK_SIZE);
    }
}

void
rust_task_thread::unprepare_c_stack() {
    if (extra_c_stack) {
        destroy_stack(kernel->region(), extra_c_stack);
        extra_c_stack = NULL;
    }
}

//
// Local Variables:
// mode: C++
// fill-column: 70;
// indent-tabs-mode: nil
// c-basic-offset: 4
// buffer-file-coding-system: utf-8-unix
// End:
//
