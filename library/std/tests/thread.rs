#![feature(mutex_arc)]

use std::sync::Mutex;
use std::thread;
use std::time::Duration;

#[test]
#[cfg_attr(target_os = "emscripten", ignore)]
fn sleep() {
    let finished = Mutex::arc(false);
    let t_finished = finished.clone();
    thread::spawn(move || {
        thread::sleep(Duration::new(u64::MAX, 0));
        *t_finished.lock().unwrap() = true;
    });
    thread::sleep(Duration::from_millis(100));
    assert_eq!(*finished.lock().unwrap(), false);
}
