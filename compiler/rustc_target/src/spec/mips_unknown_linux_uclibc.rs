use crate::spec::{Target, TargetOptions};

pub fn target() -> Target {
    Target {
        llvm_target: "mips-unknown-linux-uclibc".to_string(),
        pointer_width: 32,
        data_layout: "E-m:m-p:32:32-i8:8:32-i16:16:32-i64:64-n32-S64".to_string(),
        arch: "mips".to_string(),
        options: TargetOptions {
            endian: "big".to_string(),
            cpu: "mips32r2".to_string(),
            features: "+mips32r2,+soft-float".to_string(),
            max_atomic_width: Some(32),
            mcount: "_mcount".to_string(),

            ..super::linux_uclibc_base::opts()
        },
    }
}
