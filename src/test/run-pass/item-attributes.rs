// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// These are attributes of the implicit crate. Really this just needs to parse
// for completeness since .rs files linked from .rc files support this
// notation to specify their module's attributes

// pretty-expanded FIXME #23616

#![feature(custom_attribute, libc)]
#![allow(unused_attribute)]
#![attr1 = "val"]
#![attr2 = "val"]
#![attr3]
#![attr4(attr5)]

#![crate_id="foobar#0.1"]

// These are attributes of the following mod
#[attr1 = "val"]
#[attr2 = "val"]
mod test_first_item_in_file_mod {}

mod test_single_attr_outer {
    #[attr = "val"]
    pub static x: isize = 10;

    #[attr = "val"]
    pub fn f() { }

    #[attr = "val"]
    pub mod mod1 {}

    pub mod rustrt {
        #[attr = "val"]
        extern {}
    }
}

mod test_multi_attr_outer {
    #[attr1 = "val"]
    #[attr2 = "val"]
    pub static x: isize = 10;

    #[attr1 = "val"]
    #[attr2 = "val"]
    pub fn f() { }

    #[attr1 = "val"]
    #[attr2 = "val"]
    pub mod mod1 {}

    pub mod rustrt {
        #[attr1 = "val"]
        #[attr2 = "val"]
        extern {}
    }

    #[attr1 = "val"]
    #[attr2 = "val"]
    struct t {x: isize}
}

mod test_stmt_single_attr_outer {
    pub fn f() {
        #[attr = "val"]
        static x: isize = 10;

        #[attr = "val"]
        fn f() { }

        #[attr = "val"]
        mod mod1 {
        }

        mod rustrt {
            #[attr = "val"]
            extern {
            }
        }
    }
}

mod test_stmt_multi_attr_outer {
    pub fn f() {

        #[attr1 = "val"]
        #[attr2 = "val"]
        static x: isize = 10;

        #[attr1 = "val"]
        #[attr2 = "val"]
        fn f() { }

        #[attr1 = "val"]
        #[attr2 = "val"]
        mod mod1 {
        }

        mod rustrt {
            #[attr1 = "val"]
            #[attr2 = "val"]
            extern {
            }
        }
    }
}

mod test_attr_inner {
    pub mod m {
        // This is an attribute of mod m
        #![attr = "val"]
    }
}

mod test_attr_inner_then_outer {
    pub mod m {
        // This is an attribute of mod m
        #![attr = "val"]
        // This is an attribute of fn f
        #[attr = "val"]
        fn f() { }
    }
}

mod test_attr_inner_then_outer_multi {
    pub mod m {
        // This is an attribute of mod m
        #![attr1 = "val"]
        #![attr2 = "val"]
        // This is an attribute of fn f
        #[attr1 = "val"]
        #[attr2 = "val"]
        fn f() { }
    }
}

mod test_distinguish_syntax_ext {
    pub fn f() {
        format!("test{}", "s");
        #[attr = "val"]
        fn g() { }
    }
}

mod test_other_forms {
    #[attr]
    #[attr(word)]
    #[attr(attr(word))]
    #[attr(key1 = "val", key2 = "val", attr)]
    pub fn f() { }
}

mod test_foreign_items {
    pub mod rustrt {
        extern crate libc;

        extern {
            #![attr]

            #[attr]
            fn rust_get_test_int() -> libc::intptr_t;
        }
    }
}


// FIXME #623 - these aren't supported yet
/*mod test_literals {
    #![str = "s"]
    #![char = 'c']
    #![isize = 100]
    #![usize = 100_usize]
    #![mach_int = 100u32]
    #![float = 1.0]
    #![mach_float = 1.0f32]
    #![nil = ()]
    #![bool = true]
    mod m {}
}*/

fn test_fn_inner() {
    #![inner_fn_attr]
}

pub fn main() { }
