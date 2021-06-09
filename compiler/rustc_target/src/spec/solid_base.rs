use super::LinkerFlavor;
use crate::spec::{RelroLevel, TargetOptions};

pub fn opts(kernel: &str) -> TargetOptions {
    TargetOptions {
        os: format!("solid-{}", kernel),
        vendor: "kmc".to_string(),
        exe_suffix: ".out".to_string(),
        linker_is_gnu: true,
        linker_flavor: LinkerFlavor::Gcc,
        has_rpath: false,
        crt_static_default: true,
        crt_static_respected: true,
        has_elf_tls: true,
        dynamic_linking: true,
        executables: false,
        position_independent_executables: true,
        relro_level: RelroLevel::Full,
        ..Default::default()
    }
}
