use crate::spec::TargetOptions;

pub fn opts() -> TargetOptions {
    let mut base = super::linux_gnu_base::opts();
    base.os = "android".to_string();
    base.dwarf_version = Some(2);
    base.position_independent_executables = true;
    base.has_elf_tls = false;
    // This is for backward compatibility, see https://github.com/rust-lang/rust/issues/49867
    // for context. (At that time, there was no `-C force-unwind-tables`, so the only solution
    // was to always emit `uwtable`).
    base.default_uwtable = true;
    base.crt_static_respected = false;
    base
}
