// run-pass

#![allow(unused_must_use)]
#![allow(unused_imports)]
// This test can't be a unit test in std,
// because it needs TempDir, which is in extra

// ignore-cross-compile

use std::env;
use std::ffi::CString;
use std::fs::{self, File};
use std::path::PathBuf;

fn rename_directory() {
    let tmpdir = PathBuf::from(env::var_os("RUST_TEST_TMPDIR").unwrap());
    let old_path = tmpdir.join("foo/bar/baz");
    fs::create_dir_all(&old_path).unwrap();
    let test_file = &old_path.join("temp.txt");

    File::create(test_file).unwrap();

    let new_path = tmpdir.join("quux/blat");
    fs::create_dir_all(&new_path).unwrap();
    fs::rename(&old_path, &new_path.join("newdir"));
    assert!(fs::metadata(new_path.join("newdir")).map(|m| m.is_dir()).unwrap_or(false));
    assert!(fs::metadata(new_path.join("newdir/temp.txt")).is_ok());
}

pub fn main() { rename_directory() }
