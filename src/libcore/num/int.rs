// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Deprecated: replaced by `isize`.
//!
//! The rollout of the new type will gradually take place over the
//! alpha cycle along with the development of clearer conventions
//! around integer types.

#![deprecated = "replaced by isize"]

#[cfg(target_pointer_width = "32")] int_module! { int, 32 }
#[cfg(target_pointer_width = "64")] int_module! { int, 64 }
