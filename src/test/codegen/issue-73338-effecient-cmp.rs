// This test checks that comparison operation
// generated by #[derive(PartialOrd)]
// doesn't contain jumps for C enums

// compile-flags: -Copt-level=3

#![crate_type="lib"]

#[repr(u32)]
#[derive(Copy, Clone, Eq, PartialEq, PartialOrd)]
pub enum Foo {
    Zero,
    One,
    Two,
}

#[no_mangle]
pub fn compare_less(a: Foo, b: Foo)->bool{
    // CHECK-NOT: br {{.*}}
    a < b
}

#[no_mangle]
pub fn compare_le(a: Foo, b: Foo)->bool{
    // CHECK-NOT: br {{.*}}
    a <= b
}

#[no_mangle]
pub fn compare_ge(a: Foo, b: Foo)->bool{
    // CHECK-NOT: br {{.*}}
    a >= b
}

#[no_mangle]
pub fn compare_greater(a: Foo, b: Foo)->bool{
    // CHECK-NOT: br {{.*}}
    a > b
}
