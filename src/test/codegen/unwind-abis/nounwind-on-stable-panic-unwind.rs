// compile-flags: -C opt-level=0
// ignore-wasm32-bare compiled with panic=abort by default

#![crate_type = "lib"]

// We disable optimizations to prevent LLVM from infering the attribute.

extern "C" {
    fn bar();
}

// CHECK-NOT: Function Attrs:{{.*}}nounwind
pub unsafe extern "C" fn foo() {
    bar();
}

// Note that this test will get removed when `C-unwind` is fully stabilized
