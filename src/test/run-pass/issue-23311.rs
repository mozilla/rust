// Test that we do not ICE when pattern matching an array against a slice.

#![feature(slice_patterns)]

fn main() {
    match "foo".as_bytes() {
        b"food" => (),
        &[b'f', ..] => (),
        _ => ()
    }
}
