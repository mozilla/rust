// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

struct cat {

  name : String,

}

fn cat(in_name: String) -> cat {
    cat {
        name: in_name
    }
}

pub fn main() {
  let _nyan = cat("nyan".to_string());
}
