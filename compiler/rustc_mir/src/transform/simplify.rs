//! A number of passes which remove various redundancies in the CFG.
//!
//! The `SimplifyCfg` pass gets rid of unnecessary blocks in the CFG, whereas the `SimplifyLocals`
//! gets rid of all the unnecessary local variable declarations.
//!
//! The `SimplifyLocals` pass is kinda expensive and therefore not very suitable to be run often.
//! Most of the passes should not care or be impacted in meaningful ways due to extra locals
//! either, so running the pass once, right before codegen, should suffice.
//!
//! On the other side of the spectrum, the `SimplifyCfg` pass is considerably cheap to run, thus
//! one should run it after every pass which may modify CFG in significant ways. This pass must
//! also be run before any analysis passes because it removes dead blocks, and some of these can be
//! ill-typed.
//!
//! The cause of this typing issue is typeck allowing most blocks whose end is not reachable have
//! an arbitrary return type, rather than having the usual () return type (as a note, typeck's
//! notion of reachability is in fact slightly weaker than MIR CFG reachability - see #31617). A
//! standard example of the situation is:
//!
//! ```rust
//!   fn example() {
//!       let _a: char = { return; };
//!   }
//! ```
//!
//! Here the block (`{ return; }`) has the return type `char`, rather than `()`, but the MIR we
//! naively generate still contains the `_a = ()` write in the unreachable block "after" the
//! return.

use crate::transform::MirPass;
use rustc_index::bit_set::BitSet;
use rustc_index::vec::{Idx, IndexVec};
use rustc_middle::mir::visit::{MutVisitor, MutatingUseContext, PlaceContext, Visitor};
use rustc_middle::mir::*;
use rustc_middle::ty::TyCtxt;
use rustc_session::config::OptLevel;
use smallvec::SmallVec;
use std::borrow::Cow;
use std::convert::TryInto;

pub struct SimplifyCfg {
    label: String,
}

impl SimplifyCfg {
    pub fn new(label: &str) -> Self {
        SimplifyCfg { label: format!("SimplifyCfg-{}", label) }
    }
}

pub fn simplify_cfg(body: &mut Body<'_>) {
    CfgSimplifier::new(body).simplify();
    remove_dead_blocks(body);

    // FIXME: Should probably be moved into some kind of pass manager
    body.basic_blocks_mut().raw.shrink_to_fit();
}

impl<'tcx> MirPass<'tcx> for SimplifyCfg {
    fn name(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.label)
    }

    fn run_pass(&self, _tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
        debug!("SimplifyCfg({:?}) - simplifying {:?}", self.label, body);
        simplify_cfg(body);
    }
}

pub struct CfgSimplifier<'a, 'tcx> {
    basic_blocks: &'a mut IndexVec<BasicBlock, BasicBlockData<'tcx>>,
    pred_count: IndexVec<BasicBlock, u32>,
}

impl<'a, 'tcx> CfgSimplifier<'a, 'tcx> {
    pub fn new(body: &'a mut Body<'tcx>) -> Self {
        let mut pred_count = IndexVec::from_elem(0u32, body.basic_blocks());

        // we can't use mir.predecessors() here because that counts
        // dead blocks, which we don't want to.
        pred_count[START_BLOCK] = 1;

        for (_, data) in traversal::preorder(body) {
            if let Some(ref term) = data.terminator {
                for &tgt in term.successors() {
                    pred_count[tgt] += 1;
                }
            }
        }

        let basic_blocks = body.basic_blocks_mut();

        CfgSimplifier { basic_blocks, pred_count }
    }

    pub fn simplify(mut self) {
        self.strip_nops();

        let mut start = START_BLOCK;

        // Vec of the blocks that should be merged. We store the indices here, instead of the
        // statements itself to avoid moving the (relatively) large statements twice.
        // We do not push the statements directly into the target block (`bb`) as that is slower
        // due to additional reallocations
        let mut merged_blocks = Vec::new();
        loop {
            let mut changed = false;

            self.collapse_goto_chain(&mut start, &mut changed);

            for bb in self.basic_blocks.indices() {
                if self.pred_count[bb] == 0 {
                    continue;
                }

                debug!("simplifying {:?}", bb);

                let mut terminator =
                    self.basic_blocks[bb].terminator.take().expect("invalid terminator state");

                for successor in terminator.successors_mut() {
                    self.collapse_goto_chain(successor, &mut changed);
                }

                let mut inner_changed = true;
                merged_blocks.clear();
                while inner_changed {
                    inner_changed = false;
                    inner_changed |= self.simplify_branch(&mut terminator);
                    inner_changed |= self.merge_successor(&mut merged_blocks, &mut terminator);
                    changed |= inner_changed;
                }

                let statements_to_merge =
                    merged_blocks.iter().map(|&i| self.basic_blocks[i].statements.len()).sum();

                if statements_to_merge > 0 {
                    let mut statements = std::mem::take(&mut self.basic_blocks[bb].statements);
                    statements.reserve(statements_to_merge);
                    for &from in &merged_blocks {
                        statements.append(&mut self.basic_blocks[from].statements);
                    }
                    self.basic_blocks[bb].statements = statements;
                }

                self.basic_blocks[bb].terminator = Some(terminator);
            }

            if !changed {
                break;
            }
        }

        if start != START_BLOCK {
            debug_assert!(self.pred_count[START_BLOCK] == 0);
            self.basic_blocks.swap(START_BLOCK, start);
            self.pred_count.swap(START_BLOCK, start);

            // pred_count == 1 if the start block has no predecessor _blocks_.
            if self.pred_count[START_BLOCK] > 1 {
                for (bb, data) in self.basic_blocks.iter_enumerated_mut() {
                    if self.pred_count[bb] == 0 {
                        continue;
                    }

                    for target in data.terminator_mut().successors_mut() {
                        if *target == start {
                            *target = START_BLOCK;
                        }
                    }
                }
            }
        }
    }

    /// This function will return `None` if
    /// * the block has statements
    /// * the block has a terminator other than `goto`
    /// * the block has no terminator (meaning some other part of the current optimization stole it)
    fn take_terminator_if_simple_goto(&mut self, bb: BasicBlock) -> Option<Terminator<'tcx>> {
        match self.basic_blocks[bb] {
            BasicBlockData {
                ref statements,
                terminator:
                    ref mut terminator @ Some(Terminator { kind: TerminatorKind::Goto { .. }, .. }),
                ..
            } if statements.is_empty() => terminator.take(),
            // if `terminator` is None, this means we are in a loop. In that
            // case, let all the loop collapse to its entry.
            _ => None,
        }
    }

    /// Collapse a goto chain starting from `start`
    fn collapse_goto_chain(&mut self, start: &mut BasicBlock, changed: &mut bool) {
        // Using `SmallVec` here, because in some logs on libcore oli-obk saw many single-element
        // goto chains. We should probably benchmark different sizes.
        let mut terminators: SmallVec<[_; 1]> = Default::default();
        let mut current = *start;
        while let Some(terminator) = self.take_terminator_if_simple_goto(current) {
            let target = match terminator {
                Terminator { kind: TerminatorKind::Goto { target }, .. } => target,
                _ => unreachable!(),
            };
            terminators.push((current, terminator));
            current = target;
        }
        let last = current;
        *start = last;
        while let Some((current, mut terminator)) = terminators.pop() {
            let target = match terminator {
                Terminator { kind: TerminatorKind::Goto { ref mut target }, .. } => target,
                _ => unreachable!(),
            };
            *changed |= *target != last;
            *target = last;
            debug!("collapsing goto chain from {:?} to {:?}", current, target);

            if self.pred_count[current] == 1 {
                // This is the last reference to current, so the pred-count to
                // to target is moved into the current block.
                self.pred_count[current] = 0;
            } else {
                self.pred_count[*target] += 1;
                self.pred_count[current] -= 1;
            }
            self.basic_blocks[current].terminator = Some(terminator);
        }
    }

    // merge a block with 1 `goto` predecessor to its parent
    fn merge_successor(
        &mut self,
        merged_blocks: &mut Vec<BasicBlock>,
        terminator: &mut Terminator<'tcx>,
    ) -> bool {
        let target = match terminator.kind {
            TerminatorKind::Goto { target } if self.pred_count[target] == 1 => target,
            _ => return false,
        };

        debug!("merging block {:?} into {:?}", target, terminator);
        *terminator = match self.basic_blocks[target].terminator.take() {
            Some(terminator) => terminator,
            None => {
                // unreachable loop - this should not be possible, as we
                // don't strand blocks, but handle it correctly.
                return false;
            }
        };

        merged_blocks.push(target);
        self.pred_count[target] = 0;

        true
    }

    // turn a branch with all successors identical to a goto
    fn simplify_branch(&mut self, terminator: &mut Terminator<'tcx>) -> bool {
        match terminator.kind {
            TerminatorKind::SwitchInt { .. } => {}
            _ => return false,
        };

        let first_succ = {
            if let Some(&first_succ) = terminator.successors().next() {
                if terminator.successors().all(|s| *s == first_succ) {
                    let count = terminator.successors().count();
                    self.pred_count[first_succ] -= (count - 1) as u32;
                    first_succ
                } else {
                    return false;
                }
            } else {
                return false;
            }
        };

        debug!("simplifying branch {:?}", terminator);
        terminator.kind = TerminatorKind::Goto { target: first_succ };
        true
    }

    fn strip_nops(&mut self) {
        for blk in self.basic_blocks.iter_mut() {
            blk.statements.retain(|stmt| !matches!(stmt.kind, StatementKind::Nop))
        }
    }
}

pub fn remove_dead_blocks(body: &mut Body<'_>) {
    let mut seen = BitSet::new_empty(body.basic_blocks().len());
    for (bb, _) in traversal::preorder(body) {
        seen.insert(bb.index());
    }

    let basic_blocks = body.basic_blocks_mut();

    let num_blocks = basic_blocks.len();
    let mut replacements: Vec<_> = (0..num_blocks).map(BasicBlock::new).collect();
    let mut used_blocks = 0;
    for alive_index in seen.iter() {
        replacements[alive_index] = BasicBlock::new(used_blocks);
        if alive_index != used_blocks {
            // Swap the next alive block data with the current available slot. Since
            // alive_index is non-decreasing this is a valid operation.
            basic_blocks.raw.swap(alive_index, used_blocks);
        }
        used_blocks += 1;
    }
    basic_blocks.raw.truncate(used_blocks);

    for block in basic_blocks {
        for target in block.terminator_mut().successors_mut() {
            *target = replacements[target.index()];
        }
    }
}

pub struct SimplifyLocals;

impl<'tcx> MirPass<'tcx> for SimplifyLocals {
    fn run_pass(&self, tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
        trace!("running SimplifyLocals on {:?}", body.source);

        // First, we're going to get a count of *actual* uses for every `Local`.
        let mut used_locals = UsedLocals::new(tcx, body);

        // Next, we're going to remove any `Local` with zero actual uses. When we remove those
        // `Locals`, we're also going to subtract any uses of other `Locals` from the `used_locals`
        // count. For example, if we removed `_2 = discriminant(_1)`, then we'll subtract one from
        // `use_counts[_1]`. That in turn might make `_1` unused, so we loop until we hit a
        // fixedpoint where there are no more unused locals.
        remove_unused_definitions(&mut used_locals, body);

        // Finally, we'll actually do the work of shrinking `body.local_decls` and remapping the `Local`s.
        let map = make_local_map(&mut body.local_decls, &used_locals);

        // Only bother running the `LocalUpdater` if we actually found locals to remove.
        if map.iter().any(Option::is_none) {
            // Update references to all vars and tmps now
            let mut updater = LocalUpdater { map, tcx };
            updater.visit_body(body);

            body.local_decls.shrink_to_fit();
        }
    }
}

/// Construct the mapping while swapping out unused stuff out from the `vec`.
fn make_local_map<V>(
    local_decls: &mut IndexVec<Local, V>,
    used_locals: &UsedLocals,
) -> IndexVec<Local, Option<Local>> {
    let mut map: IndexVec<Local, Option<Local>> = IndexVec::from_elem(None, &*local_decls);
    let mut used = Local::new(0);

    for alive_index in local_decls.indices() {
        // `is_used` treats the `RETURN_PLACE` and arguments as used.
        if !used_locals.is_used(alive_index) {
            continue;
        }

        map[alive_index] = Some(used);
        if alive_index != used {
            local_decls.swap(alive_index, used);
        }
        used.increment_by(1);
    }
    local_decls.truncate(used.index());
    map
}

/// Keeps track of used & unused locals.
struct UsedLocals {
    increment: bool,
    arg_count: u32,
    use_count: IndexVec<Local, u32>,
    /// When true var debug info is always preserved. When false the var debug
    /// info that refers to otherwise unused local variables will be removed.
    preserve_var_debug_info: bool,
}

impl UsedLocals {
    /// Determines which locals are used & unused in the given body.
    fn new(tcx: TyCtxt<'_>, body: &Body<'_>) -> Self {
        let mut this = Self {
            increment: true,
            arg_count: body.arg_count.try_into().unwrap(),
            use_count: IndexVec::from_elem(0, &body.local_decls),
            preserve_var_debug_info: tcx.sess.opts.optimize != OptLevel::Aggressive,
        };
        this.visit_body(body);
        this
    }

    /// Checks if local is used.
    ///
    /// Return place and arguments are always considered used.
    fn is_used(&self, local: Local) -> bool {
        trace!("is_used({:?}): use_count: {:?}", local, self.use_count[local]);
        local.as_u32() <= self.arg_count || self.use_count[local] != 0
    }

    /// Removes the statement when it is unused or returns an error otherwise.
    /// Updates the use counts to reflect the removal of given statement.
    fn try_remove_statement(&mut self, statement: &Statement<'_>) -> Result<(), ()> {
        if self.is_statement_used(statement) {
            return Err(());
        }

        // The location of the statement is irrelevant.
        let location = Location { block: START_BLOCK, statement_index: 0 };
        self.increment = false;
        self.visit_statement(statement, location);
        Ok(())
    }

    fn is_statement_used(&self, statement: &Statement<'_>) -> bool {
        match statement.kind {
            StatementKind::StorageLive(local) | StatementKind::StorageDead(local) => {
                self.is_used(local)
            }
            StatementKind::Assign(box (place, _)) => self.is_used(place.local),
            StatementKind::SetDiscriminant { ref place, .. } => self.is_used(place.local),
            _ => true,
        }
    }

    /// Removes the var debug info when it refers to any locals that are unused
    /// or returns an error otherwise.
    fn try_remove_var_debug_info(&mut self, var_debug_info: &VarDebugInfo<'_>) -> Result<(), ()> {
        if self.is_var_debug_info_valid(var_debug_info) {
            return Err(());
        }

        self.increment = false;
        self.visit_var_debug_info(var_debug_info);
        Ok(())
    }

    /// Determines if a var debug info is valid, i.e., it refers only to locals
    /// that are used.
    fn is_var_debug_info_valid(&self, var_debug_info: &VarDebugInfo<'_>) -> bool {
        let place = match var_debug_info.value {
            VarDebugInfoContents::Const(_) => return true,
            VarDebugInfoContents::Place(place) => place,
        };
        if !self.is_used(place.local) {
            return false;
        }
        for elem in place.projection {
            match elem {
                PlaceElem::Index(local) => {
                    if !self.is_used(local) {
                        return false;
                    }
                }
                PlaceElem::Deref
                | PlaceElem::Field(..)
                | PlaceElem::ConstantIndex { .. }
                | PlaceElem::Subslice { .. }
                | PlaceElem::Downcast(..) => {}
            }
        }

        true
    }

    /// Visits a left-hand side of an assignment.
    fn visit_lhs(&mut self, place: &Place<'tcx>, location: Location) {
        if place.is_indirect() {
            // A use, not a definition.
            self.visit_place(place, PlaceContext::MutatingUse(MutatingUseContext::Store), location);
        } else {
            // A definition. Although, it still might use other locals for indexing.
            self.super_projection(
                place.local,
                &place.projection,
                PlaceContext::MutatingUse(MutatingUseContext::Projection),
                location,
            );
        }
    }
}

impl Visitor<'_> for UsedLocals {
    fn visit_var_debug_info(&mut self, var_debug_info: &VarDebugInfo<'tcx>) {
        if self.preserve_var_debug_info {
            self.super_var_debug_info(var_debug_info);
        }
    }

    fn visit_statement(&mut self, statement: &Statement<'tcx>, location: Location) {
        match statement.kind {
            StatementKind::LlvmInlineAsm(..)
            | StatementKind::Retag(..)
            | StatementKind::Coverage(..)
            | StatementKind::FakeRead(..)
            | StatementKind::AscribeUserType(..) => {
                self.super_statement(statement, location);
            }

            StatementKind::Nop => {}

            StatementKind::StorageLive(_local) | StatementKind::StorageDead(_local) => {}

            StatementKind::Assign(box (ref place, ref rvalue)) => {
                self.visit_lhs(place, location);
                self.visit_rvalue(rvalue, location);
            }

            StatementKind::SetDiscriminant { ref place, variant_index: _ } => {
                self.visit_lhs(place, location);
            }
        }
    }

    fn visit_local(&mut self, local: &Local, _ctx: PlaceContext, _location: Location) {
        if self.increment {
            self.use_count[*local] += 1;
        } else {
            assert_ne!(self.use_count[*local], 0);
            self.use_count[*local] -= 1;
        }
    }
}

/// Removes unused definitions. Updates the used locals to reflect the changes made.
fn remove_unused_definitions<'a, 'tcx>(used_locals: &'a mut UsedLocals, body: &mut Body<'tcx>) {
    // The use counts are updated as we remove the statements. A local might become unused
    // during the retain operation, leading to a temporary inconsistency (storage statements or
    // definitions referencing the local might remain). For correctness it is crucial that this
    // computation reaches a fixed point.
    loop {
        let mut modified = false;

        for data in body.basic_blocks_mut() {
            // Remove unnecessary StorageLive and StorageDead annotations.
            let old_len = data.statements.len();
            data.statements.retain(|s| used_locals.try_remove_statement(s).is_err());
            let new_len = data.statements.len();
            modified |= old_len != new_len;
        }

        if !modified {
            break;
        }
    }

    if !used_locals.preserve_var_debug_info {
        body.var_debug_info.retain(|v| used_locals.try_remove_var_debug_info(v).is_err());
    }
}

struct LocalUpdater<'tcx> {
    map: IndexVec<Local, Option<Local>>,
    tcx: TyCtxt<'tcx>,
}

impl<'tcx> MutVisitor<'tcx> for LocalUpdater<'tcx> {
    fn tcx(&self) -> TyCtxt<'tcx> {
        self.tcx
    }

    fn visit_local(&mut self, l: &mut Local, _: PlaceContext, _: Location) {
        *l = self.map[*l].unwrap();
    }
}
