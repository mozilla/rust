// Copyright 2012-2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use borrowck::BorrowckCtxt;

use syntax::ast::{self, MetaItem};
use syntax::attr::AttrMetaMethods;
use syntax::codemap::{Span, DUMMY_SP};
use syntax::ptr::P;

use rustc::hir;
use rustc::hir::intravisit::{FnKind};

use rustc::mir::repr;
use rustc::mir::repr::{BasicBlock, BasicBlockData, Mir, Statement, Terminator};
use rustc::session::Session;
use rustc::ty::{self, TyCtxt};

mod abs_domain;
mod dataflow;
mod gather_moves;
// mod graphviz;

use self::dataflow::{BitDenotation};
use self::dataflow::{Dataflow, DataflowAnalysis, DataflowResults};
use self::dataflow::{HasMoveData};
use self::dataflow::{MaybeInitializedLvals, MaybeUninitializedLvals};
use self::dataflow::{DefinitelyInitializedLvals};
use self::gather_moves::{MoveData, MovePathIndex, Location};
use self::gather_moves::{MovePathContent};

use std::fmt::Debug;

fn has_rustc_mir_with(attrs: &[ast::Attribute], name: &str) -> Option<P<MetaItem>> {
    for attr in attrs {
        if attr.check_name("rustc_mir") {
            let items = attr.meta_item_list();
            for item in items.iter().flat_map(|l| l.iter()) {
                if item.check_name(name) {
                    return Some(item.clone())
                }
            }
        }
    }
    return None;
}

pub fn borrowck_mir<'a, 'tcx: 'a>(
    bcx: &mut BorrowckCtxt<'a, 'tcx>,
    fk: FnKind,
    _decl: &hir::FnDecl,
    mir: &'a Mir<'tcx>,
    body: &hir::Block,
    _sp: Span,
    id: ast::NodeId,
    attributes: &[ast::Attribute]) {
    match fk {
        FnKind::ItemFn(name, _, _, _, _, _, _) |
        FnKind::Method(name, _, _, _) => {
            debug!("borrowck_mir({}) UNIMPLEMENTED", name);
        }
        FnKind::Closure(_) => {
            debug!("borrowck_mir closure (body.id={}) UNIMPLEMENTED", body.id);
        }
    }

    let tcx = bcx.tcx;

    let move_data = MoveData::gather_moves(mir, tcx);
    let ctxt = (tcx, mir, move_data);
    let (ctxt, flow_inits) =
        do_dataflow(tcx, mir, id, attributes, ctxt, MaybeInitializedLvals::default());
    let (ctxt, flow_uninits) =
        do_dataflow(tcx, mir, id, attributes, ctxt, MaybeUninitializedLvals::default());
    let (ctxt, flow_def_inits) =
        do_dataflow(tcx, mir, id, attributes, ctxt, DefinitelyInitializedLvals::default());

    if has_rustc_mir_with(attributes, "rustc_peek_maybe_init").is_some() {
        dataflow::sanity_check_via_rustc_peek(bcx.tcx, mir, id, attributes, &ctxt, &flow_inits);
    }
    if has_rustc_mir_with(attributes, "rustc_peek_maybe_uninit").is_some() {
        dataflow::sanity_check_via_rustc_peek(bcx.tcx, mir, id, attributes, &ctxt, &flow_uninits);
    }
    if has_rustc_mir_with(attributes, "rustc_peek_definite_init").is_some() {
        dataflow::sanity_check_via_rustc_peek(bcx.tcx, mir, id, attributes, &ctxt, &flow_def_inits);
    }
    let move_data = ctxt.2;

    if has_rustc_mir_with(attributes, "stop_after_dataflow").is_some() {
        bcx.tcx.sess.fatal("stop_after_dataflow ended compilation");
    }

    let mut mbcx = MirBorrowckCtxt {
        bcx: bcx,
        mir: mir,
        node_id: id,
        move_data: move_data,
        flow_inits: flow_inits,
        flow_uninits: flow_uninits,
    };

    for bb in mir.all_basic_blocks() {
        mbcx.process_basic_block(bb);
    }

    debug!("borrowck_mir done");
}

fn do_dataflow<'a, 'tcx, BD>(tcx: TyCtxt<'a, 'tcx, 'tcx>,
                             mir: &Mir<'tcx>,
                             node_id: ast::NodeId,
                             attributes: &[ast::Attribute],
                             ctxt: BD::Ctxt,
                             bd: BD) -> (BD::Ctxt, DataflowResults<BD>)
    where BD: BitDenotation, BD::Bit: Debug, BD::Ctxt: HasMoveData<'tcx>
{
    use syntax::attr::AttrMetaMethods;

    let name_found = |sess: &Session, attrs: &[ast::Attribute], name| -> Option<String> {
        if let Some(item) = has_rustc_mir_with(attrs, name) {
            if let Some(s) = item.value_str() {
                return Some(s.to_string())
            } else {
                sess.span_err(
                    item.span,
                    &format!("{} attribute requires a path", item.name()));
                return None;
            }
        }
        return None;
    };

    let print_preflow_to =
        name_found(tcx.sess, attributes, "borrowck_graphviz_preflow");
    let print_postflow_to =
        name_found(tcx.sess, attributes, "borrowck_graphviz_postflow");

    let mut mbcx = MirBorrowckCtxtPreDataflow {
        node_id: node_id,
        print_preflow_to: print_preflow_to,
        print_postflow_to: print_postflow_to,
        flow_state: DataflowAnalysis::new(tcx, mir, ctxt, bd),
    };

    mbcx.dataflow();
    mbcx.flow_state.results()
}


pub struct MirBorrowckCtxtPreDataflow<'a, 'tcx: 'a, BD>
    where BD: BitDenotation, BD::Ctxt: HasMoveData<'tcx>
{
    node_id: ast::NodeId,
    flow_state: DataflowAnalysis<'a, 'tcx, BD>,
    print_preflow_to: Option<String>,
    print_postflow_to: Option<String>,
}

#[allow(dead_code)]
pub struct MirBorrowckCtxt<'b, 'a: 'b, 'tcx: 'a> {
    bcx: &'b mut BorrowckCtxt<'a, 'tcx>,
    mir: &'b Mir<'tcx>,
    node_id: ast::NodeId,
    move_data: MoveData<'tcx>,
    flow_inits: DataflowResults<MaybeInitializedLvals<'a, 'tcx>>,
    flow_uninits: DataflowResults<MaybeUninitializedLvals<'a, 'tcx>>
}

impl<'b, 'a: 'b, 'tcx: 'a> MirBorrowckCtxt<'b, 'a, 'tcx> {
    fn process_basic_block(&mut self, bb: BasicBlock) {
        let &BasicBlockData { ref statements, ref terminator, is_cleanup: _ } =
            self.mir.basic_block_data(bb);
        for stmt in statements {
            self.process_statement(bb, stmt);
        }

        self.process_terminator(bb, terminator);
    }

    fn process_statement(&mut self, bb: BasicBlock, stmt: &Statement<'tcx>) {
        debug!("MirBorrowckCtxt::process_statement({:?}, {:?}", bb, stmt);
    }

    fn process_terminator(&mut self, bb: BasicBlock, term: &Option<Terminator<'tcx>>) {
        debug!("MirBorrowckCtxt::process_terminator({:?}, {:?})", bb, term);
    }
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
enum DropFlagState {
    Present, // i.e. initialized
    Absent, // i.e. deinitialized or "moved"
}

fn on_all_children_bits<'a, 'tcx, F>(
    tcx: TyCtxt<'a, 'tcx, 'tcx>,
    mir: &Mir<'tcx>,
    move_data: &MoveData<'tcx>,
    move_path_index: MovePathIndex,
    mut each_child: F)
    where F: FnMut(MovePathIndex)
{
    fn is_terminal_path<'a, 'tcx>(
        tcx: TyCtxt<'a, 'tcx, 'tcx>,
        mir: &Mir<'tcx>,
        move_data: &MoveData<'tcx>,
        path: MovePathIndex) -> bool
    {
        match move_data.move_paths[path].content {
            MovePathContent::Lvalue(ref lvalue) => {
                match mir.lvalue_ty(tcx, lvalue).to_ty(tcx).sty {
                    // don't trace paths past arrays, slices, and
                    // pointers. They can only be accessed while
                    // their parents are initialized.
                    //
                    // FIXME: we have to do something for moving
                    // slice patterns.
                    ty::TyArray(..) | ty::TySlice(..) |
                    ty::TyRef(..) | ty::TyRawPtr(..) => true,
                    _ => false
                }
            }
            _ => true
        }
    }

    fn on_all_children_bits<'a, 'tcx, F>(
        tcx: TyCtxt<'a, 'tcx, 'tcx>,
        mir: &Mir<'tcx>,
        move_data: &MoveData<'tcx>,
        move_path_index: MovePathIndex,
        each_child: &mut F)
        where F: FnMut(MovePathIndex)
    {
        each_child(move_path_index);

        if is_terminal_path(tcx, mir, move_data, move_path_index) {
            return
        }

        let mut next_child_index = move_data.move_paths[move_path_index].first_child;
        while let Some(child_index) = next_child_index {
            on_all_children_bits(tcx, mir, move_data, child_index, each_child);
            next_child_index = move_data.move_paths[child_index].next_sibling;
        }
    }
    on_all_children_bits(tcx, mir, move_data, move_path_index, &mut each_child);
}

fn drop_flag_effects_for_function_entry<'a, 'tcx, F>(
    tcx: TyCtxt<'a, 'tcx, 'tcx>,
    mir: &Mir<'tcx>,
    move_data: &MoveData<'tcx>,
    mut callback: F)
    where F: FnMut(MovePathIndex, DropFlagState)
{
    for i in 0..(mir.arg_decls.len() as u32) {
        let lvalue = repr::Lvalue::Arg(i);
        let move_path_index = move_data.rev_lookup.find(&lvalue);
        on_all_children_bits(tcx, mir, move_data,
                             move_path_index,
                             |moi| callback(moi, DropFlagState::Present));
    }
}

fn drop_flag_effects_for_location<'a, 'tcx, F>(
    tcx: TyCtxt<'a, 'tcx, 'tcx>,
    mir: &Mir<'tcx>,
    move_data: &MoveData<'tcx>,
    loc: Location,
    mut callback: F)
    where F: FnMut(MovePathIndex, DropFlagState)
{
    debug!("drop_flag_effects_for_location({:?})", loc);

    // first, move out of the RHS
    for mi in &move_data.loc_map[loc] {
        let path = mi.move_path_index(move_data);
        debug!("moving out of path {:?}", move_data.move_paths[path]);

        // don't move out of non-Copy things
        if let MovePathContent::Lvalue(ref lvalue) = move_data.move_paths[path].content {
            let ty = mir.lvalue_ty(tcx, lvalue).to_ty(tcx);
            let empty_param_env = tcx.empty_parameter_environment();
            if !ty.moves_by_default(tcx, &empty_param_env, DUMMY_SP) {
                continue;
            }
        }

        on_all_children_bits(tcx, mir, move_data,
                             path,
                             |moi| callback(moi, DropFlagState::Absent))
    }

    let bb = mir.basic_block_data(loc.block);
    match bb.statements.get(loc.index) {
        Some(stmt) => match stmt.kind {
            repr::StatementKind::Assign(ref lvalue, _) => {
                debug!("drop_flag_effects: assignment {:?}", stmt);
                on_all_children_bits(tcx, mir, move_data,
                                     move_data.rev_lookup.find(lvalue),
                                     |moi| callback(moi, DropFlagState::Present))
            }
        },
        None => {
            // terminator - no move-ins except for function return edge
            let term = bb.terminator();
            debug!("drop_flag_effects: terminator {:?}", term);
        }
    }
}
