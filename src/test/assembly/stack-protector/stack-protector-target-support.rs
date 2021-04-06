// Test that stack smash protection code is emitted for all tier1 and tier2
// targets, with the exception of nvptx64-nvidia-cuda
//
// revisions: r1 r2 r3 r4 r5 r6 r7 r8 r9 r10 r11 r12 r13 r14 r15 r16 r17 r18 r19 r20 r21 r22 r23
// revisions: r24 r25 r26 r27 r28 r29 r30 r31 r32 r33 r34 r35 r36 r37 r38 r39 r40 r41 r42 r43 r44
// revisions: r45 r46 r47 r48 r49 r50 r51 r52 r53 r54 r55 r56 r57 r58 r59 r60 r61 r62 r63 r64 r65
// revisions: r66 r67 r68 r69 r70 r71 r72 r73 r74 r75 r76 r77 r78 r79 r80 r81 r82 r83 r84
// assembly-output: emit-asm
// needs-llvm-components: aarch64 x86 arm webassembly mips nvptx powerpc riscv systemz sparc
// min-llvm-version: 11.0.0
// compile-flags: -Z stack-protector=all
// compile-flags: -C opt-level=2

// [r1] compile-flags: --target aarch64-unknown-linux-gnu
// [r2] compile-flags: --target i686-pc-windows-gnu
// [r3] compile-flags: --target i686-pc-windows-msvc
// [r4] compile-flags: --target i686-unknown-linux-gnu
// [r5] compile-flags: --target x86_64-apple-darwin
// [r6] compile-flags: --target x86_64-pc-windows-gnu
// [r7] compile-flags: --target x86_64-pc-windows-msvc
// [r8] compile-flags: --target x86_64-unknown-linux-gnu
// [r9] compile-flags: --target aarch64-apple-darwin
// [r10] compile-flags: --target aarch64-apple-ios
// [r11] compile-flags: --target aarch64-fuchsia
// [r12] compile-flags: --target aarch64-linux-android
// [r13] compile-flags: --target aarch64-pc-windows-msvc
// [r14] compile-flags: --target aarch64-unknown-linux-musl
// [r15] compile-flags: --target aarch64-unknown-none
// [r16] compile-flags: --target aarch64-unknown-none-softfloat
// [r17] compile-flags: --target arm-linux-androideabi
// [r18] compile-flags: --target arm-unknown-linux-gnueabi
// [r19] compile-flags: --target arm-unknown-linux-gnueabihf
// [r20] compile-flags: --target arm-unknown-linux-musleabi
// [r21] compile-flags: --target arm-unknown-linux-musleabihf
// [r22] compile-flags: --target armebv7r-none-eabi
// [r23] compile-flags: --target armebv7r-none-eabihf
// [r24] compile-flags: --target armv5te-unknown-linux-gnueabi
// [r25] compile-flags: --target armv5te-unknown-linux-musleabi
// [r26] compile-flags: --target armv7-linux-androideabi
// [r27] compile-flags: --target armv7a-none-eabi
// [r28] compile-flags: --target armv7r-none-eabi
// [r29] compile-flags: --target armv7r-none-eabihf
// [r30] compile-flags: --target armv7-unknown-linux-gnueabi
// [r31] compile-flags: --target armv7-unknown-linux-gnueabihf
// [r32] compile-flags: --target armv7-unknown-linux-musleabi
// [r33] compile-flags: --target armv7-unknown-linux-musleabihf
// [r34] compile-flags: --target asmjs-unknown-emscripten
// [r35] compile-flags: --target i586-pc-windows-msvc
// [r36] compile-flags: --target i586-unknown-linux-gnu
// [r37] compile-flags: --target i586-unknown-linux-musl
// [r38] compile-flags: --target i686-linux-android
// [r39] compile-flags: --target i686-unknown-freebsd
// [r40] compile-flags: --target i686-unknown-linux-musl
// [r41] compile-flags: --target mips-unknown-linux-gnu
// [r42] compile-flags: --target mips-unknown-linux-musl
// [r43] compile-flags: --target mips64-unknown-linux-gnuabi64
// [r44] compile-flags: --target mips64-unknown-linux-muslabi64
// [r45] compile-flags: --target mips64el-unknown-linux-gnuabi64
// [r46] compile-flags: --target mips64el-unknown-linux-muslabi64
// [r47] compile-flags: --target mipsel-unknown-linux-gnu
// [r48] compile-flags: --target mipsel-unknown-linux-musl
// [r49] compile-flags: --target nvptx64-nvidia-cuda
// [r50] compile-flags: --target powerpc-unknown-linux-gnu
// [r51] compile-flags: --target powerpc64-unknown-linux-gnu
// [r52] compile-flags: --target powerpc64le-unknown-linux-gnu
// [r53] compile-flags: --target riscv32i-unknown-none-elf
// [r54] compile-flags: --target riscv32imac-unknown-none-elf
// [r55] compile-flags:--target riscv32imc-unknown-none-elf
// [r56] compile-flags:--target riscv64gc-unknown-linux-gnu
// [r57] compile-flags:--target riscv64gc-unknown-none-elf
// [r58] compile-flags:--target riscv64imac-unknown-none-elf
// [r59] compile-flags:--target s390x-unknown-linux-gnu
// [r60] compile-flags:--target sparc64-unknown-linux-gnu
// [r61] compile-flags:--target sparcv9-sun-solaris
// [r62] compile-flags:--target thumbv6m-none-eabi
// [r63] compile-flags:--target thumbv7em-none-eabi
// [r64] compile-flags:--target thumbv7em-none-eabihf
// [r65] compile-flags:--target thumbv7m-none-eabi
// [r66] compile-flags:--target thumbv7neon-linux-androideabi
// [r67] compile-flags:--target thumbv7neon-unknown-linux-gnueabihf
// [r68] compile-flags:--target thumbv8m.base-none-eabi
// [r69] compile-flags:--target thumbv8m.main-none-eabi
// [r70] compile-flags:--target thumbv8m.main-none-eabihf
// [r71] compile-flags:--target wasm32-unknown-emscripten
// [r72] compile-flags:--target wasm32-unknown-unknown
// [r73] compile-flags:--target wasm32-wasi
// [r74] compile-flags:--target x86_64-apple-ios
// [r75] compile-flags:--target x86_64-fortanix-unknown-sgx
// [r76] compile-flags:--target x86_64-fuchsia
// [r77] compile-flags:--target x86_64-linux-android
// [r78] compile-flags:--target x86_64-sun-solaris
// [r79] compile-flags:--target x86_64-unknown-freebsd
// [r80] compile-flags:--target x86_64-unknown-illumos
// [r81] compile-flags:--target x86_64-unknown-linux-gnux32
// [r82] compile-flags:--target x86_64-unknown-linux-musl
// [r83] compile-flags:--target x86_64-unknown-netbsd
// [r84] compile-flags: --target x86_64-unknown-redox

#![crate_type = "lib"]

#![feature(no_core, lang_items)]
#![crate_type = "lib"]
#![no_core]

#[lang = "sized"]
trait Sized {}
#[lang = "copy"]
trait Copy {}

#[no_mangle]
pub fn foo() {
    // CHECK: foo{{:|()}}

    // MSVC does the stack checking within a stack-check function:
    // r3: calll @__security_check_cookie
    // r7: callq __security_check_cookie
    // r13: bl __security_check_cookie
    // r35: calll @__security_check_cookie

    // cuda doesn't support stack-smash protection
    // r49-NOT: __security_check_cookie
    // r49-NOT: __stack_chk_fail

    // Other targets do stack checking within the function, and call a failure function on error
    // r1: __stack_chk_fail
    // r2: __stack_chk_fail
    // r4: __stack_chk_fail
    // r5: __stack_chk_fail
    // r6: __stack_chk_fail
    // r8: __stack_chk_fail
    // r9: __stack_chk_fail
    // r10: __stack_chk_fail
    // r11: __stack_chk_fail
    // r12: __stack_chk_fail
    // r14: __stack_chk_fail
    // r15: __stack_chk_fail
    // r16: __stack_chk_fail
    // r17: __stack_chk_fail
    // r18: __stack_chk_fail
    // r19: __stack_chk_fail
    // r20: __stack_chk_fail
    // r21: __stack_chk_fail
    // r22: __stack_chk_fail
    // r23: __stack_chk_fail
    // r24: __stack_chk_fail
    // r25: __stack_chk_fail
    // r26: __stack_chk_fail
    // r27: __stack_chk_fail
    // r28: __stack_chk_fail
    // r29: __stack_chk_fail
    // r30: __stack_chk_fail
    // r31: __stack_chk_fail
    // r32: __stack_chk_fail
    // r33: __stack_chk_fail
    // r34: __stack_chk_fail
    // r36: __stack_chk_fail
    // r37: __stack_chk_fail
    // r38: __stack_chk_fail
    // r39: __stack_chk_fail
    // r40: __stack_chk_fail
    // r41: __stack_chk_fail
    // r42: __stack_chk_fail
    // r43: __stack_chk_fail
    // r44: __stack_chk_fail
    // r45: __stack_chk_fail
    // r46: __stack_chk_fail
    // r47: __stack_chk_fail
    // r48: __stack_chk_fail
    // r50: __stack_chk_fail
    // r51: __stack_chk_fail
    // r52: __stack_chk_fail
    // r53: __stack_chk_fail
    // r54: __stack_chk_fail
    // r55: __stack_chk_fail
    // r56: __stack_chk_fail
    // r57: __stack_chk_fail
    // r58: __stack_chk_fail
    // r59: __stack_chk_fail
    // r60: __stack_chk_fail
    // r61: __stack_chk_fail
    // r62: __stack_chk_fail
    // r63: __stack_chk_fail
    // r64: __stack_chk_fail
    // r65: __stack_chk_fail
    // r66: __stack_chk_fail
    // r67: __stack_chk_fail
    // r68: __stack_chk_fail
    // r69: __stack_chk_fail
    // r70: __stack_chk_fail
    // r71: __stack_chk_fail
    // r72: __stack_chk_fail
    // r73: __stack_chk_fail
    // r74: __stack_chk_fail
    // r75: __stack_chk_fail
    // r76: __stack_chk_fail
    // r77: __stack_chk_fail
    // r78: __stack_chk_fail
    // r79: __stack_chk_fail
    // r80: __stack_chk_fail
    // r81: __stack_chk_fail
    // r82: __stack_chk_fail
    // r83: __stack_chk_fail
    // r84: __stack_chk_fail
}
