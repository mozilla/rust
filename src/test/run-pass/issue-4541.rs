// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn parse_args() -> String {
    let args = ::std::os::args();
    let args = args.as_slice();
    let mut n = 0;

    while n < args.len() {
        match args[n].as_slice() {
            "-v" => (),
            s => {
                return s.to_string();
            }
        }
        n += 1;
    }

    return "".to_string()
}

pub fn main() {
    println!("{}", parse_args());
}
