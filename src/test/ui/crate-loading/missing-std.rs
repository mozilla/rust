// compile-flags: --target x86_64-unknown-uefi
// rustc-env:CARGO=/usr/bin/cargo
// rustc-env:RUSTUP_HOME=/home/bors/.rustup
#![no_core]
extern crate core;
//~^ ERROR can't find crate for `core`
//~| NOTE can't find crate
//~| NOTE target may not be installed
//~| HELP consider building the standard library from source with `cargo build -Zbuild-std`
//~| HELP consider downloading the target with `rustup target add x86_64-unknown-uefi`
fn main() {}
