// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Continue kindck-send-object1.rs.

fn assert_send<T:Send>() { }
trait Dummy { }

fn test50() {
    assert_send::<&'static Dummy>(); //~ ERROR the trait `core::marker::Send` is not implemented
}

fn test53() {
    assert_send::<Box<Dummy>>(); //~ ERROR the trait `core::marker::Send` is not implemented
}

// ...unless they are properly bounded
fn test60() {
    assert_send::<&'static (Dummy+Send)>();
}
fn test61() {
    assert_send::<Box<Dummy+Send>>();
}

fn main() { }
