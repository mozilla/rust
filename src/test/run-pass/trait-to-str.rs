// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// ignore-lexer-test FIXME #15883


trait to_str {
    fn to_string_(&self) -> String;
}

impl to_str for int {
    fn to_string_(&self) -> String { self.to_string() }
}

impl<T:to_str> to_str for Vec<T> {
    fn to_string_(&self) -> String {
        format!("[{}]",
                self.iter()
                    .map(|e| e.to_string_())
                    .collect::<Vec<String>>()
                    .connect(", "))
    }
}

pub fn main() {
    assert!(1.to_string_() == "1".to_string());
    assert!((vec!(2i, 3, 4)).to_string_() == "[2, 3, 4]".to_string());

    fn indirect<T:to_str>(x: T) -> String {
        format!("{}!", x.to_string_())
    }
    assert!(indirect(vec!(10i, 20)) == "[10, 20]!".to_string());

    fn indirect2<T:to_str>(x: T) -> String {
        indirect(x)
    }
    assert!(indirect2(vec!(1i)) == "[1]!".to_string());
}
