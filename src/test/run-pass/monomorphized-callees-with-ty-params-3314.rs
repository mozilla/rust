// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.


trait Serializer {
}

trait Serializable {
    fn serialize<S:Serializer>(&self, s: S);
}

impl Serializable for int {
    fn serialize<S:Serializer>(&self, _s: S) { }
}

struct F<A> { a: A }

impl<A:Serializable> Serializable for F<A> {
    fn serialize<S:Serializer>(&self, s: S) {
        self.a.serialize(s);
    }
}

impl Serializer for int {
}

pub fn main() {
    let foo = F { a: 1i };
    foo.serialize(1i);

    let bar = F { a: F {a: 1i } };
    bar.serialize(2i);
}
