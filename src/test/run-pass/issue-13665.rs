// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn foo<'r>() {
  let maybe_value_ref: Option<&'r u8> = None;

  let _ = maybe_value_ref.map(|& ref v| v);
  let _ = maybe_value_ref.map(|& ref v| -> &'r u8 {v});
  let _ = maybe_value_ref.map(|& ref v: &'r u8| -> &'r u8 {v});
  let _ = maybe_value_ref.map(|& ref v: &'r u8| {v});
}

fn main() {
  foo();
}
