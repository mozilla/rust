pub use super::*;

use rustc::mir::*;
use dataflow::BitDenotation;

#[derive(Copy, Clone)]
pub struct MaybeStorageLive<'a, 'tcx: 'a> {
    mir: &'a Mir<'tcx>,
}

impl<'a, 'tcx: 'a> MaybeStorageLive<'a, 'tcx> {
    pub fn new(mir: &'a Mir<'tcx>)
               -> Self {
        MaybeStorageLive { mir: mir }
    }

    pub fn mir(&self) -> &Mir<'tcx> {
        self.mir
    }
}

impl<'a, 'tcx> BitDenotation for MaybeStorageLive<'a, 'tcx> {
    type Idx = Local;
    fn name() -> &'static str { "maybe_storage_live" }
    fn bits_per_block(&self) -> usize {
        self.mir.local_decls.len()
    }

    fn start_block_effect(&self, _sets: &mut IdxSet<Local>) {
        // Nothing is live on function entry
    }

    fn statement_effect(&self,
                        sets: &mut BlockSets<Local>,
                        loc: Location) {
        let stmt = &self.mir[loc.block].statements[loc.statement_index];

        match stmt.kind {
            StatementKind::StorageLive(l) => sets.gen(&l),
            StatementKind::StorageDead(l) => sets.kill(&l),
            _ => (),
        }
    }

    fn terminator_effect(&self,
                         _sets: &mut BlockSets<Local>,
                         _loc: Location) {
        // Terminators have no effect
    }

    fn propagate_call_return(&self,
                             _in_out: &mut IdxSet<Local>,
                             _call_bb: mir::BasicBlock,
                             _dest_bb: mir::BasicBlock,
                             _dest_place: &mir::Place) {
        // Nothing to do when a call returns successfully
    }
}

impl<'a, 'tcx> BitwiseOperator for MaybeStorageLive<'a, 'tcx> {
    #[inline]
    fn join(&self, pred1: Word, pred2: Word) -> Word {
        pred1 | pred2 // "maybe" means we union effects of both preds
    }
}

impl<'a, 'tcx> InitialFlow for MaybeStorageLive<'a, 'tcx> {
    #[inline]
    fn bottom_value() -> bool {
        false // bottom = dead
    }
}
