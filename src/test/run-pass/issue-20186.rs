struct Foo;

impl Foo {
    fn putc(&self, b: u8) { }

    fn puts(&self, s: &str) {
        for byte in s.bytes() {
            self.putc(byte)
        }
    }
}

fn main() {}
