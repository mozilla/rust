// error-pattern: `#[panic_implementation]` function required, but not found

#![feature(lang_items)]
#![no_main]
#![no_std]

#[lang = "eh_personality"]
fn eh() {}
