//! This module contains the `InterpCx` methods for executing a single step of the interpreter.
//!
//! The main entry point is the `step` method.

use crate::interpret::OpTy;
use rustc_middle::mir;
use rustc_middle::mir::interpret::{InterpResult, Scalar};
use rustc_target::abi::LayoutOf;

use super::{InterpCx, Machine};

/// Classify whether an operator is "left-homogeneous", i.e., the LHS has the
/// same type as the result.
#[inline]
fn binop_left_homogeneous(op: mir::BinOp) -> bool {
    use rustc_middle::mir::BinOp::*;
    match op {
        Add | Sub | Mul | Div | Rem | BitXor | BitAnd | BitOr | Offset | Shl | Shr => true,
        Eq | Ne | Lt | Le | Gt | Ge => false,
    }
}
/// Classify whether an operator is "right-homogeneous", i.e., the RHS has the
/// same type as the LHS.
#[inline]
fn binop_right_homogeneous(op: mir::BinOp) -> bool {
    use rustc_middle::mir::BinOp::*;
    match op {
        Add | Sub | Mul | Div | Rem | BitXor | BitAnd | BitOr | Eq | Ne | Lt | Le | Gt | Ge => true,
        Offset | Shl | Shr => false,
    }
}

impl<'mir, 'tcx: 'mir, M: Machine<'mir, 'tcx>> InterpCx<'mir, 'tcx, M> {
    pub fn run(&mut self) -> InterpResult<'tcx> {
        while self.step()? {}
        Ok(())
    }

    /// Returns `true` as long as there are more things to do.
    ///
    /// This is used by [priroda](https://github.com/oli-obk/priroda)
    ///
    /// This is marked `#inline(always)` to work around adverserial codegen when `opt-level = 3`
    #[inline(always)]
    pub fn step(&mut self) -> InterpResult<'tcx, bool> {
        if self.stack().is_empty() {
            return Ok(false);
        }

        let loc = match self.frame().loc {
            Ok(loc) => loc,
            Err(_) => {
                // We are unwinding and this fn has no cleanup code.
                // Just go on unwinding.
                trace!("unwinding: skipping frame");
                self.pop_stack_frame(/* unwinding */ true)?;
                return Ok(true);
            }
        };
        let basic_block = &self.body().basic_blocks()[loc.block];

        let old_frames = self.frame_idx();

        if let Some(stmt) = basic_block.statements.get(loc.statement_index) {
            assert_eq!(old_frames, self.frame_idx());
            self.statement(stmt)?;
            return Ok(true);
        }

        M::before_terminator(self)?;

        let terminator = basic_block.terminator();
        assert_eq!(old_frames, self.frame_idx());
        self.terminator(terminator)?;
        Ok(true)
    }

    /// Runs the interpretation logic for the given `mir::Statement` at the current frame and
    /// statement counter. This also moves the statement counter forward.
    crate fn statement(&mut self, stmt: &mir::Statement<'tcx>) -> InterpResult<'tcx> {
        info!("{:?}", stmt);

        use rustc_middle::mir::StatementKind::*;

        // Some statements (e.g., box) push new stack frames.
        // We have to record the stack frame number *before* executing the statement.
        let frame_idx = self.frame_idx();

        match &stmt.kind {
            Assign(box (place, rvalue)) => self.eval_rvalue_into_place(rvalue, *place)?,

            SetDiscriminant { place, variant_index } => {
                let dest = self.eval_place(**place)?;
                self.write_discriminant(*variant_index, &dest)?;
            }

            // Mark locals as alive
            StorageLive(local) => {
                self.storage_live(*local)?;
            }

            // Mark locals as dead
            StorageDead(local) => {
                self.storage_dead(*local)?;
            }

            // No dynamic semantics attached to `FakeRead`; MIR
            // interpreter is solely intended for borrowck'ed code.
            FakeRead(..) => {}

            // Stacked Borrows.
            Retag(kind, place) => {
                let dest = self.eval_place(**place)?;
                M::retag(self, *kind, &dest)?;
            }

            // Call CopyNonOverlapping
            CopyNonOverlapping(box rustc_middle::mir::CopyNonOverlapping { src, dst, count }) => {
                let src = self.eval_operand(src, None)?;
                let dst = self.eval_operand(dst, None)?;
                let count = self.eval_operand(count, None)?;
                self.copy(&src, &dst, &count, /* nonoverlapping */ true)?;
            }

            // Statements we do not track.
            AscribeUserType(..) => {}

            // Currently, Miri discards Coverage statements. Coverage statements are only injected
            // via an optional compile time MIR pass and have no side effects. Since Coverage
            // statements don't exist at the source level, it is safe for Miri to ignore them, even
            // for undefined behavior (UB) checks.
            //
            // A coverage counter inside a const expression (for example, a counter injected in a
            // const function) is discarded when the const is evaluated at compile time. Whether
            // this should change, and/or how to implement a const eval counter, is a subject of the
            // following issue:
            //
            // FIXME(#73156): Handle source code coverage in const eval
            Coverage(..) => {}

            // Defined to do nothing. These are added by optimization passes, to avoid changing the
            // size of MIR constantly.
            Nop => {}

            LlvmInlineAsm { .. } => throw_unsup_format!("inline assembly is not supported"),
        }

        self.stack_mut()[frame_idx].loc.as_mut().unwrap().statement_index += 1;
        Ok(())
    }

    pub(crate) fn copy(
        &mut self,
        src: &OpTy<'tcx, <M as Machine<'mir, 'tcx>>::PointerTag>,
        dst: &OpTy<'tcx, <M as Machine<'mir, 'tcx>>::PointerTag>,
        count: &OpTy<'tcx, <M as Machine<'mir, 'tcx>>::PointerTag>,
        nonoverlapping: bool,
    ) -> InterpResult<'tcx> {
        let count = self.read_scalar(&count)?.to_machine_usize(self)?;
        let layout = self.layout_of(src.layout.ty.builtin_deref(true).unwrap().ty)?;
        let (size, align) = (layout.size, layout.align.abi);
        let size = size.checked_mul(count, self).ok_or_else(|| {
            err_ub_format!(
                "overflow computing total size of `{}`",
                if nonoverlapping { "copy_nonoverlapping" } else { "copy" }
            )
        })?;

        // Make sure we check both pointers for an access of the total size and aligment,
        // *even if* the total size is 0.
        let src =
            self.memory.check_ptr_access(self.read_scalar(&src)?.check_init()?, size, align)?;

        let dst =
            self.memory.check_ptr_access(self.read_scalar(&dst)?.check_init()?, size, align)?;

        if let (Some(src), Some(dst)) = (src, dst) {
            self.memory.copy(src, dst, size, nonoverlapping)?;
        }
        Ok(())
    }

    /// Evaluate an assignment statement.
    ///
    /// There is no separate `eval_rvalue` function. Instead, the code for handling each rvalue
    /// type writes its results directly into the memory specified by the place.
    pub fn eval_rvalue_into_place(
        &mut self,
        rvalue: &mir::Rvalue<'tcx>,
        place: mir::Place<'tcx>,
    ) -> InterpResult<'tcx> {
        let dest = self.eval_place(place)?;

        use rustc_middle::mir::Rvalue::*;
        match *rvalue {
            ThreadLocalRef(did) => {
                let id = M::thread_local_static_alloc_id(self, did)?;
                let val = self.global_base_pointer(id.into())?;
                self.write_scalar(val, &dest)?;
            }

            Use(ref operand) => {
                // Avoid recomputing the layout
                let op = self.eval_operand(operand, Some(dest.layout))?;
                self.copy_op(&op, &dest)?;
            }

            BinaryOp(bin_op, box (ref left, ref right)) => {
                let layout = binop_left_homogeneous(bin_op).then_some(dest.layout);
                let left = self.read_immediate(&self.eval_operand(left, layout)?)?;
                let layout = binop_right_homogeneous(bin_op).then_some(left.layout);
                let right = self.read_immediate(&self.eval_operand(right, layout)?)?;
                self.binop_ignore_overflow(bin_op, &left, &right, &dest)?;
            }

            CheckedBinaryOp(bin_op, box (ref left, ref right)) => {
                // Due to the extra boolean in the result, we can never reuse the `dest.layout`.
                let left = self.read_immediate(&self.eval_operand(left, None)?)?;
                let layout = binop_right_homogeneous(bin_op).then_some(left.layout);
                let right = self.read_immediate(&self.eval_operand(right, layout)?)?;
                self.binop_with_overflow(bin_op, &left, &right, &dest)?;
            }

            UnaryOp(un_op, ref operand) => {
                // The operand always has the same type as the result.
                let val = self.read_immediate(&self.eval_operand(operand, Some(dest.layout))?)?;
                let val = self.unary_op(un_op, &val)?;
                assert_eq!(val.layout, dest.layout, "layout mismatch for result of {:?}", un_op);
                self.write_immediate(*val, &dest)?;
            }

            Aggregate(ref kind, ref operands) => {
                let (dest, active_field_index) = match **kind {
                    mir::AggregateKind::Adt(adt_def, variant_index, _, _, active_field_index) => {
                        self.write_discriminant(variant_index, &dest)?;
                        if adt_def.is_enum() {
                            (self.place_downcast(&dest, variant_index)?, active_field_index)
                        } else {
                            (dest, active_field_index)
                        }
                    }
                    _ => (dest, None),
                };

                for (i, operand) in operands.iter().enumerate() {
                    let op = self.eval_operand(operand, None)?;
                    // Ignore zero-sized fields.
                    if !op.layout.is_zst() {
                        let field_index = active_field_index.unwrap_or(i);
                        let field_dest = self.place_field(&dest, field_index)?;
                        self.copy_op(&op, &field_dest)?;
                    }
                }
            }

            Repeat(ref operand, _) => {
                let op = self.eval_operand(operand, None)?;
                let dest = self.force_allocation(&dest)?;
                let length = dest.len(self)?;

                if let Some(first_ptr) = self.check_mplace_access(&dest, None)? {
                    // Write the first.
                    let first = self.mplace_field(&dest, 0)?;
                    self.copy_op(&op, &first.into())?;

                    if length > 1 {
                        let elem_size = first.layout.size;
                        // Copy the rest. This is performance-sensitive code
                        // for big static/const arrays!
                        let rest_ptr = first_ptr.offset(elem_size, self)?;
                        self.memory.copy_repeatedly(
                            first_ptr,
                            rest_ptr,
                            elem_size,
                            length - 1,
                            /*nonoverlapping:*/ true,
                        )?;
                    }
                }
            }

            Len(place) => {
                // FIXME(CTFE): don't allow computing the length of arrays in const eval
                let src = self.eval_place(place)?;
                let mplace = self.force_allocation(&src)?;
                let len = mplace.len(self)?;
                self.write_scalar(Scalar::from_machine_usize(len, self), &dest)?;
            }

            AddressOf(_, place) | Ref(_, _, place) => {
                let src = self.eval_place(place)?;
                let place = self.force_allocation(&src)?;
                if place.layout.size.bytes() > 0 {
                    // definitely not a ZST
                    assert!(place.ptr.is_ptr(), "non-ZST places should be normalized to `Pointer`");
                }
                self.write_immediate(place.to_ref(), &dest)?;
            }

            NullaryOp(mir::NullOp::Box, _) => {
                M::box_alloc(self, &dest)?;
            }

            NullaryOp(mir::NullOp::SizeOf, ty) => {
                let ty = self.subst_from_current_frame_and_normalize_erasing_regions(ty);
                let layout = self.layout_of(ty)?;
                if layout.is_unsized() {
                    // FIXME: This should be a span_bug (#80742)
                    self.tcx.sess.delay_span_bug(
                        self.frame().current_span(),
                        &format!("SizeOf nullary MIR operator called for unsized type {}", ty),
                    );
                    throw_inval!(SizeOfUnsizedType(ty));
                }
                self.write_scalar(Scalar::from_machine_usize(layout.size.bytes(), self), &dest)?;
            }

            Cast(cast_kind, ref operand, cast_ty) => {
                let src = self.eval_operand(operand, None)?;
                let cast_ty = self.subst_from_current_frame_and_normalize_erasing_regions(cast_ty);
                self.cast(&src, cast_kind, cast_ty, &dest)?;
            }

            Discriminant(place) => {
                let op = self.eval_place_to_op(place, None)?;
                let discr_val = self.read_discriminant(&op)?.0;
                self.write_scalar(discr_val, &dest)?;
            }
        }

        trace!("{:?}", self.dump_place(*dest));

        Ok(())
    }

    fn terminator(&mut self, terminator: &mir::Terminator<'tcx>) -> InterpResult<'tcx> {
        info!("{:?}", terminator.kind);

        self.eval_terminator(terminator)?;
        if !self.stack().is_empty() {
            if let Ok(loc) = self.frame().loc {
                info!("// executing {:?}", loc.block);
            }
        }
        Ok(())
    }
}
