use crate::spec::TargetOptions;

pub fn opts() -> TargetOptions {
    TargetOptions {
        os: "vxworks".to_string(),
        env: "gnu".to_string(),
        vendor: "wrs".to_string(),
        linker: Some("wr-c++".to_string()),
        exe_suffix: ".vxe".to_string(),
        dynamic_linking: true,
        executables: true,
        os_family: Some("unix".to_string()),
        linker_is_gnu: true,
        has_rpath: true,
        position_independent_executables: false,
        has_elf_tls: true,
        crt_static_default: true,
        crt_static_respected: true,
        crt_static_allows_dylibs: true,
        // VxWorks needs to implement this to support profiling
        mcount: "_mcount".to_string(),
        ..Default::default()
    }
}
