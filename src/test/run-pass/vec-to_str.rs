// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

pub fn main() {
    assert_eq!(format!("{:?}", vec!(0i, 1)), "[0i, 1i]".to_string());

    let foo = vec!(3i, 4);
    let bar: &[int] = &[4, 5];

    assert_eq!(format!("{:?}", foo), "[3i, 4i]");
    assert_eq!(format!("{:?}", bar), "[4i, 5i]");
}
