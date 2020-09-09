//! Check for external package sources. Allow only vendorable packages.

use std::fs;
use std::path::Path;

/// List of allowed sources for packages.
const ALLOWED_SOURCES: &[&str] = &[
    "\"registry+https://github.com/rust-lang/crates.io-index\"",
    "\"git+https://github.com/michaelwoerister/measureme.git?branch=paged_sinks#26d150475c8d83613beffe970d1c18dd2494f607\"",
];

/// Checks for external package sources. `root` is the path to the directory that contains the
/// workspace `Cargo.toml`.
pub fn check(root: &Path, bad: &mut bool) {
    // `Cargo.lock` of rust.
    let path = root.join("Cargo.lock");

    // Open and read the whole file.
    let cargo_lock = t!(fs::read_to_string(&path));

    // Process each line.
    for line in cargo_lock.lines() {
        // Consider only source entries.
        if !line.starts_with("source = ") {
            continue;
        }

        // Extract source value.
        let source = line.splitn(2, '=').nth(1).unwrap().trim();

        // Ensure source is allowed.
        if !ALLOWED_SOURCES.contains(&&*source) {
            println!("invalid source: {}", source);
            *bad = true;
        }
    }
}
