// xfail-win32
use std;
import task;

fn f() {
    let a = @0;
    fail;
}

fn main() {
    let builder = task::mk_task_builder();
    task::unsupervise(builder);
    task::run(builder) {|| f(); }
}