// edition:2018

use std::sync::Mutex;

fn is_send<T: Send>(t: T) {

}

async fn foo() {
    bar(&Mutex::new(22)).await;
}

async fn bar(x: &Mutex<u32>) {
    let g = x.lock().unwrap();
    baz().await;
}

async fn baz() {

}

fn main() {
    is_send(foo());
    //~^ ERROR `std::sync::MutexGuard<'_, u32>` cannot be sent between threads safely [E0277]
}
