use std::cell::Cell;
use std::fmt;
use std::mem;

use rustc_data_structures::fx::FxHashMap;
use rustc_data_structures::stable_hasher::{HashStable, StableHasher};
use rustc_hir::{self as hir, def::DefKind, def_id::DefId, definitions::DefPathData};
use rustc_index::vec::IndexVec;
use rustc_macros::HashStable;
use rustc_middle::ich::StableHashingContext;
use rustc_middle::mir;
use rustc_middle::mir::interpret::{
    sign_extend, truncate, GlobalId, InterpResult, Pointer, Scalar,
};
use rustc_middle::ty::layout::{self, TyAndLayout};
use rustc_middle::ty::{
    self, query::TyCtxtAt, subst::SubstsRef, ParamEnv, Ty, TyCtxt, TypeFoldable,
};
use rustc_span::{Pos, Span};
use rustc_target::abi::{Align, HasDataLayout, LayoutOf, Size, TargetDataLayout};

use super::{
    Immediate, MPlaceTy, Machine, MemPlace, MemPlaceMeta, Memory, Operand, Place, PlaceTy,
    ScalarMaybeUninit, StackPopJump,
};
use crate::transform::validate::equal_up_to_regions;
use crate::util::storage::AlwaysLiveLocals;

pub struct InterpCx<'mir, 'tcx, M: Machine<'mir, 'tcx>> {
    /// Stores the `Machine` instance.
    ///
    /// Note: the stack is provided by the machine.
    pub machine: M,

    /// The results of the type checker, from rustc.
    /// The span in this is the "root" of the evaluation, i.e., the const
    /// we are evaluating (if this is CTFE).
    pub tcx: TyCtxtAt<'tcx>,

    /// Bounds in scope for polymorphic evaluations.
    pub(crate) param_env: ty::ParamEnv<'tcx>,

    /// The virtual memory system.
    pub memory: Memory<'mir, 'tcx, M>,

    /// A cache for deduplicating vtables
    pub(super) vtables:
        FxHashMap<(Ty<'tcx>, Option<ty::PolyExistentialTraitRef<'tcx>>), Pointer<M::PointerTag>>,
}

/// A stack frame.
#[derive(Clone)]
pub struct Frame<'mir, 'tcx, Tag = (), Extra = ()> {
    ////////////////////////////////////////////////////////////////////////////////
    // Function and callsite information
    ////////////////////////////////////////////////////////////////////////////////
    /// The MIR for the function called on this frame.
    pub body: &'mir mir::Body<'tcx>,

    /// The def_id and substs of the current function.
    pub instance: ty::Instance<'tcx>,

    /// Extra data for the machine.
    pub extra: Extra,

    ////////////////////////////////////////////////////////////////////////////////
    // Return place and locals
    ////////////////////////////////////////////////////////////////////////////////
    /// Work to perform when returning from this function.
    pub return_to_block: StackPopCleanup,

    /// The location where the result of the current stack frame should be written to,
    /// and its layout in the caller.
    pub return_place: Option<PlaceTy<'tcx, Tag>>,

    /// The list of locals for this stack frame, stored in order as
    /// `[return_ptr, arguments..., variables..., temporaries...]`.
    /// The locals are stored as `Option<Value>`s.
    /// `None` represents a local that is currently dead, while a live local
    /// can either directly contain `Scalar` or refer to some part of an `Allocation`.
    pub locals: IndexVec<mir::Local, LocalState<'tcx, Tag>>,

    ////////////////////////////////////////////////////////////////////////////////
    // Current position within the function
    ////////////////////////////////////////////////////////////////////////////////
    /// If this is `Err`, we are not currently executing any particular statement in
    /// this frame (can happen e.g. during frame initialization, and during unwinding on
    /// frames without cleanup code).
    /// We basically abuse `Result` as `Either`.
    pub(super) loc: Result<mir::Location, Span>,
}

/// What we store about a frame in an interpreter backtrace.
#[derive(Debug)]
pub struct FrameInfo<'tcx> {
    pub instance: ty::Instance<'tcx>,
    pub span: Span,
    pub lint_root: Option<hir::HirId>,
}

#[derive(Clone, Eq, PartialEq, Debug, HashStable)] // Miri debug-prints these
pub enum StackPopCleanup {
    /// Jump to the next block in the caller, or cause UB if None (that's a function
    /// that may never return). Also store layout of return place so
    /// we can validate it at that layout.
    /// `ret` stores the block we jump to on a normal return, while `unwind`
    /// stores the block used for cleanup during unwinding.
    Goto { ret: Option<mir::BasicBlock>, unwind: Option<mir::BasicBlock> },
    /// Just do nothing: Used by Main and for the `box_alloc` hook in miri.
    /// `cleanup` says whether locals are deallocated. Static computation
    /// wants them leaked to intern what they need (and just throw away
    /// the entire `ecx` when it is done).
    None { cleanup: bool },
}

/// State of a local variable including a memoized layout
#[derive(Clone, PartialEq, Eq, HashStable)]
pub struct LocalState<'tcx, Tag = ()> {
    pub value: LocalValue<Tag>,
    /// Don't modify if `Some`, this is only used to prevent computing the layout twice
    #[stable_hasher(ignore)]
    pub layout: Cell<Option<TyAndLayout<'tcx>>>,
}

/// Current value of a local variable
#[derive(Copy, Clone, PartialEq, Eq, Debug, HashStable)] // Miri debug-prints these
pub enum LocalValue<Tag = ()> {
    /// This local is not currently alive, and cannot be used at all.
    Dead,
    /// This local is alive but not yet initialized. It can be written to
    /// but not read from or its address taken. Locals get initialized on
    /// first write because for unsized locals, we do not know their size
    /// before that.
    Uninitialized,
    /// A normal, live local.
    /// Mostly for convenience, we re-use the `Operand` type here.
    /// This is an optimization over just always having a pointer here;
    /// we can thus avoid doing an allocation when the local just stores
    /// immediate values *and* never has its address taken.
    Live(Operand<Tag>),
}

impl<'tcx, Tag: Copy + 'static> LocalState<'tcx, Tag> {
    /// Read the local's value or error if the local is not yet live or not live anymore.
    ///
    /// Note: This may only be invoked from the `Machine::access_local` hook and not from
    /// anywhere else. You may be invalidating machine invariants if you do!
    pub fn access(&self) -> InterpResult<'tcx, Operand<Tag>> {
        match self.value {
            LocalValue::Dead => throw_ub!(DeadLocal),
            LocalValue::Uninitialized => {
                bug!("The type checker should prevent reading from a never-written local")
            }
            LocalValue::Live(val) => Ok(val),
        }
    }

    /// Overwrite the local.  If the local can be overwritten in place, return a reference
    /// to do so; otherwise return the `MemPlace` to consult instead.
    ///
    /// Note: This may only be invoked from the `Machine::access_local_mut` hook and not from
    /// anywhere else. You may be invalidating machine invariants if you do!
    pub fn access_mut(
        &mut self,
    ) -> InterpResult<'tcx, Result<&mut LocalValue<Tag>, MemPlace<Tag>>> {
        match self.value {
            LocalValue::Dead => throw_ub!(DeadLocal),
            LocalValue::Live(Operand::Indirect(mplace)) => Ok(Err(mplace)),
            ref mut
            local @ (LocalValue::Live(Operand::Immediate(_)) | LocalValue::Uninitialized) => {
                Ok(Ok(local))
            }
        }
    }
}

impl<'mir, 'tcx, Tag> Frame<'mir, 'tcx, Tag> {
    pub fn with_extra<Extra>(self, extra: Extra) -> Frame<'mir, 'tcx, Tag, Extra> {
        Frame {
            body: self.body,
            instance: self.instance,
            return_to_block: self.return_to_block,
            return_place: self.return_place,
            locals: self.locals,
            loc: self.loc,
            extra,
        }
    }
}

impl<'mir, 'tcx, Tag, Extra> Frame<'mir, 'tcx, Tag, Extra> {
    /// Return the `SourceInfo` of the current instruction.
    pub fn current_source_info(&self) -> Option<&mir::SourceInfo> {
        self.loc.ok().map(|loc| self.body.source_info(loc))
    }

    pub fn current_span(&self) -> Span {
        match self.loc {
            Ok(loc) => self.body.source_info(loc).span,
            Err(span) => span,
        }
    }
}

impl<'tcx> fmt::Display for FrameInfo<'tcx> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        ty::tls::with(|tcx| {
            if tcx.def_key(self.instance.def_id()).disambiguated_data.data
                == DefPathData::ClosureExpr
            {
                write!(f, "inside closure")?;
            } else {
                write!(f, "inside `{}`", self.instance)?;
            }
            if !self.span.is_dummy() {
                let lo = tcx.sess.source_map().lookup_char_pos(self.span.lo());
                write!(f, " at {}:{}:{}", lo.file.name, lo.line, lo.col.to_usize() + 1)?;
            }
            Ok(())
        })
    }
}

impl<'mir, 'tcx, M: Machine<'mir, 'tcx>> HasDataLayout for InterpCx<'mir, 'tcx, M> {
    #[inline]
    fn data_layout(&self) -> &TargetDataLayout {
        &self.tcx.data_layout
    }
}

impl<'mir, 'tcx, M> layout::HasTyCtxt<'tcx> for InterpCx<'mir, 'tcx, M>
where
    M: Machine<'mir, 'tcx>,
{
    #[inline]
    fn tcx(&self) -> TyCtxt<'tcx> {
        *self.tcx
    }
}

impl<'mir, 'tcx, M> layout::HasParamEnv<'tcx> for InterpCx<'mir, 'tcx, M>
where
    M: Machine<'mir, 'tcx>,
{
    fn param_env(&self) -> ty::ParamEnv<'tcx> {
        self.param_env
    }
}

impl<'mir, 'tcx: 'mir, M: Machine<'mir, 'tcx>> LayoutOf for InterpCx<'mir, 'tcx, M> {
    type Ty = Ty<'tcx>;
    type TyAndLayout = InterpResult<'tcx, TyAndLayout<'tcx>>;

    #[inline]
    fn layout_of(&self, ty: Ty<'tcx>) -> Self::TyAndLayout {
        self.tcx
            .layout_of(self.param_env.and(ty))
            .map_err(|layout| err_inval!(Layout(layout)).into())
    }
}

/// Test if it is valid for a MIR assignment to assign `src`-typed place to `dest`-typed value.
/// This test should be symmetric, as it is primarily about layout compatibility.
pub(super) fn mir_assign_valid_types<'tcx>(
    tcx: TyCtxt<'tcx>,
    param_env: ParamEnv<'tcx>,
    src: TyAndLayout<'tcx>,
    dest: TyAndLayout<'tcx>,
) -> bool {
    // Type-changing assignments can happen when subtyping is used. While
    // all normal lifetimes are erased, higher-ranked types with their
    // late-bound lifetimes are still around and can lead to type
    // differences. So we compare ignoring lifetimes.
    if equal_up_to_regions(tcx, param_env, src.ty, dest.ty) {
        // Make sure the layout is equal, too -- just to be safe. Miri really
        // needs layout equality. For performance reason we skip this check when
        // the types are equal. Equal types *can* have different layouts when
        // enum downcast is involved (as enum variants carry the type of the
        // enum), but those should never occur in assignments.
        if cfg!(debug_assertions) || src.ty != dest.ty {
            assert_eq!(src.layout, dest.layout);
        }
        true
    } else {
        false
    }
}

/// Use the already known layout if given (but sanity check in debug mode),
/// or compute the layout.
#[cfg_attr(not(debug_assertions), inline(always))]
pub(super) fn from_known_layout<'tcx>(
    tcx: TyCtxtAt<'tcx>,
    param_env: ParamEnv<'tcx>,
    known_layout: Option<TyAndLayout<'tcx>>,
    compute: impl FnOnce() -> InterpResult<'tcx, TyAndLayout<'tcx>>,
) -> InterpResult<'tcx, TyAndLayout<'tcx>> {
    match known_layout {
        None => compute(),
        Some(known_layout) => {
            if cfg!(debug_assertions) {
                let check_layout = compute()?;
                if !mir_assign_valid_types(tcx.tcx, param_env, check_layout, known_layout) {
                    span_bug!(
                        tcx.span,
                        "expected type differs from actual type.\nexpected: {:?}\nactual: {:?}",
                        known_layout.ty,
                        check_layout.ty,
                    );
                }
            }
            Ok(known_layout)
        }
    }
}

impl<'mir, 'tcx: 'mir, M: Machine<'mir, 'tcx>> InterpCx<'mir, 'tcx, M> {
    pub fn new(
        tcx: TyCtxt<'tcx>,
        root_span: Span,
        param_env: ty::ParamEnv<'tcx>,
        machine: M,
        memory_extra: M::MemoryExtra,
    ) -> Self {
        InterpCx {
            machine,
            tcx: tcx.at(root_span),
            param_env,
            memory: Memory::new(tcx, memory_extra),
            vtables: FxHashMap::default(),
        }
    }

    #[inline(always)]
    pub fn cur_span(&self) -> Span {
        self.stack().last().map(|f| f.current_span()).unwrap_or(self.tcx.span)
    }

    #[inline(always)]
    pub fn force_ptr(
        &self,
        scalar: Scalar<M::PointerTag>,
    ) -> InterpResult<'tcx, Pointer<M::PointerTag>> {
        self.memory.force_ptr(scalar)
    }

    #[inline(always)]
    pub fn force_bits(
        &self,
        scalar: Scalar<M::PointerTag>,
        size: Size,
    ) -> InterpResult<'tcx, u128> {
        self.memory.force_bits(scalar, size)
    }

    /// Call this to turn untagged "global" pointers (obtained via `tcx`) into
    /// the machine pointer to the allocation.  Must never be used
    /// for any other pointers, nor for TLS statics.
    ///
    /// Using the resulting pointer represents a *direct* access to that memory
    /// (e.g. by directly using a `static`),
    /// as opposed to access through a pointer that was created by the program.
    ///
    /// This function can fail only if `ptr` points to an `extern static`.
    #[inline(always)]
    pub fn global_base_pointer(&self, ptr: Pointer) -> InterpResult<'tcx, Pointer<M::PointerTag>> {
        self.memory.global_base_pointer(ptr)
    }

    #[inline(always)]
    pub(crate) fn stack(&self) -> &[Frame<'mir, 'tcx, M::PointerTag, M::FrameExtra>] {
        M::stack(self)
    }

    #[inline(always)]
    pub(crate) fn stack_mut(
        &mut self,
    ) -> &mut Vec<Frame<'mir, 'tcx, M::PointerTag, M::FrameExtra>> {
        M::stack_mut(self)
    }

    #[inline(always)]
    pub fn frame_idx(&self) -> usize {
        let stack = self.stack();
        assert!(!stack.is_empty());
        stack.len() - 1
    }

    #[inline(always)]
    pub fn frame(&self) -> &Frame<'mir, 'tcx, M::PointerTag, M::FrameExtra> {
        self.stack().last().expect("no call frames exist")
    }

    #[inline(always)]
    pub fn frame_mut(&mut self) -> &mut Frame<'mir, 'tcx, M::PointerTag, M::FrameExtra> {
        self.stack_mut().last_mut().expect("no call frames exist")
    }

    #[inline(always)]
    pub(super) fn body(&self) -> &'mir mir::Body<'tcx> {
        self.frame().body
    }

    #[inline(always)]
    pub fn sign_extend(&self, value: u128, ty: TyAndLayout<'_>) -> u128 {
        assert!(ty.abi.is_signed());
        sign_extend(value, ty.size)
    }

    #[inline(always)]
    pub fn truncate(&self, value: u128, ty: TyAndLayout<'_>) -> u128 {
        truncate(value, ty.size)
    }

    #[inline]
    pub fn type_is_sized(&self, ty: Ty<'tcx>) -> bool {
        ty.is_sized(self.tcx, self.param_env)
    }

    #[inline]
    pub fn type_is_freeze(&self, ty: Ty<'tcx>) -> bool {
        ty.is_freeze(self.tcx, self.param_env)
    }

    pub fn load_mir(
        &self,
        instance: ty::InstanceDef<'tcx>,
        promoted: Option<mir::Promoted>,
    ) -> InterpResult<'tcx, &'tcx mir::Body<'tcx>> {
        // do not continue if typeck errors occurred (can only occur in local crate)
        let def = instance.with_opt_param();
        if let Some(def) = def.as_local() {
            if self.tcx.has_typeck_results(def.did) {
                if let Some(error_reported) = self.tcx.typeck_opt_const_arg(def).tainted_by_errors {
                    throw_inval!(TypeckError(error_reported))
                }
            }
        }
        trace!("load mir(instance={:?}, promoted={:?})", instance, promoted);
        if let Some(promoted) = promoted {
            return Ok(&self.tcx.promoted_mir_of_opt_const_arg(def)[promoted]);
        }
        match instance {
            ty::InstanceDef::Item(def) => {
                if self.tcx.is_mir_available(def.did) {
                    if let Some((did, param_did)) = def.as_const_arg() {
                        Ok(self.tcx.optimized_mir_of_const_arg((did, param_did)))
                    } else {
                        Ok(self.tcx.optimized_mir(def.did))
                    }
                } else {
                    throw_unsup!(NoMirFor(def.did))
                }
            }
            _ => Ok(self.tcx.instance_mir(instance)),
        }
    }

    /// Call this on things you got out of the MIR (so it is as generic as the current
    /// stack frame), to bring it into the proper environment for this interpreter.
    pub(super) fn subst_from_current_frame_and_normalize_erasing_regions<T: TypeFoldable<'tcx>>(
        &self,
        value: T,
    ) -> T {
        self.subst_from_frame_and_normalize_erasing_regions(self.frame(), value)
    }

    /// Call this on things you got out of the MIR (so it is as generic as the provided
    /// stack frame), to bring it into the proper environment for this interpreter.
    pub(super) fn subst_from_frame_and_normalize_erasing_regions<T: TypeFoldable<'tcx>>(
        &self,
        frame: &Frame<'mir, 'tcx, M::PointerTag, M::FrameExtra>,
        value: T,
    ) -> T {
        if let Some(substs) = frame.instance.substs_for_mir_body() {
            self.tcx.subst_and_normalize_erasing_regions(substs, self.param_env, &value)
        } else {
            self.tcx.normalize_erasing_regions(self.param_env, value)
        }
    }

    /// The `substs` are assumed to already be in our interpreter "universe" (param_env).
    pub(super) fn resolve(
        &self,
        def_id: DefId,
        substs: SubstsRef<'tcx>,
    ) -> InterpResult<'tcx, ty::Instance<'tcx>> {
        trace!("resolve: {:?}, {:#?}", def_id, substs);
        trace!("param_env: {:#?}", self.param_env);
        trace!("substs: {:#?}", substs);
        match ty::Instance::resolve(*self.tcx, self.param_env, def_id, substs) {
            Ok(Some(instance)) => Ok(instance),
            Ok(None) => throw_inval!(TooGeneric),

            // FIXME(eddyb) this could be a bit more specific than `TypeckError`.
            Err(error_reported) => throw_inval!(TypeckError(error_reported)),
        }
    }

    pub fn layout_of_local(
        &self,
        frame: &Frame<'mir, 'tcx, M::PointerTag, M::FrameExtra>,
        local: mir::Local,
        layout: Option<TyAndLayout<'tcx>>,
    ) -> InterpResult<'tcx, TyAndLayout<'tcx>> {
        // `const_prop` runs into this with an invalid (empty) frame, so we
        // have to support that case (mostly by skipping all caching).
        match frame.locals.get(local).and_then(|state| state.layout.get()) {
            None => {
                let layout = from_known_layout(self.tcx, self.param_env, layout, || {
                    let local_ty = frame.body.local_decls[local].ty;
                    let local_ty =
                        self.subst_from_frame_and_normalize_erasing_regions(frame, local_ty);
                    self.layout_of(local_ty)
                })?;
                if let Some(state) = frame.locals.get(local) {
                    // Layouts of locals are requested a lot, so we cache them.
                    state.layout.set(Some(layout));
                }
                Ok(layout)
            }
            Some(layout) => Ok(layout),
        }
    }

    /// Returns the actual dynamic size and alignment of the place at the given type.
    /// Only the "meta" (metadata) part of the place matters.
    /// This can fail to provide an answer for extern types.
    pub(super) fn size_and_align_of(
        &self,
        metadata: MemPlaceMeta<M::PointerTag>,
        layout: TyAndLayout<'tcx>,
    ) -> InterpResult<'tcx, Option<(Size, Align)>> {
        if !layout.is_unsized() {
            return Ok(Some((layout.size, layout.align.abi)));
        }
        match layout.ty.kind() {
            ty::Adt(..) | ty::Tuple(..) => {
                // First get the size of all statically known fields.
                // Don't use type_of::sizing_type_of because that expects t to be sized,
                // and it also rounds up to alignment, which we want to avoid,
                // as the unsized field's alignment could be smaller.
                assert!(!layout.ty.is_simd());
                assert!(layout.fields.count() > 0);
                trace!("DST layout: {:?}", layout);

                let sized_size = layout.fields.offset(layout.fields.count() - 1);
                let sized_align = layout.align.abi;
                trace!(
                    "DST {} statically sized prefix size: {:?} align: {:?}",
                    layout.ty,
                    sized_size,
                    sized_align
                );

                // Recurse to get the size of the dynamically sized field (must be
                // the last field).  Can't have foreign types here, how would we
                // adjust alignment and size for them?
                let field = layout.field(self, layout.fields.count() - 1)?;
                let (unsized_size, unsized_align) = match self.size_and_align_of(metadata, field)? {
                    Some(size_and_align) => size_and_align,
                    None => {
                        // A field with extern type.  If this field is at offset 0, we behave
                        // like the underlying extern type.
                        // FIXME: Once we have made decisions for how to handle size and alignment
                        // of `extern type`, this should be adapted.  It is just a temporary hack
                        // to get some code to work that probably ought to work.
                        if sized_size == Size::ZERO {
                            return Ok(None);
                        } else {
                            span_bug!(
                                self.cur_span(),
                                "Fields cannot be extern types, unless they are at offset 0"
                            )
                        }
                    }
                };

                // FIXME (#26403, #27023): We should be adding padding
                // to `sized_size` (to accommodate the `unsized_align`
                // required of the unsized field that follows) before
                // summing it with `sized_size`. (Note that since #26403
                // is unfixed, we do not yet add the necessary padding
                // here. But this is where the add would go.)

                // Return the sum of sizes and max of aligns.
                let size = sized_size + unsized_size; // `Size` addition

                // Choose max of two known alignments (combined value must
                // be aligned according to more restrictive of the two).
                let align = sized_align.max(unsized_align);

                // Issue #27023: must add any necessary padding to `size`
                // (to make it a multiple of `align`) before returning it.
                let size = size.align_to(align);

                // Check if this brought us over the size limit.
                if size.bytes() >= self.tcx.data_layout.obj_size_bound() {
                    throw_ub!(InvalidMeta("total size is bigger than largest supported object"));
                }
                Ok(Some((size, align)))
            }
            ty::Dynamic(..) => {
                let vtable = metadata.unwrap_meta();
                // Read size and align from vtable (already checks size).
                Ok(Some(self.read_size_and_align_from_vtable(vtable)?))
            }

            ty::Slice(_) | ty::Str => {
                let len = metadata.unwrap_meta().to_machine_usize(self)?;
                let elem = layout.field(self, 0)?;

                // Make sure the slice is not too big.
                let size = elem.size.checked_mul(len, self).ok_or_else(|| {
                    err_ub!(InvalidMeta("slice is bigger than largest supported object"))
                })?;
                Ok(Some((size, elem.align.abi)))
            }

            ty::Foreign(_) => Ok(None),

            _ => span_bug!(self.cur_span(), "size_and_align_of::<{:?}> not supported", layout.ty),
        }
    }
    #[inline]
    pub fn size_and_align_of_mplace(
        &self,
        mplace: MPlaceTy<'tcx, M::PointerTag>,
    ) -> InterpResult<'tcx, Option<(Size, Align)>> {
        self.size_and_align_of(mplace.meta, mplace.layout)
    }

    pub fn push_stack_frame(
        &mut self,
        instance: ty::Instance<'tcx>,
        body: &'mir mir::Body<'tcx>,
        return_place: Option<PlaceTy<'tcx, M::PointerTag>>,
        return_to_block: StackPopCleanup,
    ) -> InterpResult<'tcx> {
        if !self.stack().is_empty() {
            info!("PAUSING({}) {}", self.frame_idx(), self.frame().instance);
        }
        ::log_settings::settings().indentation += 1;

        // first push a stack frame so we have access to the local substs
        let pre_frame = Frame {
            body,
            loc: Err(body.span), // Span used for errors caused during preamble.
            return_to_block,
            return_place,
            // empty local array, we fill it in below, after we are inside the stack frame and
            // all methods actually know about the frame
            locals: IndexVec::new(),
            instance,
            extra: (),
        };
        let frame = M::init_frame_extra(self, pre_frame)?;
        self.stack_mut().push(frame);

        // Make sure all the constants required by this frame evaluate successfully (post-monomorphization check).
        for const_ in &body.required_consts {
            let span = const_.span;
            let const_ =
                self.subst_from_current_frame_and_normalize_erasing_regions(const_.literal);
            self.const_to_op(const_, None).map_err(|err| {
                // If there was an error, set the span of the current frame to this constant.
                // Avoiding doing this when evaluation succeeds.
                self.frame_mut().loc = Err(span);
                err
            })?;
        }

        // Locals are initially uninitialized.
        let dummy = LocalState { value: LocalValue::Uninitialized, layout: Cell::new(None) };
        let mut locals = IndexVec::from_elem(dummy, &body.local_decls);

        // Now mark those locals as dead that we do not want to initialize
        match self.tcx.def_kind(instance.def_id()) {
            // statics and constants don't have `Storage*` statements, no need to look for them
            //
            // FIXME: The above is likely untrue. See
            // <https://github.com/rust-lang/rust/pull/70004#issuecomment-602022110>. Is it
            // okay to ignore `StorageDead`/`StorageLive` annotations during CTFE?
            DefKind::Static | DefKind::Const | DefKind::AssocConst => {}
            _ => {
                // Mark locals that use `Storage*` annotations as dead on function entry.
                let always_live = AlwaysLiveLocals::new(self.body());
                for local in locals.indices() {
                    if !always_live.contains(local) {
                        locals[local].value = LocalValue::Dead;
                    }
                }
            }
        }
        // done
        self.frame_mut().locals = locals;
        M::after_stack_push(self)?;
        self.frame_mut().loc = Ok(mir::Location::START);
        info!("ENTERING({}) {}", self.frame_idx(), self.frame().instance);

        Ok(())
    }

    /// Jump to the given block.
    #[inline]
    pub fn go_to_block(&mut self, target: mir::BasicBlock) {
        self.frame_mut().loc = Ok(mir::Location { block: target, statement_index: 0 });
    }

    /// *Return* to the given `target` basic block.
    /// Do *not* use for unwinding! Use `unwind_to_block` instead.
    ///
    /// If `target` is `None`, that indicates the function cannot return, so we raise UB.
    pub fn return_to_block(&mut self, target: Option<mir::BasicBlock>) -> InterpResult<'tcx> {
        if let Some(target) = target {
            self.go_to_block(target);
            Ok(())
        } else {
            throw_ub!(Unreachable)
        }
    }

    /// *Unwind* to the given `target` basic block.
    /// Do *not* use for returning! Use `return_to_block` instead.
    ///
    /// If `target` is `None`, that indicates the function does not need cleanup during
    /// unwinding, and we will just keep propagating that upwards.
    pub fn unwind_to_block(&mut self, target: Option<mir::BasicBlock>) {
        self.frame_mut().loc = match target {
            Some(block) => Ok(mir::Location { block, statement_index: 0 }),
            None => Err(self.frame_mut().body.span),
        };
    }

    /// Pops the current frame from the stack, deallocating the
    /// memory for allocated locals.
    ///
    /// If `unwinding` is `false`, then we are performing a normal return
    /// from a function. In this case, we jump back into the frame of the caller,
    /// and continue execution as normal.
    ///
    /// If `unwinding` is `true`, then we are in the middle of a panic,
    /// and need to unwind this frame. In this case, we jump to the
    /// `cleanup` block for the function, which is responsible for running
    /// `Drop` impls for any locals that have been initialized at this point.
    /// The cleanup block ends with a special `Resume` terminator, which will
    /// cause us to continue unwinding.
    pub(super) fn pop_stack_frame(&mut self, unwinding: bool) -> InterpResult<'tcx> {
        info!(
            "LEAVING({}) {} (unwinding = {})",
            self.frame_idx(),
            self.frame().instance,
            unwinding
        );

        // Sanity check `unwinding`.
        assert_eq!(
            unwinding,
            match self.frame().loc {
                Ok(loc) => self.body().basic_blocks()[loc.block].is_cleanup,
                Err(_) => true,
            }
        );

        if unwinding && self.frame_idx() == 0 {
            throw_ub_format!("unwinding past the topmost frame of the stack");
        }

        ::log_settings::settings().indentation -= 1;
        let frame =
            self.stack_mut().pop().expect("tried to pop a stack frame, but there were none");

        if !unwinding {
            // Copy the return value to the caller's stack frame.
            if let Some(return_place) = frame.return_place {
                let op = self.access_local(&frame, mir::RETURN_PLACE, None)?;
                self.copy_op_transmute(op, return_place)?;
                trace!("{:?}", self.dump_place(*return_place));
            } else {
                throw_ub!(Unreachable);
            }
        }

        // Now where do we jump next?

        // Usually we want to clean up (deallocate locals), but in a few rare cases we don't.
        // In that case, we return early. We also avoid validation in that case,
        // because this is CTFE and the final value will be thoroughly validated anyway.
        let (cleanup, next_block) = match frame.return_to_block {
            StackPopCleanup::Goto { ret, unwind } => {
                (true, Some(if unwinding { unwind } else { ret }))
            }
            StackPopCleanup::None { cleanup, .. } => (cleanup, None),
        };

        if !cleanup {
            assert!(self.stack().is_empty(), "only the topmost frame should ever be leaked");
            assert!(next_block.is_none(), "tried to skip cleanup when we have a next block!");
            assert!(!unwinding, "tried to skip cleanup during unwinding");
            // Leak the locals, skip validation, skip machine hook.
            return Ok(());
        }

        // Cleanup: deallocate all locals that are backed by an allocation.
        for local in &frame.locals {
            self.deallocate_local(local.value)?;
        }

        if M::after_stack_pop(self, frame, unwinding)? == StackPopJump::NoJump {
            // The hook already did everything.
            // We want to skip the `info!` below, hence early return.
            return Ok(());
        }
        // Normal return, figure out where to jump.
        if unwinding {
            // Follow the unwind edge.
            let unwind = next_block.expect("Encountered StackPopCleanup::None when unwinding!");
            self.unwind_to_block(unwind);
        } else {
            // Follow the normal return edge.
            if let Some(ret) = next_block {
                self.return_to_block(ret)?;
            }
        }

        if !self.stack().is_empty() {
            info!(
                "CONTINUING({}) {} (unwinding = {})",
                self.frame_idx(),
                self.frame().instance,
                unwinding
            );
        }

        Ok(())
    }

    /// Mark a storage as live, killing the previous content and returning it.
    /// Remember to deallocate that!
    pub fn storage_live(
        &mut self,
        local: mir::Local,
    ) -> InterpResult<'tcx, LocalValue<M::PointerTag>> {
        assert!(local != mir::RETURN_PLACE, "Cannot make return place live");
        trace!("{:?} is now live", local);

        let local_val = LocalValue::Uninitialized;
        // StorageLive *always* kills the value that's currently stored.
        // However, we do not error if the variable already is live;
        // see <https://github.com/rust-lang/rust/issues/42371>.
        Ok(mem::replace(&mut self.frame_mut().locals[local].value, local_val))
    }

    /// Returns the old value of the local.
    /// Remember to deallocate that!
    pub fn storage_dead(&mut self, local: mir::Local) -> LocalValue<M::PointerTag> {
        assert!(local != mir::RETURN_PLACE, "Cannot make return place dead");
        trace!("{:?} is now dead", local);

        mem::replace(&mut self.frame_mut().locals[local].value, LocalValue::Dead)
    }

    pub(super) fn deallocate_local(
        &mut self,
        local: LocalValue<M::PointerTag>,
    ) -> InterpResult<'tcx> {
        // FIXME: should we tell the user that there was a local which was never written to?
        if let LocalValue::Live(Operand::Indirect(MemPlace { ptr, .. })) = local {
            // All locals have a backing allocation, even if the allocation is empty
            // due to the local having ZST type.
            let ptr = ptr.assert_ptr();
            trace!("deallocating local: {:?}", self.memory.dump_alloc(ptr.alloc_id));
            self.memory.deallocate_local(ptr)?;
        };
        Ok(())
    }

    pub fn eval_to_allocation(
        &self,
        gid: GlobalId<'tcx>,
    ) -> InterpResult<'tcx, MPlaceTy<'tcx, M::PointerTag>> {
        // For statics we pick `ParamEnv::reveal_all`, because statics don't have generics
        // and thus don't care about the parameter environment. While we could just use
        // `self.param_env`, that would mean we invoke the query to evaluate the static
        // with different parameter environments, thus causing the static to be evaluated
        // multiple times.
        let param_env = if self.tcx.is_static(gid.instance.def_id()) {
            ty::ParamEnv::reveal_all()
        } else {
            self.param_env
        };
        let val = self.tcx.eval_to_allocation_raw(param_env.and(gid))?;
        self.raw_const_to_mplace(val)
    }

    #[must_use]
    pub fn dump_place(&'a self, place: Place<M::PointerTag>) -> PlacePrinter<'a, 'mir, 'tcx, M> {
        PlacePrinter { ecx: self, place }
    }

    #[must_use]
    pub fn generate_stacktrace(&self) -> Vec<FrameInfo<'tcx>> {
        let mut frames = Vec::new();
        for frame in self.stack().iter().rev() {
            let lint_root = frame.current_source_info().and_then(|source_info| {
                match &frame.body.source_scopes[source_info.scope].local_data {
                    mir::ClearCrossCrate::Set(data) => Some(data.lint_root),
                    mir::ClearCrossCrate::Clear => None,
                }
            });
            let span = frame.current_span();

            frames.push(FrameInfo { span, instance: frame.instance, lint_root });
        }
        trace!("generate stacktrace: {:#?}", frames);
        frames
    }
}

#[doc(hidden)]
/// Helper struct for the `dump_place` function.
pub struct PlacePrinter<'a, 'mir, 'tcx, M: Machine<'mir, 'tcx>> {
    ecx: &'a InterpCx<'mir, 'tcx, M>,
    place: Place<M::PointerTag>,
}

impl<'a, 'mir, 'tcx: 'mir, M: Machine<'mir, 'tcx>> std::fmt::Debug
    for PlacePrinter<'a, 'mir, 'tcx, M>
{
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.place {
            Place::Local { frame, local } => {
                let mut allocs = Vec::new();
                write!(fmt, "{:?}", local)?;
                if frame != self.ecx.frame_idx() {
                    write!(fmt, " ({} frames up)", self.ecx.frame_idx() - frame)?;
                }
                write!(fmt, ":")?;

                match self.ecx.stack()[frame].locals[local].value {
                    LocalValue::Dead => write!(fmt, " is dead")?,
                    LocalValue::Uninitialized => write!(fmt, " is uninitialized")?,
                    LocalValue::Live(Operand::Indirect(mplace)) => match mplace.ptr {
                        Scalar::Ptr(ptr) => {
                            write!(
                                fmt,
                                " by align({}){} ref:",
                                mplace.align.bytes(),
                                match mplace.meta {
                                    MemPlaceMeta::Meta(meta) => format!(" meta({:?})", meta),
                                    MemPlaceMeta::Poison | MemPlaceMeta::None => String::new(),
                                }
                            )?;
                            allocs.push(ptr.alloc_id);
                        }
                        ptr => write!(fmt, " by integral ref: {:?}", ptr)?,
                    },
                    LocalValue::Live(Operand::Immediate(Immediate::Scalar(val))) => {
                        write!(fmt, " {:?}", val)?;
                        if let ScalarMaybeUninit::Scalar(Scalar::Ptr(ptr)) = val {
                            allocs.push(ptr.alloc_id);
                        }
                    }
                    LocalValue::Live(Operand::Immediate(Immediate::ScalarPair(val1, val2))) => {
                        write!(fmt, " ({:?}, {:?})", val1, val2)?;
                        if let ScalarMaybeUninit::Scalar(Scalar::Ptr(ptr)) = val1 {
                            allocs.push(ptr.alloc_id);
                        }
                        if let ScalarMaybeUninit::Scalar(Scalar::Ptr(ptr)) = val2 {
                            allocs.push(ptr.alloc_id);
                        }
                    }
                }

                write!(fmt, ": {:?}", self.ecx.memory.dump_allocs(allocs))
            }
            Place::Ptr(mplace) => match mplace.ptr {
                Scalar::Ptr(ptr) => write!(
                    fmt,
                    "by align({}) ref: {:?}",
                    mplace.align.bytes(),
                    self.ecx.memory.dump_alloc(ptr.alloc_id)
                ),
                ptr => write!(fmt, " integral by ref: {:?}", ptr),
            },
        }
    }
}

impl<'ctx, 'mir, 'tcx, Tag, Extra> HashStable<StableHashingContext<'ctx>>
    for Frame<'mir, 'tcx, Tag, Extra>
where
    Extra: HashStable<StableHashingContext<'ctx>>,
    Tag: HashStable<StableHashingContext<'ctx>>,
{
    fn hash_stable(&self, hcx: &mut StableHashingContext<'ctx>, hasher: &mut StableHasher) {
        // Exhaustive match on fields to make sure we forget no field.
        let Frame { body, instance, return_to_block, return_place, locals, loc, extra } = self;
        body.hash_stable(hcx, hasher);
        instance.hash_stable(hcx, hasher);
        return_to_block.hash_stable(hcx, hasher);
        return_place.as_ref().map(|r| &**r).hash_stable(hcx, hasher);
        locals.hash_stable(hcx, hasher);
        loc.hash_stable(hcx, hasher);
        extra.hash_stable(hcx, hasher);
    }
}
