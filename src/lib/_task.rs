native "rust" mod rustrt {
    fn task_sleep(uint time_in_us);
    fn task_yield();
    fn task_join(task t);
}

/**
 * Hints the scheduler to yield this task for a specified ammount of time.
 *
 * arg: time_in_us maximum number of microseconds to yield control for
 */
fn sleep(uint time_in_us) {
    ret rustrt.task_sleep(time_in_us);
}

fn join(task t) {
    ret rustrt.task_join(t);
}

fn yield() {
    ret rustrt.task_yield();
}

// Local Variables:
// mode: rust;
// fill-column: 78;
// indent-tabs-mode: nil
// c-basic-offset: 4
// buffer-file-coding-system: utf-8-unix
// compile-command: "make -k -C .. 2>&1 | sed -e 's/\\/x\\//x:\\//g'";
// End:
