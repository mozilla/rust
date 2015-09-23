// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// no-pretty-expanded FIXME #15189
// ignore-android FIXME #17520
// ignore-msvc FIXME #28133
// ignore-windows

use std::env;
use std::process::{Command, Stdio};
use std::str;
use std::ops::{Drop, FnMut, FnOnce};

#[inline(never)]
fn foo() {
    let _v = vec![1, 2, 3];
    if env::var_os("IS_TEST").is_some() {
        panic!()
    }
}

#[inline(never)]
fn double() {
    struct Double;

    impl Drop for Double {
        fn drop(&mut self) { panic!("twice") }
    }

    let _d = Double;

    panic!("once");
}

fn template(me: &str) -> Command {
    let mut m = Command::new(me);
    m.env("IS_TEST", "1")
     .stdout(Stdio::piped())
     .stderr(Stdio::piped());
    return m;
}

fn runtest(me: &str) {
    // Make sure that the stack trace is printed
    let p = template(me).arg("fail").env("RUST_BACKTRACE", "1").spawn().unwrap();
    let out = p.wait_with_output().unwrap();
    assert!(!out.status.success());
    let s = str::from_utf8(&out.stderr).unwrap();
    assert!(s.contains("stack backtrace") && s.contains("foo::h"),
            "bad output: {}", s);

    // Make sure the stack trace is *not* printed
    // (Remove RUST_BACKTRACE from our own environment, in case developer
    // is running `make check` with it on.)
    let p = template(me).arg("fail").env_remove("RUST_BACKTRACE").spawn().unwrap();
    let out = p.wait_with_output().unwrap();
    assert!(!out.status.success());
    let s = str::from_utf8(&out.stderr).unwrap();
    assert!(!s.contains("stack backtrace") && !s.contains("foo::h"),
            "bad output2: {}", s);

    // Make sure a stack trace is printed
    let p = template(me).arg("double-fail").spawn().unwrap();
    let out = p.wait_with_output().unwrap();
    assert!(!out.status.success());
    let s = str::from_utf8(&out.stderr).unwrap();
    // loosened the following from double::h to double:: due to
    // spurious failures on mac, 32bit, optimized
    assert!(s.contains("stack backtrace") && s.contains("double::"),
            "bad output3: {}", s);

    // Make sure a stack trace isn't printed too many times
    let p = template(me).arg("double-fail")
                                .env("RUST_BACKTRACE", "1").spawn().unwrap();
    let out = p.wait_with_output().unwrap();
    assert!(!out.status.success());
    let s = str::from_utf8(&out.stderr).unwrap();
    let mut i = 0;
    for _ in 0..2 {
        i += s[i + 10..].find("stack backtrace").unwrap() + 10;
    }
    assert!(s[i + 10..].find("stack backtrace").is_none(),
            "bad output4: {}", s);
}

#[cfg(not(all(windows, target_arch = "x86")))]
fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() >= 2 && args[1] == "fail" {
        foo();
    } else if args.len() >= 2 && args[1] == "double-fail" {
        double();
    } else {
        runtest(&args[0]);
    }
}

// See issue 28218
#[cfg(all(windows, target_arch = "x86"))]
fn main() {}
