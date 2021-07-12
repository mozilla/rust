//! A intra-block copy propagation pass.
//!
//! Given an assignment `_a = _b` replaces subsequent uses of destination `_a` with source `_b`, as
//! long as neither `a` nor `_b` had been modified in the intervening statements.
//!
//! The implementation processes block statements & terminator in the execution order. For each
//! local it keeps track of a source that defined its current value. When it encounters a copy use
//! of a local, it verifies that source had not been modified since the assignment and replaces the
//! local with the source.
//!
//! To detect modifications, each local has a generation number that is increased after each direct
//! modification. The local generation number is recorded at the time of the assignment and
//! verified before the propagation to ensure that the local remains unchanged since the
//! assignment.
//!
//! Instead of detecting indirect modifications, locals that have their address taken never
//! participate in copy propagation.
//!
//! When moving in-between the blocks, all recorded values are invalidated. To do that in O(1)
//! time, generation numbers have a global component that is increased after each block.

use crate::transform::MirPass;
use crate::util::ever_borrowed_locals;
use rustc_index::bit_set::BitSet;
use rustc_index::vec::IndexVec;
use rustc_middle::mir::visit::*;
use rustc_middle::mir::*;
use rustc_middle::ty::{List, TyCtxt};

pub struct CopyPropagation;

impl<'tcx> MirPass<'tcx> for CopyPropagation {
    fn run_pass(&self, tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
        copy_move_operands_in_statements(tcx, body);
        propagate_copies(tcx, body);
    }
}

fn propagate_copies(tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
    let mut values = LocalValues {
        tcx,
        borrowed_locals: ever_borrowed_locals(body),
        values: IndexVec::from_elem_n(LocalValue::default(), body.local_decls.len()),
        block_generation: 0,
        // Copying from places with projections tends to generate more LLVM IR.
        // Disable it by default.
        copy_projections: tcx.sess.mir_opt_level() > 2,
    };
    for (block, data) in body.basic_blocks_mut().iter_enumerated_mut() {
        for (statement_index, statement) in data.statements.iter_mut().enumerate() {
            let location = Location { block, statement_index };
            InvalidateModifiedLocals { values: &mut values }.visit_statement(statement, location);
            CopyPropagate { tcx, values: &mut values }.visit_statement(statement, location);
            values.record_assignment(statement);
        }

        let location = Location { block, statement_index: data.statements.len() };
        InvalidateModifiedLocals { values: &mut values }
            .visit_terminator(data.terminator_mut(), location);
        CopyPropagate { tcx, values: &mut values }
            .visit_terminator(data.terminator_mut(), location);
        values.invalidate_all();
    }
}

struct LocalValues<'tcx> {
    tcx: TyCtxt<'tcx>,
    /// Locals that have their address taken. They do not participate in copy propagation.
    borrowed_locals: BitSet<Local>,
    /// A symbolic value of each local.
    values: IndexVec<Local, LocalValue<'tcx>>,
    /// Block generation number. Used to invalidate locals' values in-between the blocks in O(1) time.
    block_generation: u32,
    /// Enables copying from places with projections.
    copy_projections: bool,
}

/// A symbolic value of a local variable.
#[derive(Copy, Clone, Default)]
struct LocalValue<'tcx> {
    /// Generation of the current value.
    generation: Generation,
    /// Generation of the source value at the time of the assignment.
    src_generation: Generation,
    /// If present the current value of this local is a result of assignment `this = src`.
    src: Option<Place<'tcx>>,
}

#[derive(Copy, Clone, Default, PartialEq, Eq)]
struct Generation {
    /// Local generation number. Increased after each mutation.
    local: u32,
    /// Block generation number. Increased in-between the blocks.
    block: u32,
}

impl<'tcx> LocalValues<'tcx> {
    /// Invalidates all locals' values.
    fn invalidate_all(&mut self) {
        assert!(self.block_generation != u32::MAX);
        self.block_generation += 1;
    }

    /// Invalidates the local's value.
    fn invalidate_local(&mut self, local: Local) {
        let value = &mut self.values[local];
        assert!(value.generation.local != u32::MAX);
        value.generation.local += 1;
        value.src_generation = Generation::default();
        value.src = None;
    }

    fn record_assignment(&mut self, statement: &Statement<'tcx>) {
        let (place, rvalue) = match statement.kind {
            StatementKind::Assign(box (ref place, ref rvalue)) => (place, rvalue),
            _ => return,
        };

        // Record only complete definitions of local variables.
        let dst = match place.as_local() {
            Some(dst) => dst,
            None => return,
        };
        // Reject borrowed destinations.
        if self.borrowed_locals.contains(dst) {
            return;
        }

        let src = match rvalue {
            Rvalue::Use(Operand::Copy(src)) => src,
            _ => return,
        };
        if !self.copy_projections && !src.projection.is_empty() {
            return;
        }
        // Don't propagate sources that require additional tracking:
        // * a deref would require tracking of a pointee
        // * an index would require tracking the index
        for elem in src.projection {
            match elem {
                ProjectionElem::Deref | ProjectionElem::Index(_) => return,

                ProjectionElem::Field(_, _)
                | ProjectionElem::ConstantIndex { .. }
                | ProjectionElem::Subslice { .. }
                | ProjectionElem::Downcast(_, _) => {}
            }
        }
        // Reject borrowed sources.
        if self.borrowed_locals.contains(src.local) {
            return;
        }

        // Record `dst = src` assignment.
        let src_generation = self.values[src.local].generation;
        let value = &mut self.values[dst];
        value.generation.local += 1;
        value.generation.block = self.block_generation;
        value.src = Some(*src);
        value.src_generation = src_generation;
    }

    /// Replaces a use of dst with its current value.
    fn propagate_local(&mut self, dst: &mut Local) {
        let dst_value = &self.values[*dst];

        let src = match dst_value.src {
            Some(src) => src,
            None => return,
        };
        let src = match src.as_local() {
            Some(src) => src,
            None => return,
        };
        // Last definition of dst was of the form `dst = src`.

        // Check that dst was defined in this block.
        if dst_value.generation.block != self.block_generation {
            return;
        }
        // Check that src still has the same value.
        if dst_value.src_generation != self.values[src].generation {
            return;
        }

        // Propagate
        *dst = src;
    }

    /// Replaces a use of dst with its current value.
    fn propagate_place(&mut self, dst: &mut Place<'tcx>) {
        let dst_value = &self.values[dst.local];

        let src = match dst_value.src {
            Some(src) => src,
            None => return,
        };
        // Last definition of dst.local was of the form `dst.local = src`.

        // Check that dst.local was defined in this block.
        if dst_value.generation.block != self.block_generation {
            return;
        }
        // Check that src still has the same value.
        if dst_value.src_generation != self.values[src.local].generation {
            return;
        }

        // Propagate while rebasing any additional projections on top of src.
        *dst = Place {
            local: src.local,
            projection: rebase_onto(self.tcx, dst.projection, src.projection),
        };
    }
}

/// Rebase projection `proj` on top of a `base`.
fn rebase_onto<'tcx>(
    tcx: TyCtxt<'tcx>,
    proj: &'tcx List<PlaceElem<'tcx>>,
    base: &'tcx List<PlaceElem<'tcx>>,
) -> &'tcx List<PlaceElem<'tcx>> {
    if base.is_empty() {
        return proj;
    }
    if proj.is_empty() {
        return base;
    }
    let mut projection: Vec<_> = Vec::with_capacity(base.len() + proj.len());
    projection.extend(base);
    projection.extend(proj);
    tcx.intern_place_elems(&projection)
}

/// Invalidates locals that could be modified during execution of visited MIR.
struct InvalidateModifiedLocals<'tcx, 'a> {
    values: &'a mut LocalValues<'tcx>,
}

impl<'tcx, 'a> Visitor<'tcx> for InvalidateModifiedLocals<'tcx, 'a> {
    fn visit_local(&mut self, local: &Local, context: PlaceContext, _location: Location) {
        match context {
            PlaceContext::NonUse(_)
            | PlaceContext::MutatingUse(_)
            | PlaceContext::NonMutatingUse(NonMutatingUseContext::Move) => {
                self.values.invalidate_local(*local)
            }
            PlaceContext::NonMutatingUse(_) => {}
        }
    }
}

/// Replaces copy uses of locals with their current value.
struct CopyPropagate<'tcx, 'a> {
    tcx: TyCtxt<'tcx>,
    values: &'a mut LocalValues<'tcx>,
}

impl<'tcx, 'a> MutVisitor<'tcx> for CopyPropagate<'tcx, 'a> {
    fn tcx(&self) -> TyCtxt<'tcx> {
        self.tcx
    }

    fn visit_place(&mut self, place: &mut Place<'tcx>, context: PlaceContext, location: Location) {
        if let PlaceContext::NonMutatingUse(NonMutatingUseContext::Copy) = context {
            self.values.propagate_place(place);
        } else {
            self.super_place(place, context, location);
        }
    }

    fn visit_local(&mut self, local: &mut Local, context: PlaceContext, _location: Location) {
        if let PlaceContext::NonMutatingUse(NonMutatingUseContext::Copy) = context {
            self.values.propagate_local(local);
        }
    }
}

/// Transforms move operands into copy operands inside statements.
///
/// When a move operand is used in a call terminator and ABI passes value by a reference, the code
/// generation uses provided operand in place instead of making a copy.
///
/// To avoid introducing extra copies, we retain move operands in call terminators.
fn copy_move_operands_in_statements<'tcx>(tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
    let mut visitor = CopyMoveOperands { tcx };
    for (block, data) in body.basic_blocks_mut().iter_enumerated_mut() {
        visitor.visit_basic_block_data(block, data);
    }
}

struct CopyMoveOperands<'tcx> {
    tcx: TyCtxt<'tcx>,
}

impl<'tcx> MutVisitor<'tcx> for CopyMoveOperands<'tcx> {
    fn tcx(&self) -> TyCtxt<'tcx> {
        self.tcx
    }

    fn visit_operand(&mut self, operand: &mut Operand<'tcx>, _location: Location) {
        if let Operand::Move(place) = operand {
            *operand = Operand::Copy(*place);
        }
    }

    fn visit_terminator(&mut self, _terminator: &mut Terminator<'tcx>, _location: Location) {}
}
