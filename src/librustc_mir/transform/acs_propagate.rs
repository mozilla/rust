// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! This is Alias-Constant-Simplify propagation pass. This is a composition of three distinct
//! dataflow passes: alias-propagation, constant-propagation and terminator simplification.
//!
//! All these are very similar in their nature:
//!
//!                 | Constant  |  Alias   | Simplify  |
//!|----------------|-----------|----------|-----------|
//!| Lattice Domain | Lvalue    | Lvalue   | Lvalue    |
//!| Lattice Value  | Constant  | Lvalue   | Constant  |
//!| Transfer       | x = const | x = lval | x = const |
//!| Rewrite        | x → const | x → lval | T(x) → T' |
//!| Bottom         | {}        | {}       | {}        |
//!
//! For all of them we will be using a lattice of Hashmap from Lvalue to
//! WTop<Either<Lvalue, Constant>>
//!
//! My personal believ is that it should be possible to make a way to compose two hashmap lattices
//! into one, but I can’t seem to get it just right yet, so we do the composing and decomposing
//! manually here.

use rustc_data_structures::fnv::FnvHashMap;
use rustc_data_structures::bitvec::BitVector;
use rustc::mir::repr::*;
use rustc::mir::visit::{MutVisitor, LvalueContext};
use rustc::mir::transform::lattice::Lattice;
use rustc::mir::transform::dataflow::*;
use rustc::mir::transform::{Pass, MirPass, MirSource};
use rustc::ty::TyCtxt;
use rustc::middle::const_val::ConstVal;
use pretty;

#[derive(PartialEq, Debug, Eq, Clone)]
pub enum Either<'tcx> {
    Top,
    Lvalue(Lvalue<'tcx>),
    Const(Constant<'tcx>),
}

impl<'tcx> Lattice for Either<'tcx> {
    fn bottom() -> Self { unimplemented!() }
    fn join(&mut self, other: &Self) -> bool {
        if self == other {
            false
        } else {
            *self = Either::Top;
            true
        }
    }
}

pub type ACSLattice<'a> = FnvHashMap<Lvalue<'a>, Either<'a>>;

pub struct ACSPropagate;

impl Pass for ACSPropagate {}

impl<'tcx> MirPass<'tcx> for ACSPropagate {
    fn run_pass<'a>(&mut self, tcx: TyCtxt<'a, 'tcx, 'tcx>, src: MirSource, mir: &mut Mir<'tcx>) {
        let mut q = BitVector::new(mir.cfg.len());
        q.insert(START_BLOCK.index());
        let ret = ar_forward::<ACSPropagateTransfer, ACSPropagate>(&mut mir.cfg, Facts::new(), q);
        mir.cfg = ret.0;
        pretty::dump_mir(tcx, "acs_propagate", &0, src, mir, None);
    }

}

impl<'tcx> DataflowPass<'tcx> for ACSPropagate {
    type Lattice = ACSLattice<'tcx>;
    type Rewrite = RewriteAndThen<'tcx, AliasRewrite,
                                  RewriteAndThen<'tcx, ConstRewrite, SimplifyRewrite>>;
    type Transfer = ACSPropagateTransfer;
}

pub struct ACSPropagateTransfer;

impl<'tcx> Transfer<'tcx, ACSLattice<'tcx>> for ACSPropagateTransfer {
    type TerminatorOut = Vec<ACSLattice<'tcx>>;
    fn stmt(s: &Statement<'tcx>, mut lat: ACSLattice<'tcx>) -> ACSLattice<'tcx> {
        let StatementKind::Assign(ref lval, ref rval) = s.kind;
        match *rval {
            Rvalue::Use(Operand::Consume(ref nlval)) =>
                lat.insert(lval.clone(), Either::Lvalue(nlval.clone())),
            Rvalue::Use(Operand::Constant(ref c)) =>
                lat.insert(lval.clone(), Either::Const(c.clone())),
            _ => lat.insert(lval.clone(), Either::Top)
        };
        lat
    }
    fn term(t: &Terminator<'tcx>, lat: ACSLattice<'tcx>) -> Self::TerminatorOut {
        // FIXME: this should inspect the terminators and set their known values to constants. Esp.
        // for the if: in the truthy branch the operand is known to be true and in the falsy branch
        // the operand is known to be false. Now we just ignore the potential here.
        let mut ret = vec![];
        ret.resize(t.successors().len(), lat);
        ret
    }
}

pub struct AliasRewrite;

impl<'tcx> Rewrite<'tcx, ACSLattice<'tcx>> for AliasRewrite {
    fn stmt(s: &Statement<'tcx>, l: &ACSLattice<'tcx>, cfg: &mut CFG<'tcx>)
    -> StatementChange<'tcx> {
        let mut ns = s.clone();
        let mut vis = RewriteAliasVisitor(&l, false);
        vis.visit_statement(START_BLOCK, &mut ns);
        if vis.1 { StatementChange::Statement(ns) } else { StatementChange::None }
    }
    fn term(t: &Terminator<'tcx>, l: &ACSLattice<'tcx>, cfg: &mut CFG<'tcx>)
    -> TerminatorChange<'tcx> {
        let mut nt = t.clone();
        let mut vis = RewriteAliasVisitor(&l, false);
        vis.visit_terminator(START_BLOCK, &mut nt);
        if vis.1 { TerminatorChange::Terminator(nt) } else { TerminatorChange::None }
    }
}

struct RewriteAliasVisitor<'a, 'tcx: 'a>(pub &'a ACSLattice<'tcx>, pub bool);
impl<'a, 'tcx> MutVisitor<'tcx> for RewriteAliasVisitor<'a, 'tcx> {
    fn visit_lvalue(&mut self, lvalue: &mut Lvalue<'tcx>, context: LvalueContext) {
        match context {
            LvalueContext::Store | LvalueContext::Call => {}
            _ => {
                let replacement = self.0.get(lvalue);
                match replacement {
                    Some(&Either::Lvalue(ref nlval)) => {
                        self.1 = true;
                        *lvalue = nlval.clone();
                    }
                    _ => {}
                }
            }
        }
        self.super_lvalue(lvalue, context);
    }
}

pub struct ConstRewrite;

impl<'tcx> Rewrite<'tcx, ACSLattice<'tcx>> for ConstRewrite {
    fn stmt(s: &Statement<'tcx>, l: &ACSLattice<'tcx>, cfg: &mut CFG<'tcx>)
    -> StatementChange<'tcx> {
        let mut ns = s.clone();
        let mut vis = RewriteConstVisitor(&l, false);
        vis.visit_statement(START_BLOCK, &mut ns);
        if vis.1 { StatementChange::Statement(ns) } else { StatementChange::None }
    }
    fn term(t: &Terminator<'tcx>, l: &ACSLattice<'tcx>, cfg: &mut CFG<'tcx>)
    -> TerminatorChange<'tcx> {
        let mut nt = t.clone();
        let mut vis = RewriteConstVisitor(&l, false);
        vis.visit_terminator(START_BLOCK, &mut nt);
        if vis.1 { TerminatorChange::Terminator(nt) } else { TerminatorChange::None }
    }
}

struct RewriteConstVisitor<'a, 'tcx: 'a>(pub &'a ACSLattice<'tcx>, pub bool);
impl<'a, 'tcx> MutVisitor<'tcx> for RewriteConstVisitor<'a, 'tcx> {
    fn visit_operand(&mut self, op: &mut Operand<'tcx>) {
        let repl = if let Operand::Consume(ref lval) = *op {
            if let Some(&Either::Const(ref c)) = self.0.get(lval) {
                Some(c.clone())
            } else {
                None
            }
        } else {
            None
        };
        if let Some(c) = repl {
            *op = Operand::Constant(c);
        }
        self.super_operand(op);
    }
}


pub struct SimplifyRewrite;

impl<'tcx> Rewrite<'tcx, ACSLattice<'tcx>> for SimplifyRewrite {
    fn stmt(s: &Statement<'tcx>, l: &ACSLattice<'tcx>, cfg: &mut CFG<'tcx>)
    -> StatementChange<'tcx> {
        StatementChange::None
    }
    fn term(t: &Terminator<'tcx>, l: &ACSLattice<'tcx>, cfg: &mut CFG<'tcx>)
    -> TerminatorChange<'tcx> {
        match t.kind {
            TerminatorKind::If { ref targets, .. } if targets.0 == targets.1 => {
                let mut nt = t.clone();
                nt.kind = TerminatorKind::Goto { target: targets.0 };
                TerminatorChange::Terminator(nt)
            }
            TerminatorKind::If { ref targets, cond: Operand::Constant(Constant {
                literal: Literal::Value {
                    value: ConstVal::Bool(cond)
                }, ..
            }) } => {
                let mut nt = t.clone();
                if cond {
                    nt.kind = TerminatorKind::Goto { target: targets.0 };
                } else {
                    nt.kind = TerminatorKind::Goto { target: targets.1 };
                }
                TerminatorChange::Terminator(nt)
            }
            TerminatorKind::SwitchInt { ref targets, .. } if targets.len() == 1 => {
                let mut nt = t.clone();
                nt.kind = TerminatorKind::Goto { target: targets[0] };
                TerminatorChange::Terminator(nt)
            }
            _ => TerminatorChange::None
        }
    }
}
