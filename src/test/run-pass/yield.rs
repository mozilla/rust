// ignore-emscripten no threads support

use std::thread;

pub fn main() {
    let mut result = thread::spawn(child);
    println!("1");
    thread::yield_now();
    println!("2");
    thread::yield_now();
    println!("3");
    result.join();
}

fn child() {
    println!("4"); thread::yield_now(); println!("5"); thread::yield_now(); println!("6");
}
