// error-pattern:greetings from the panic handler

use std::panic;

fn main() {
    panic::set_hook(Box::new(|i| {
        eprint!("greetings from the panic handler");
    }));
    panic!("foobar");
}
