// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.


extern crate serialize;

use serialize::{json, Decodable};

trait JD : Decodable {}

fn exec<T: JD>() {
    let doc = json::from_str("").unwrap();
    let mut decoder = json::Decoder::new(doc);
    let _v: T = Decodable::decode(&mut decoder).unwrap();
    panic!()
}

pub fn main() {}
