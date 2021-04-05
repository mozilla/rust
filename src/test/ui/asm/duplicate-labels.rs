// compile-flags: -g
// build-pass

#![feature(asm)]

fn asm1() {
    unsafe {
        asm!("duplabel: nop",);
    }
}

fn asm2() {
    unsafe {
        asm!("nop", "duplabel: nop",);
    }
}

fn main() {
    asm1();
    asm2();
}
