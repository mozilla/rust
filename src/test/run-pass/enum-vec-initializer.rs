// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

enum Flopsy {
    Bunny = 2
}

const BAR:uint = Flopsy::Bunny as uint;
const BAR2:uint = BAR;

pub fn main() {
    let _v = [0i;  Flopsy::Bunny as uint];
    let _v = [0i;  BAR];
    let _v = [0i;  BAR2];
    const BAR3:uint = BAR2;
    let _v = [0i;  BAR3];
}
