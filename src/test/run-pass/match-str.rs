// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Issue #53

pub fn main() {
    match "test" { "not-test" => panic!(), "test" => (), _ => panic!() }

    enum t { tag1(String), tag2, }


    match t::tag1("test".to_string()) {
      t::tag2 => panic!(),
      t::tag1(ref s) if "test" != s.as_slice() => panic!(),
      t::tag1(ref s) if "test" == s.as_slice() => (),
      _ => panic!()
    }

    let x = match "a" { "a" => 1i, "b" => 2i, _ => panic!() };
    assert_eq!(x, 1);

    match "a" { "a" => { } "b" => { }, _ => panic!() }

}
