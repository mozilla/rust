// xfail-stage0
// xfail-stage1
// xfail-stage2
// -*- rust -*-

fn a(chan[int] c) {
    log "task a0";
    log "task a1";
    c <| 10;
}

fn main() {
    let port[int] p = port();
    auto task_a = spawn a(chan(p));
    auto task_b = spawn b(chan(p));
    let int n = 0;
    n <- p;
    n <- p;
    log "Finished.";
}

fn b(chan[int] c) {
    log "task b0";
    log "task b1";
    log "task b2";
    log "task b2";
    log "task b3";
    c <| 10;
}
