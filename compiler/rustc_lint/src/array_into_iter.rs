use crate::{LateContext, LateLintPass, LintContext};
use rustc_errors::Applicability;
use rustc_hir as hir;
use rustc_middle::ty;
use rustc_middle::ty::adjustment::{Adjust, Adjustment};
use rustc_span::symbol::sym;

declare_lint! {
    /// The `array_into_iter` lint detects calling `into_iter` on arrays.
    ///
    /// ### Example
    ///
    /// ```rust
    /// # #![allow(unused)]
    /// [1, 2, 3].into_iter().for_each(|n| { *n; });
    /// ```
    ///
    /// {{produces}}
    ///
    /// ### Explanation
    ///
    /// Since Rust 1.53, arrays implement `IntoIterator`. However, to avoid
    /// breakage, `array.into_iter()` in Rust 2015 and 2018 code will still
    /// behave as `(&array).into_iter()`, returning an iterator over
    /// references, just like in Rust 1.52 and earlier.
    /// This only applies to the method call syntax `array.into_iter()`, not to
    /// any other syntax such as `for _ in array` or `IntoIterator::into_iter(array)`.
    pub ARRAY_INTO_ITER,
    Warn,
    "detects calling `into_iter` on arrays in Rust 2015 and 2018",
}

declare_lint_pass!(
    /// Checks for instances of calling `into_iter` on arrays.
    ArrayIntoIter => [ARRAY_INTO_ITER]
);

impl<'tcx> LateLintPass<'tcx> for ArrayIntoIter {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx hir::Expr<'tcx>) {
        // We only care about method call expressions.
        if let hir::ExprKind::MethodCall(call, span, args, _) = &expr.kind {
            if call.ident.name != sym::into_iter {
                return;
            }

            // Check if the method call actually calls the libcore
            // `IntoIterator::into_iter`.
            let def_id = cx.typeck_results().type_dependent_def_id(expr.hir_id).unwrap();
            match cx.tcx.trait_of_item(def_id) {
                Some(trait_id) if cx.tcx.is_diagnostic_item(sym::IntoIterator, trait_id) => {}
                _ => return,
            };

            // As this is a method call expression, we have at least one
            // argument.
            let receiver_arg = &args[0];

            // Peel all `Box<_>` layers. We have to special case `Box` here as
            // `Box` is the only thing that values can be moved out of via
            // method call. `Box::new([1]).into_iter()` should trigger this
            // lint.
            let mut recv_ty = cx.typeck_results().expr_ty(receiver_arg);
            let mut num_box_derefs = 0;
            while recv_ty.is_box() {
                num_box_derefs += 1;
                recv_ty = recv_ty.boxed_ty();
            }

            // Make sure we found an array after peeling the boxes.
            if !matches!(recv_ty.kind(), ty::Array(..)) {
                return;
            }

            // Make sure that there is an autoref coercion at the expected
            // position. The first `num_box_derefs` adjustments are the derefs
            // of the box.
            match cx.typeck_results().expr_adjustments(receiver_arg).get(num_box_derefs) {
                Some(Adjustment { kind: Adjust::Borrow(_), .. }) => {}
                _ => return,
            }

            // Emit lint diagnostic.
            let target = match *cx.typeck_results().expr_ty_adjusted(receiver_arg).kind() {
                ty::Ref(_, inner_ty, _) if inner_ty.is_array() => "[T; N]",
                ty::Ref(_, inner_ty, _) if matches!(inner_ty.kind(), ty::Slice(..)) => "[T]",

                // We know the original first argument type is an array type,
                // we know that the first adjustment was an autoref coercion
                // and we know that `IntoIterator` is the trait involved. The
                // array cannot be coerced to something other than a reference
                // to an array or to a slice.
                _ => bug!("array type coerced to something other than array or slice"),
            };
            cx.struct_span_lint(ARRAY_INTO_ITER, *span, |lint| {
                lint.build(&format!(
                    "this method call resolves to `<&{} as IntoIterator>::into_iter` \
                    (due to backwards compatibility), \
                    but will resolve to <{} as IntoIterator>::into_iter in Rust 2021.",
                    target, target,
                ))
                .span_suggestion(
                    call.ident.span,
                    "use `.iter()` instead of `.into_iter()` to avoid ambiguity",
                    "iter".into(),
                    Applicability::MachineApplicable,
                )
                .multipart_suggestion(
                    "or use `IntoIterator::into_iter(..)` instead of `.into_iter()` to explicitly iterate by value",
                    vec![
                        (expr.span.shrink_to_lo(), "IntoIterator::into_iter(".into()),
                        (receiver_arg.span.shrink_to_hi().to(expr.span.shrink_to_hi()), ")".into()),
                    ],
                    Applicability::MaybeIncorrect,
                )
                .emit();
            })
        }
    }
}
