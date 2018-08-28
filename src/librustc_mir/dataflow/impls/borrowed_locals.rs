// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

pub use super::*;

use dataflow::BitDenotation;
use rustc::mir::visit::Visitor;
use rustc::mir::*;

/// This calculates if any part of a MIR local could have previously been borrowed.
/// This means that once a local has been borrowed, its bit will be set
/// from that point and onwards, until we see a StorageDead statement for the local,
/// at which points there is no memory associated with the local, so it cannot be borrowed.
/// This is used to compute which locals are live during a yield expression for
/// immovable generators.
#[derive(Copy, Clone)]
pub struct HaveBeenBorrowedLocals<'a, 'tcx: 'a> {
    mir: &'a Mir<'tcx>,
}

impl<'a, 'tcx: 'a> HaveBeenBorrowedLocals<'a, 'tcx> {
    pub fn new(mir: &'a Mir<'tcx>) -> Self {
        HaveBeenBorrowedLocals { mir }
    }

    pub fn mir(&self) -> &Mir<'tcx> {
        self.mir
    }
}

impl<'a, 'tcx> BitDenotation for HaveBeenBorrowedLocals<'a, 'tcx> {
    type Idx = Local;

    fn name() -> &'static str {
        "has_been_borrowed_locals"
    }

    fn bits_per_block(&self) -> usize {
        self.mir.local_decls.len()
    }

    fn start_block_effect(&self, _sets: &mut IdxSet<Local>) {
        // Nothing is borrowed on function entry
    }

    fn statement_effect(&self, sets: &mut BlockSets<Local>, loc: Location) {
        let stmt = &self.mir[loc.block].statements[loc.statement_index];

        BorrowedLocalsVisitor {
            sets,
        }.visit_statement(loc.block, stmt, loc);

        // StorageDead invalidates all borrows and raw pointers to a local
        match stmt.kind {
            StatementKind::StorageDead(l) => sets.kill(&l),
            _ => (),
        }
    }

    fn terminator_effect(&self, sets: &mut BlockSets<Local>, loc: Location) {
        BorrowedLocalsVisitor {
            sets,
        }.visit_terminator(loc.block, self.mir[loc.block].terminator(), loc);
    }

    fn propagate_call_return(
        &self,
        _in_out: &mut IdxSet<Local>,
        _call_bb: mir::BasicBlock,
        _dest_bb: mir::BasicBlock,
        _dest_place: &mir::Place,
    ) {
        // Nothing to do when a call returns successfully
    }
}

impl<'a, 'tcx> BitwiseOperator for HaveBeenBorrowedLocals<'a, 'tcx> {
    #[inline]
    fn join(&self, pred1: Word, pred2: Word) -> Word {
        pred1 | pred2 // "maybe" means we union effects of both preds
    }
}

impl<'a, 'tcx> InitialFlow for HaveBeenBorrowedLocals<'a, 'tcx> {
    #[inline]
    fn bottom_value() -> bool {
        false // bottom = unborrowed
    }
}

struct BorrowedLocalsVisitor<'b, 'c: 'b> {
    sets: &'b mut BlockSets<'c, Local>,
}

impl<'b, 'c, 'tcx> BorrowedLocalsVisitor<'b, 'c> {
    fn find_local(&self, place: &Place<'tcx>) -> Option<Local> {
        let mut local = match place.base {
            PlaceBase::Local(local) => Some(local),
            PlaceBase::Promoted(_) | PlaceBase::Static(..) => None,
        };
        if !place.has_no_projection() {
            for elem in place.elems.iter() {
                match elem {
                    ProjectionElem::Deref => local = None,
                    _ => {},
                }
            }
        }
        local
    }
}

impl<'tcx, 'b, 'c> Visitor<'tcx> for BorrowedLocalsVisitor<'b, 'c> {
    fn visit_rvalue(&mut self, rvalue: &Rvalue<'tcx>, location: Location) {
        if let Rvalue::Ref(_, _, ref place) = *rvalue {
            if let Some(local) = self.find_local(&place) {
                self.sets.gen(&local);
            }
        }

        self.super_rvalue(rvalue, location)
    }
}
