// Targets the Cortex-M3 processor (ARMv7-M)

use spec::{LinkerFlavor, Target, TargetOptions, TargetResult};

pub fn target() -> TargetResult {
    Ok(Target {
        llvm_target: "thumbv7m-none-eabi".to_string(),
        target_endian: "little".to_string(),
        target_pointer_width: "32".to_string(),
        target_c_int_width: "32".to_string(),
        data_layout: "e-m:e-p:32:32-i64:64-v128:64:128-a:0:32-n32-S64".to_string(),
        arch: "arm".to_string(),
        target_os: "none".to_string(),
        target_env: "".to_string(),
        target_vendor: "".to_string(),
        linker_flavor: LinkerFlavor::Gcc,

        options: TargetOptions {
            max_atomic_width: Some(32),
            .. super::thumb_base::opts()
        },
    })
}
