use super::wasm32_base;
use super::{LinkArgs, LinkerFlavor, Target, TargetOptions, PanicStrategy};

pub fn target() -> Result<Target, String> {
    let mut post_link_args = LinkArgs::new();
    post_link_args.insert(LinkerFlavor::Em,
                          vec!["-s".to_string(),
                               "ERROR_ON_UNDEFINED_SYMBOLS=1".to_string(),
                               "-s".to_string(),
                               "ASSERTIONS=1".to_string(),
                               "-s".to_string(),
                               "DISABLE_EXCEPTION_CATCHING=1".to_string(),
                               "-s".to_string(),
                               "ABORTING_MALLOC=0".to_string(),
                               // FIXME(tlively): Enable this linker option once libc type errors
                               // are resolved. See https://github.com/rust-lang/libc/pull/1478.
                               // "-Wl,--fatal-warnings".to_string(),
                               ]);

    let opts = TargetOptions {
        // emcc emits two files - a .js file to instantiate the wasm and supply platform
        // functionality, and a .wasm file.
        exe_suffix: ".js".to_string(),
        linker: None,
        linker_is_gnu: true,
        is_like_emscripten: true,
        // FIXME(tlively): Emscripten supports unwinding, but we would have to pass
        // -enable-emscripten-cxx-exceptions to LLVM at codegen time and merge
        // https://reviews.llvm.org/rG5c3cdef84b82464756bb571c13c31cf7773860c3to use it.
        panic_strategy: PanicStrategy::Abort,
        post_link_args,
        target_family: Some("unix".to_string()),
        .. wasm32_base::options()
    };
    Ok(Target {
        llvm_target: "wasm32-unknown-emscripten".to_string(),
        target_endian: "little".to_string(),
        target_pointer_width: "32".to_string(),
        target_c_int_width: "32".to_string(),
        target_os: "emscripten".to_string(),
        target_env: String::new(),
        target_vendor: "unknown".to_string(),
        data_layout: "e-m:e-p:32:32-i64:64-n32:64-S128".to_string(),
        arch: "wasm32".to_string(),
        linker_flavor: LinkerFlavor::Em,
        options: opts,
    })
}
