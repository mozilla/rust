//! lint on C-like enums that are `repr(isize/usize)` and have values that
//! don't fit into an `i32`

use crate::consts::{miri_to_const, Constant};
use crate::utils::span_lint;
use rustc_ast::ast::{IntTy, UintTy};
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty;
use rustc_middle::ty::util::IntTypeExt;
use rustc_session::{declare_lint_pass, declare_tool_lint};
use std::convert::TryFrom;

declare_clippy_lint! {
    /// **What it does:** Checks for C-like enumerations that are
    /// `repr(isize/usize)` and have values that don't fit into an `i32`.
    ///
    /// **Why is this bad?** This will truncate the variant value on 32 bit
    /// architectures, but works fine on 64 bit.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    /// ```rust
    /// # #[cfg(target_pointer_width = "64")]
    /// #[repr(usize)]
    /// enum NonPortable {
    ///     X = 0x1_0000_0000,
    ///     Y = 0,
    /// }
    /// ```
    pub ENUM_CLIKE_UNPORTABLE_VARIANT,
    correctness,
    "C-like enums that are `repr(isize/usize)` and have values that don't fit into an `i32`"
}

declare_lint_pass!(UnportableVariant => [ENUM_CLIKE_UNPORTABLE_VARIANT]);

impl<'tcx> LateLintPass<'tcx> for UnportableVariant {
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap, clippy::cast_sign_loss)]
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'_>) {
        if cx.tcx.data_layout.pointer_size.bits() != 64 {
            return;
        }
        if let ItemKind::Enum(def, _) = &item.kind {
            for var in def.variants {
                if let Some(anon_const) = &var.disr_expr {
                    let def_id = cx.tcx.hir().body_owner_def_id(anon_const.body);
                    let mut ty = cx.tcx.type_of(def_id.to_def_id());
                    let constant = cx
                        .tcx
                        .const_eval_poly(def_id.to_def_id())
                        .ok()
                        .map(|val| rustc_middle::ty::Const::from_value(cx.tcx, val, ty));
                    if let Some(Constant::Int(val)) = constant.and_then(miri_to_const) {
                        if let ty::Adt(adt, _) = ty.kind() {
                            if adt.is_enum() {
                                ty = adt.repr.discr_type().to_ty(cx.tcx);
                            }
                        }
                        match ty.kind() {
                            ty::Int(IntTy::Isize) => {
                                let val = ((val as i128) << 64) >> 64;
                                if i32::try_from(val).is_ok() {
                                    continue;
                                }
                            },
                            ty::Uint(UintTy::Usize) if val > u128::from(u32::MAX) => {},
                            _ => continue,
                        }
                        span_lint(
                            cx,
                            ENUM_CLIKE_UNPORTABLE_VARIANT,
                            cx.tcx.hir().span(var.id),
                            "C-like enum variant discriminant is not portable to 32-bit targets",
                        );
                    };
                }
            }
        }
    }
}
