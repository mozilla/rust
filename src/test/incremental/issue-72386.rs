// revisions: rpass1 cfail1 rpass3
// Regression test for issue #72386
// Checks that we don't ICE when switching to an invalid register
// and back again

#![feature(asm)]

#[cfg(any(rpass1, rpass3))]
fn main() {
    unsafe {
        asm!("nop")
    }
}

#[cfg(cfail1)]
fn main() {
    unsafe {
        asm!("nop",out("invalid_reg")_)
        //[cfail1]~^ ERROR invalid register
    }
}
