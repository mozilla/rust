// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn main() {
    let mut my_stuff = std::collections::HashMap::new();
    my_stuff.insert(0is, 42is);

    let (_, thing) = my_stuff.iter().next().unwrap();

    my_stuff.clear(); //~ ERROR cannot borrow

    println!("{}", *thing);
}
