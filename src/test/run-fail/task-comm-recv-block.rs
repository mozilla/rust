// error-pattern:goodfail

use std;
import std::task;
import std::comm;

fn# goodfail(&&_i: ()) {
    task::yield();
    fail "goodfail";
}

fn main() {
    task::spawn2((), goodfail);
    let po = comm::port();
    // We shouldn't be able to get past this recv since there's no
    // message available
    let i: int = comm::recv(po);
    fail "badfail";
}
