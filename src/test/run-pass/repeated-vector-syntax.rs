// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

pub fn main() {
    let x = [ [true]; 512 ];
    let y = [ 0i; 1 ];

    print!("[");
    for xi in x.iter() {
        print!("{:?}, ", &xi[]);
    }
    println!("]");
    println!("{:?}", &y[]);
}
