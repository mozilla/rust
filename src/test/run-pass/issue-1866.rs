// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

mod a {
    pub type rust_task = uint;
    pub mod rustrt {
        use super::rust_task;
        extern {
            pub fn rust_task_is_unwinding(rt: *const rust_task) -> bool;
        }
    }
}

mod b {
    pub type rust_task = bool;
    pub mod rustrt {
        use super::rust_task;
        extern {
            pub fn rust_task_is_unwinding(rt: *const rust_task) -> bool;
        }
    }
}

pub fn main() { }
