//! A less precise version of `MaybeInitializedPlaces` whose domain is entire locals.
//!
//! A local will be maybe initialized if *any* projections of that local might be initialized.

use crate::dataflow::{self, BottomValue, GenKill};

use rustc_index::bit_set::BitSet;
use rustc_middle::mir::visit::{PlaceContext, Visitor};
use rustc_middle::mir::{self, BasicBlock, Local, Location};

pub struct MaybeInitializedLocals {
    deinit_on_move: bool,
}

impl MaybeInitializedLocals {
    /// Creates a new default `MaybeInitializedLocals` analysis.
    pub fn new() -> Self {
        Self { deinit_on_move: true }
    }

    /// Creates a new `MaybeInitializedLocals` analysis that does not consider a move from a local
    /// to deinitialize it.
    ///
    /// `StorageDead` still deinitializes locals.
    pub fn no_deinit_on_move() -> Self {
        Self { deinit_on_move: false }
    }
}

impl BottomValue for MaybeInitializedLocals {
    /// bottom = uninit
    const BOTTOM_VALUE: bool = false;
}

impl dataflow::AnalysisDomain<'tcx> for MaybeInitializedLocals {
    type Idx = Local;

    const NAME: &'static str = "maybe_init_locals";

    fn bits_per_block(&self, body: &mir::Body<'tcx>) -> usize {
        body.local_decls.len()
    }

    fn initialize_start_block(&self, body: &mir::Body<'tcx>, entry_set: &mut BitSet<Self::Idx>) {
        // Function arguments are initialized to begin with.
        for arg in body.args_iter() {
            entry_set.insert(arg);
        }
    }
}

impl dataflow::GenKillAnalysis<'tcx> for MaybeInitializedLocals {
    // The generator transform relies on the fact that this analysis does **not** use "before"
    // effects.

    fn statement_effect(
        &self,
        trans: &mut impl GenKill<Self::Idx>,
        statement: &mir::Statement<'tcx>,
        loc: Location,
    ) {
        TransferFunction { trans, deinit_on_move: self.deinit_on_move }
            .visit_statement(statement, loc)
    }

    fn terminator_effect(
        &self,
        trans: &mut impl GenKill<Self::Idx>,
        terminator: &mir::Terminator<'tcx>,
        loc: Location,
    ) {
        TransferFunction { trans, deinit_on_move: self.deinit_on_move }
            .visit_terminator(terminator, loc)
    }

    fn call_return_effect(
        &self,
        trans: &mut impl GenKill<Self::Idx>,
        _block: BasicBlock,
        _func: &mir::Operand<'tcx>,
        _args: &[mir::Operand<'tcx>],
        return_place: mir::Place<'tcx>,
    ) {
        trans.gen(return_place.local)
    }

    /// See `Analysis::apply_yield_resume_effect`.
    fn yield_resume_effect(
        &self,
        trans: &mut impl GenKill<Self::Idx>,
        _resume_block: BasicBlock,
        resume_place: mir::Place<'tcx>,
    ) {
        trans.gen(resume_place.local)
    }
}

struct TransferFunction<'a, T> {
    deinit_on_move: bool,
    trans: &'a mut T,
}

impl<T> Visitor<'tcx> for TransferFunction<'a, T>
where
    T: GenKill<Local>,
{
    fn visit_local(&mut self, &local: &Local, context: PlaceContext, _: Location) {
        use rustc_middle::mir::visit::{MutatingUseContext, NonMutatingUseContext, NonUseContext};
        match context {
            // These are handled specially in `call_return_effect` and `yield_resume_effect`.
            PlaceContext::MutatingUse(MutatingUseContext::Call | MutatingUseContext::Yield) => {}

            // Otherwise, when a place is mutated, we must consider it possibly initialized.
            PlaceContext::MutatingUse(_) => self.trans.gen(local),

            // If the local is moved out of, or if it gets marked `StorageDead`, consider it no
            // longer initialized.
            PlaceContext::NonUse(NonUseContext::StorageDead) => self.trans.kill(local),
            PlaceContext::NonMutatingUse(NonMutatingUseContext::Move) => {
                if self.deinit_on_move {
                    self.trans.kill(local)
                }
            }

            // All other uses do not affect this analysis.
            PlaceContext::NonUse(
                NonUseContext::StorageLive
                | NonUseContext::AscribeUserTy
                | NonUseContext::VarDebugInfo,
            )
            | PlaceContext::NonMutatingUse(
                NonMutatingUseContext::Inspect
                | NonMutatingUseContext::Copy
                | NonMutatingUseContext::SharedBorrow
                | NonMutatingUseContext::ShallowBorrow
                | NonMutatingUseContext::UniqueBorrow
                | NonMutatingUseContext::AddressOf
                | NonMutatingUseContext::Projection,
            ) => {}
        }
    }
}
