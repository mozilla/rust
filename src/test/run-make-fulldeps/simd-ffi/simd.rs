// ensures that public symbols are not removed completely
#![crate_type = "lib"]
// we can compile to a variety of platforms, because we don't need
// cross-compiled standard libraries.
#![feature(no_core, auto_traits)]
#![no_core]
#![feature(
    mips_target_feature,
    repr_simd,
    simd_ffi,
    link_llvm_intrinsics,
    lang_items,
    rustc_attrs
)]

#[derive(Copy)]
#[repr(simd)]
pub struct F32x4(f32, f32, f32, f32);

extern "C" {
    #[link_name = "llvm.sqrt.v4f32"]
    fn vsqrt(x: F32x4) -> F32x4;
}

pub fn foo(x: F32x4) -> F32x4 {
    unsafe { vsqrt(x) }
}

#[derive(Copy)]
#[repr(simd)]
pub struct I32x4(i32, i32, i32, i32);

extern "C" {
    // _mm_sll_epi32
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[link_name = "llvm.x86.sse2.psll.d"]
    fn integer(a: I32x4, b: I32x4) -> I32x4;

    // vmaxq_s32
    #[cfg(target_arch = "arm")]
    #[link_name = "llvm.arm.neon.vmaxs.v4i32"]
    fn integer(a: I32x4, b: I32x4) -> I32x4;
    // vmaxq_s32
    #[cfg(target_arch = "aarch64")]
    #[link_name = "llvm.aarch64.neon.maxs.v4i32"]
    fn integer(a: I32x4, b: I32x4) -> I32x4;

    // just some substitute foreign symbol, not an LLVM intrinsic; so
    // we still get type checking, but not as detailed as (ab)using
    // LLVM.
    #[cfg(target_arch = "mips")]
    #[target_feature(enable = "msa")]
    fn integer(a: I32x4, b: I32x4) -> I32x4;
}

pub fn bar(a: I32x4, b: I32x4) -> I32x4 {
    unsafe { integer(a, b) }
}

#[lang = "sized"]
pub trait Sized {}

#[lang = "copy"]
pub trait Copy {}

impl Copy for f32 {}
impl Copy for i32 {}

pub mod marker {
    pub use Copy;
}

#[lang = "freeze"]
auto trait Freeze {}

#[macro_export]
#[rustc_builtin_macro]
macro_rules! Copy {
    () => {};
}
#[macro_export]
#[rustc_builtin_macro]
macro_rules! derive {
    () => {};
}
