use std::borrow::Cow;
use std::convert::TryFrom;

use rustc_middle::ty::layout::TyAndLayout;
use rustc_middle::ty::Instance;
use rustc_middle::{mir, ty};
use rustc_span::source_map::Span;
use rustc_target::abi::{self, LayoutOf};
use rustc_target::spec::abi::Abi;

use super::{
    FnVal, ImmTy, InterpCx, InterpResult, MPlaceTy, Machine, OpTy, PlaceTy, StackPopCleanup,
};

impl<'mir, 'tcx, M: Machine<'mir, 'tcx>> InterpCx<'mir, 'tcx, M> {
    pub(super) fn eval_terminator(
        &mut self,
        terminator: &mir::Terminator<'tcx>,
    ) -> InterpResult<'tcx> {
        use rustc_middle::mir::TerminatorKind::*;
        match terminator.kind {
            Return => {
                self.frame().return_place.map(|r| self.dump_place(*r));
                self.pop_stack_frame(/* unwinding */ false)?
            }

            Goto { target } => self.go_to_block(target),

            SwitchInt { ref discr, ref values, ref targets, .. } => {
                let discr = self.read_immediate(self.eval_operand(discr, None)?)?;
                trace!("SwitchInt({:?})", *discr);

                // Branch to the `otherwise` case by default, if no match is found.
                assert!(!targets.is_empty());
                let mut target_block = targets[targets.len() - 1];

                for (index, &const_int) in values.iter().enumerate() {
                    // Compare using binary_op, to also support pointer values
                    let res = self
                        .overflowing_binary_op(
                            mir::BinOp::Eq,
                            discr,
                            ImmTy::from_uint(const_int, discr.layout),
                        )?
                        .0;
                    if res.to_bool()? {
                        target_block = targets[index];
                        break;
                    }
                }

                self.go_to_block(target_block);
            }

            Call { ref func, ref args, ref destination, ref cleanup, .. } => {
                let func = self.eval_operand(func, None)?;
                let (fn_val, abi) = match func.layout.ty.kind {
                    ty::FnPtr(sig) => {
                        let caller_abi = sig.abi();
                        let fn_ptr = self.read_scalar(func)?.not_undef()?;
                        let fn_val = self.memory.get_fn(fn_ptr)?;
                        (fn_val, caller_abi)
                    }
                    ty::FnDef(def_id, substs) => {
                        let sig = func.layout.ty.fn_sig(*self.tcx);
                        (FnVal::Instance(self.resolve(def_id, substs)?), sig.abi())
                    }
                    _ => bug!("invalid callee of type {:?}", func.layout.ty),
                };
                let args = self.eval_operands(args)?;
                let ret = match destination {
                    Some((dest, ret)) => Some((self.eval_place(dest)?, *ret)),
                    None => None,
                };
                self.eval_fn_call(
                    fn_val,
                    terminator.source_info.span,
                    abi,
                    &args[..],
                    ret,
                    *cleanup,
                )?;
            }

            Drop { ref location, target, unwind } => {
                // FIXME(CTFE): forbid drop in const eval
                let place = self.eval_place(location)?;
                let ty = place.layout.ty;
                trace!("TerminatorKind::drop: {:?}, type {}", location, ty);

                let instance = Instance::resolve_drop_in_place(*self.tcx, ty);
                self.drop_in_place(place, instance, terminator.source_info.span, target, unwind)?;
            }

            Assert { ref cond, expected, ref msg, target, cleanup } => {
                let cond_val =
                    self.read_immediate(self.eval_operand(cond, None)?)?.to_scalar()?.to_bool()?;
                if expected == cond_val {
                    self.go_to_block(target);
                } else {
                    M::assert_panic(self, msg, cleanup)?;
                }
            }

            Abort => {
                M::abort(self)?;
            }

            // When we encounter Resume, we've finished unwinding
            // cleanup for the current stack frame. We pop it in order
            // to continue unwinding the next frame
            Resume => {
                trace!("unwinding: resuming from cleanup");
                // By definition, a Resume terminator means
                // that we're unwinding
                self.pop_stack_frame(/* unwinding */ true)?;
                return Ok(());
            }

            // It is UB to ever encounter this.
            Unreachable => throw_ub!(Unreachable),

            // These should never occur for MIR we actually run.
            DropAndReplace { .. }
            | FalseEdges { .. }
            | FalseUnwind { .. }
            | Yield { .. }
            | GeneratorDrop => {
                bug!("{:#?} should have been eliminated by MIR pass", terminator.kind)
            }
        }

        Ok(())
    }

    fn check_argument_compat(
        rust_abi: bool,
        caller: TyAndLayout<'tcx>,
        callee: TyAndLayout<'tcx>,
    ) -> bool {
        if caller.ty == callee.ty {
            // No question
            return true;
        }
        if !rust_abi {
            // Don't risk anything
            return false;
        }
        // Compare layout
        match (&caller.abi, &callee.abi) {
            // Different valid ranges are okay (once we enforce validity,
            // that will take care to make it UB to leave the range, just
            // like for transmute).
            (abi::Abi::Scalar(ref caller), abi::Abi::Scalar(ref callee)) => {
                caller.value == callee.value
            }
            (
                abi::Abi::ScalarPair(ref caller1, ref caller2),
                abi::Abi::ScalarPair(ref callee1, ref callee2),
            ) => caller1.value == callee1.value && caller2.value == callee2.value,
            // Be conservative
            _ => false,
        }
    }

    /// Pass a single argument, checking the types for compatibility.
    fn pass_argument(
        &mut self,
        rust_abi: bool,
        caller_arg: &mut impl Iterator<Item = OpTy<'tcx, M::PointerTag>>,
        callee_arg: PlaceTy<'tcx, M::PointerTag>,
    ) -> InterpResult<'tcx> {
        if rust_abi && callee_arg.layout.is_zst() {
            // Nothing to do.
            trace!("Skipping callee ZST");
            return Ok(());
        }
        let caller_arg = caller_arg.next().ok_or_else(|| {
            err_ub_format!("calling a function with fewer arguments than it requires")
        })?;
        if rust_abi {
            assert!(!caller_arg.layout.is_zst(), "ZSTs must have been already filtered out");
        }
        // Now, check
        if !Self::check_argument_compat(rust_abi, caller_arg.layout, callee_arg.layout) {
            throw_ub_format!(
                "calling a function with argument of type {:?} passing data of type {:?}",
                callee_arg.layout.ty,
                caller_arg.layout.ty
            )
        }
        // We allow some transmutes here
        self.copy_op_transmute(caller_arg, callee_arg)
    }

    /// Call this function -- pushing the stack frame and initializing the arguments.
    fn eval_fn_call(
        &mut self,
        fn_val: FnVal<'tcx, M::ExtraFnVal>,
        span: Span,
        caller_abi: Abi,
        args: &[OpTy<'tcx, M::PointerTag>],
        ret: Option<(PlaceTy<'tcx, M::PointerTag>, mir::BasicBlock)>,
        unwind: Option<mir::BasicBlock>,
    ) -> InterpResult<'tcx> {
        trace!("eval_fn_call: {:#?}", fn_val);

        let instance = match fn_val {
            FnVal::Instance(instance) => instance,
            FnVal::Other(extra) => {
                return M::call_extra_fn(self, extra, args, ret, unwind);
            }
        };

        // ABI check
        {
            let callee_abi = {
                let instance_ty = instance.ty_env(*self.tcx, self.param_env);
                match instance_ty.kind {
                    ty::FnDef(..) => instance_ty.fn_sig(*self.tcx).abi(),
                    ty::Closure(..) => Abi::RustCall,
                    ty::Generator(..) => Abi::Rust,
                    _ => bug!("unexpected callee ty: {:?}", instance_ty),
                }
            };
            let normalize_abi = |abi| match abi {
                Abi::Rust | Abi::RustCall | Abi::RustIntrinsic | Abi::PlatformIntrinsic =>
                // These are all the same ABI, really.
                {
                    Abi::Rust
                }
                abi => abi,
            };
            if normalize_abi(caller_abi) != normalize_abi(callee_abi) {
                throw_ub_format!(
                    "calling a function with ABI {:?} using caller ABI {:?}",
                    callee_abi,
                    caller_abi
                )
            }
        }

        match instance.def {
            ty::InstanceDef::Intrinsic(..) => {
                assert!(caller_abi == Abi::RustIntrinsic || caller_abi == Abi::PlatformIntrinsic);
                M::call_intrinsic(self, span, instance, args, ret, unwind)
            }
            ty::InstanceDef::VtableShim(..)
            | ty::InstanceDef::ReifyShim(..)
            | ty::InstanceDef::ClosureOnceShim { .. }
            | ty::InstanceDef::FnPtrShim(..)
            | ty::InstanceDef::DropGlue(..)
            | ty::InstanceDef::CloneShim(..)
            | ty::InstanceDef::Item(_) => {
                // We need MIR for this fn
                let body = match M::find_mir_or_eval_fn(self, span, instance, args, ret, unwind)? {
                    Some(body) => body,
                    None => return Ok(()),
                };

                self.push_stack_frame(
                    instance,
                    span,
                    body,
                    ret.map(|p| p.0),
                    StackPopCleanup::Goto { ret: ret.map(|p| p.1), unwind },
                )?;

                // If an error is raised here, pop the frame again to get an accurate backtrace.
                // To this end, we wrap it all in a `try` block.
                let res: InterpResult<'tcx> = try {
                    trace!(
                        "caller ABI: {:?}, args: {:#?}",
                        caller_abi,
                        args.iter()
                            .map(|arg| (arg.layout.ty, format!("{:?}", **arg)))
                            .collect::<Vec<_>>()
                    );
                    trace!(
                        "spread_arg: {:?}, locals: {:#?}",
                        body.spread_arg,
                        body.args_iter()
                            .map(|local| (
                                local,
                                self.layout_of_local(self.frame(), local, None).unwrap().ty
                            ))
                            .collect::<Vec<_>>()
                    );

                    // Figure out how to pass which arguments.
                    // The Rust ABI is special: ZST get skipped.
                    let rust_abi = match caller_abi {
                        Abi::Rust | Abi::RustCall => true,
                        _ => false,
                    };
                    // We have two iterators: Where the arguments come from,
                    // and where they go to.

                    // For where they come from: If the ABI is RustCall, we untuple the
                    // last incoming argument.  These two iterators do not have the same type,
                    // so to keep the code paths uniform we accept an allocation
                    // (for RustCall ABI only).
                    let caller_args: Cow<'_, [OpTy<'tcx, M::PointerTag>]> =
                        if caller_abi == Abi::RustCall && !args.is_empty() {
                            // Untuple
                            let (&untuple_arg, args) = args.split_last().unwrap();
                            trace!("eval_fn_call: Will pass last argument by untupling");
                            Cow::from(
                                args.iter()
                                    .map(|&a| Ok(a))
                                    .chain(
                                        (0..untuple_arg.layout.fields.count())
                                            .map(|i| self.operand_field(untuple_arg, i)),
                                    )
                                    .collect::<InterpResult<'_, Vec<OpTy<'tcx, M::PointerTag>>>>(
                                    )?,
                            )
                        } else {
                            // Plain arg passing
                            Cow::from(args)
                        };
                    // Skip ZSTs
                    let mut caller_iter =
                        caller_args.iter().filter(|op| !rust_abi || !op.layout.is_zst()).copied();

                    // Now we have to spread them out across the callee's locals,
                    // taking into account the `spread_arg`.  If we could write
                    // this is a single iterator (that handles `spread_arg`), then
                    // `pass_argument` would be the loop body. It takes care to
                    // not advance `caller_iter` for ZSTs.
                    for local in body.args_iter() {
                        let dest = self.eval_place(&mir::Place::from(local))?;
                        if Some(local) == body.spread_arg {
                            // Must be a tuple
                            for i in 0..dest.layout.fields.count() {
                                let dest = self.place_field(dest, i)?;
                                self.pass_argument(rust_abi, &mut caller_iter, dest)?;
                            }
                        } else {
                            // Normal argument
                            self.pass_argument(rust_abi, &mut caller_iter, dest)?;
                        }
                    }
                    // Now we should have no more caller args
                    if caller_iter.next().is_some() {
                        throw_ub_format!("calling a function with more arguments than it expected")
                    }
                    // Don't forget to check the return type!
                    if let Some((caller_ret, _)) = ret {
                        let callee_ret = self.eval_place(&mir::Place::return_place())?;
                        if !Self::check_argument_compat(
                            rust_abi,
                            caller_ret.layout,
                            callee_ret.layout,
                        ) {
                            throw_ub_format!(
                                "calling a function with return type {:?} passing \
                                     return place of type {:?}",
                                callee_ret.layout.ty,
                                caller_ret.layout.ty
                            )
                        }
                    } else {
                        let local = mir::RETURN_PLACE;
                        let callee_layout = self.layout_of_local(self.frame(), local, None)?;
                        if !callee_layout.abi.is_uninhabited() {
                            throw_ub_format!("calling a returning function without a return place")
                        }
                    }
                };
                match res {
                    Err(err) => {
                        self.stack.pop();
                        Err(err)
                    }
                    Ok(()) => Ok(()),
                }
            }
            // cannot use the shim here, because that will only result in infinite recursion
            ty::InstanceDef::Virtual(_, idx) => {
                let mut args = args.to_vec();
                // We have to implement all "object safe receivers".  Currently we
                // support built-in pointers (&, &mut, Box) as well as unsized-self.  We do
                // not yet support custom self types.
                // Also see librustc_codegen_llvm/abi.rs and librustc_codegen_llvm/mir/block.rs.
                let receiver_place = match args[0].layout.ty.builtin_deref(true) {
                    Some(_) => {
                        // Built-in pointer.
                        self.deref_operand(args[0])?
                    }
                    None => {
                        // Unsized self.
                        args[0].assert_mem_place(self)
                    }
                };
                // Find and consult vtable
                let vtable = receiver_place.vtable();
                let drop_fn = self.get_vtable_slot(vtable, u64::try_from(idx).unwrap())?;

                // `*mut receiver_place.layout.ty` is almost the layout that we
                // want for args[0]: We have to project to field 0 because we want
                // a thin pointer.
                assert!(receiver_place.layout.is_unsized());
                let receiver_ptr_ty = self.tcx.mk_mut_ptr(receiver_place.layout.ty);
                let this_receiver_ptr = self.layout_of(receiver_ptr_ty)?.field(self, 0)?;
                // Adjust receiver argument.
                args[0] =
                    OpTy::from(ImmTy { layout: this_receiver_ptr, imm: receiver_place.ptr.into() });
                trace!("Patched self operand to {:#?}", args[0]);
                // recurse with concrete function
                self.eval_fn_call(drop_fn, span, caller_abi, &args, ret, unwind)
            }
        }
    }

    fn drop_in_place(
        &mut self,
        place: PlaceTy<'tcx, M::PointerTag>,
        instance: ty::Instance<'tcx>,
        span: Span,
        target: mir::BasicBlock,
        unwind: Option<mir::BasicBlock>,
    ) -> InterpResult<'tcx> {
        trace!("drop_in_place: {:?},\n  {:?}, {:?}", *place, place.layout.ty, instance);
        // We take the address of the object.  This may well be unaligned, which is fine
        // for us here.  However, unaligned accesses will probably make the actual drop
        // implementation fail -- a problem shared by rustc.
        let place = self.force_allocation(place)?;

        let (instance, place) = match place.layout.ty.kind {
            ty::Dynamic(..) => {
                // Dropping a trait object.
                self.unpack_dyn_trait(place)?
            }
            _ => (instance, place),
        };

        let arg = ImmTy {
            imm: place.to_ref(),
            layout: self.layout_of(self.tcx.mk_mut_ptr(place.layout.ty))?,
        };

        let ty = self.tcx.mk_unit(); // return type is ()
        let dest = MPlaceTy::dangling(self.layout_of(ty)?, self);

        self.eval_fn_call(
            FnVal::Instance(instance),
            span,
            Abi::Rust,
            &[arg.into()],
            Some((dest.into(), target)),
            unwind,
        )
    }
}
