#![feature(lang_items)]

// Arc is expected to be a struct, so this will error.
#[lang = "arc"] //~ ERROR language item must be applied to a struct
static X: u32 = 42;

fn main() {}
