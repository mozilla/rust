// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Performs various peephole optimizations.

use rustc::mir::{Constant, Location, Place, Mir, Operand, ProjectionElem, Rvalue, Local};
use rustc::mir::visit::{MutVisitor, Visitor};
use rustc::mir::tcx::PlaceTy;
use rustc::ty::{TyCtxt, TypeVariants};
use rustc::util::nodemap::{FxHashMap, FxHashSet};
use rustc_data_structures::indexed_vec::Idx;
use std::mem;
use transform::{MirPass, MirSource};

pub struct InstCombine;

impl MirPass for InstCombine {
    fn run_pass<'a, 'tcx>(&self,
                          tcx: TyCtxt<'a, 'tcx, 'tcx>,
                          _: MirSource,
                          mir: &mut Mir<'tcx>) {
        // We only run when optimizing MIR (at any level).
        if tcx.sess.opts.debugging_opts.mir_opt_level == 0 {
            return
        }

        // First, find optimization opportunities. This is done in a pre-pass to keep the MIR
        // read-only so that we can do global analyses on the MIR in the process (e.g.
        // `Place::ty()`).
        let optimizations = {
            let mut optimization_finder = OptimizationFinder::new(mir, tcx);
            optimization_finder.visit_mir(mir);
            optimization_finder.optimizations
        };

        // Then carry out those optimizations.
        MutVisitor::visit_mir(&mut InstCombineVisitor { optimizations }, mir);
    }
}

pub struct InstCombineVisitor<'tcx> {
    optimizations: OptimizationList<'tcx>,
}

impl<'tcx> MutVisitor<'tcx> for InstCombineVisitor<'tcx> {
    fn visit_rvalue(&mut self, rvalue: &mut Rvalue<'tcx>, location: Location) {
        if self.optimizations.and_stars.remove(&location) {
            let local = Place::local(Local::new(0));
            debug!("Replacing `&*`: {:?}", rvalue);
            let new_place = match rvalue {
                Rvalue::Ref(_, _, place) => {
                    // Replace with dummy
                    mem::replace(&mut place.clone(), local)
                }
                _ => bug!("Detected `&*` but didn't find `&*`!"),
            };
            *rvalue = Rvalue::Use(Operand::Copy(new_place.clone()))
        }

        if let Some(constant) = self.optimizations.arrays_lengths.remove(&location) {
            debug!("Replacing `Len([_; N])`: {:?}", rvalue);
            *rvalue = Rvalue::Use(Operand::Constant(box constant));
        }

        self.super_rvalue(rvalue, location)
    }
}

/// Finds optimization opportunities on the MIR.
struct OptimizationFinder<'b, 'a, 'tcx:'a+'b> {
    mir: &'b Mir<'tcx>,
    tcx: TyCtxt<'a, 'tcx, 'tcx>,
    optimizations: OptimizationList<'tcx>,
}

impl<'b, 'a, 'tcx:'b> OptimizationFinder<'b, 'a, 'tcx> {
    fn new(mir: &'b Mir<'tcx>, tcx: TyCtxt<'a, 'tcx, 'tcx>) -> OptimizationFinder<'b, 'a, 'tcx> {
        OptimizationFinder {
            mir,
            tcx,
            optimizations: OptimizationList::default(),
        }
    }
}

impl<'b, 'a, 'tcx> Visitor<'tcx> for OptimizationFinder<'b, 'a, 'tcx> {
    fn visit_rvalue(&mut self, rvalue: &Rvalue<'tcx>, location: Location) {
        if let Rvalue::Ref(_, _, ref place) = *rvalue {
            if !place.has_no_projection() {
                let mut base_ty = place.base.ty(self.mir);
                for elem in place.elems.iter() {
                    if let ProjectionElem::Deref = elem {
                        if base_ty.is_region_ptr() {
                            self.optimizations.and_stars.insert(location);
                        }
                    }
                    base_ty = PlaceTy::from(base_ty)
                                .projection_ty(self.tcx, elem)
                                .to_ty(self.tcx);
                }
            }
        }

        if let Rvalue::Len(ref place) = *rvalue {
            let place_ty = place.ty(&self.mir.local_decls, self.tcx).to_ty(self.tcx);
            if let TypeVariants::TyArray(_, len) = place_ty.sty {
                let span = self.mir.source_info(location).span;
                let ty = self.tcx.types.usize;
                let constant = Constant { span, ty, literal: len };
                self.optimizations.arrays_lengths.insert(location, constant);
            }
        }

        self.super_rvalue(rvalue, location)
    }
}

#[derive(Default)]
struct OptimizationList<'tcx> {
    and_stars: FxHashSet<Location>,
    arrays_lengths: FxHashMap<Location, Constant<'tcx>>,
}
