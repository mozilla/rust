use std;
import std._task;

impure fn start(chan[int] c, int start, int number_of_messages) {
    let int i = 0;
    while (i < number_of_messages) {
        c <| start + i;
        i += 1;
    }    
}

fn main() -> () {
    log "Check that we don't deadlock.";
    let port[int] p = port();
    let task a = spawn "start" start(chan(p), 0, 10);
    _task.join(a);
    log "Joined Task";
}