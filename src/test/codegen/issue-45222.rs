// compile-flags: -O
// ignore-debug: the debug assertions get in the way
// Ignored due to a codegen regression in LLVM 10, see https://bugs.llvm.org/show_bug.cgi?id=44461.
// ignore-llvm-version: 10.0

#![crate_type = "lib"]

// verify that LLVM recognizes a loop involving 0..=n and will const-fold it.

// Example from original issue #45222

fn foo2(n: u64) -> u64 {
    let mut count = 0;
    for _ in 0..n {
        for j in (0..=n).rev() {
            count += j;
        }
    }
    count
}

// CHECK-LABEL: @check_foo2
#[no_mangle]
pub fn check_foo2() -> u64 {
    // CHECK: ret i64 500005000000000
    foo2(100000)
}

// Simplified example of #45222

fn triangle_inc(n: u64) -> u64 {
    let mut count = 0;
    for j in 0 ..= n {
        count += j;
    }
    count
}

// CHECK-LABEL: @check_triangle_inc
#[no_mangle]
pub fn check_triangle_inc() -> u64 {
    // CHECK: ret i64 5000050000
    triangle_inc(100000)
}

// Demo in #48012

fn foo3r(n: u64) -> u64 {
    let mut count = 0;
    (0..n).for_each(|_| {
        (0 ..= n).rev().for_each(|j| {
            count += j;
        })
    });
    count
}

// CHECK-LABEL: @check_foo3r
#[no_mangle]
pub fn check_foo3r() -> u64 {
    // CHECK: ret i64 500050000000
    foo3r(10000)
}
