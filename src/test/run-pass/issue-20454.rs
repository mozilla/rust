use std::thread;

fn _foo() {
    thread::spawn(move || { // no need for -> ()
        loop {
            println!("hello");
        }
    }).join();
}

fn main() {}
