// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/*!

Data structures used for tracking moves. Please see the extensive
comments in the section "Moves and initialization" and in `doc.rs`.

*/

use std::cell::RefCell;
use std::rc::Rc;
use std::uint;
use std::collections::{HashMap, HashSet};
use middle::borrowck::*;
use middle::cfg;
use middle::dataflow::DataFlowContext;
use middle::dataflow::BitwiseOperator;
use middle::dataflow::DataFlowOperator;
use middle::expr_use_visitor as euv;
use middle::mem_categorization as mc;
use middle::ty;
use syntax::ast;
use syntax::ast_util;
use syntax::codemap::Span;
use util::ppaux::Repr;

pub struct MoveData {
    /// Move paths. See section "Move paths" in `doc.rs`.
    pub paths: RefCell<Vec<MovePath>>,

    /// Cache of loan path to move path index, for easy lookup.
    pub path_map: RefCell<HashMap<Rc<LoanPath>, MovePathIndex>>,

    /// Each move or uninitialized variable gets an entry here.
    pub moves: RefCell<Vec<Move>>,

    /// Assignments to a variable, like `x = foo`. These are assigned
    /// bits for dataflow, since we must track them to ensure that
    /// immutable variables are assigned at most once along each path.
    pub var_assignments: RefCell<Vec<Assignment>>,

    /// Assignments to a path, like `x.f = foo`. These are not
    /// assigned dataflow bits, but we track them because they still
    /// kill move bits.
    pub path_assignments: RefCell<Vec<Assignment>>,

    /// Enum variant matched within a pattern on some match arm, like
    /// `SomeStruct{ f: Variant1(x, y) } => ...`
    pub variant_matches: RefCell<Vec<VariantMatch>>,

    /// Assignments to a variable or path, like `x = foo`, but not `x += foo`.
    pub assignee_ids: RefCell<HashSet<ast::NodeId>>,

    /// During move_data construction, `fragments` tracks paths that
    /// *might* be needs-drop leftovers.  When move_data has been
    /// completed, `fragments` tracks paths that are *definitely*
    /// needs-drop left-overs.
    pub fragments: RefCell<Vec<MovePathIndex>>,

    /// `nonfragments` always tracks paths that have been definitely
    /// used directly (in moves).
    pub nonfragments: RefCell<Vec<MovePathIndex>>,
}

pub struct FlowedMoveData<'a, 'tcx: 'a> {
    pub move_data: MoveData,

    pub dfcx_moves: MoveDataFlow<'a, 'tcx>,

    // We could (and maybe should, for efficiency) combine both move
    // and assign data flow into one, but this way it's easier to
    // distinguish the bits that correspond to moves and assignments.
    pub dfcx_assign: AssignDataFlow<'a, 'tcx>
}

/// Index into `MoveData.paths`, used like a pointer
#[deriving(PartialEq)]
pub struct MovePathIndex(uint);

impl MovePathIndex {
    fn get(&self) -> uint {
        let MovePathIndex(v) = *self; v
    }
}

impl Clone for MovePathIndex {
    fn clone(&self) -> MovePathIndex {
        MovePathIndex(self.get())
    }
}

static InvalidMovePathIndex: MovePathIndex =
    MovePathIndex(uint::MAX);

/// Index into `MoveData.moves`, used like a pointer
#[deriving(PartialEq)]
pub struct MoveIndex(uint);

impl MoveIndex {
    fn get(&self) -> uint {
        let MoveIndex(v) = *self; v
    }
}

static InvalidMoveIndex: MoveIndex =
    MoveIndex(uint::MAX);

pub struct MovePath {
    /// Loan path corresponding to this move path
    pub loan_path: Rc<LoanPath>,

    /// Parent pointer, `InvalidMovePathIndex` if root
    pub parent: MovePathIndex,

    /// Head of linked list of moves to this path,
    /// `InvalidMoveIndex` if not moved
    pub first_move: MoveIndex,

    /// First node in linked list of children, `InvalidMovePathIndex` if leaf
    pub first_child: MovePathIndex,

    /// Next node in linked list of parent's children (siblings),
    /// `InvalidMovePathIndex` if none.
    pub next_sibling: MovePathIndex,
}

#[deriving(PartialEq)]
pub enum MoveKind {
    Declared,   // When declared, variables start out "moved".
    MoveExpr,   // Expression or binding that moves a variable
    MovePat,    // By-move binding
    Captured    // Closure creation that moves a value
}

pub struct Move {
    /// Path being moved.
    pub path: MovePathIndex,

    /// id of node that is doing the move.
    pub id: ast::NodeId,

    /// Kind of move, for error messages.
    pub kind: MoveKind,

    /// Next node in linked list of moves from `path`, or `InvalidMoveIndex`
    pub next_move: MoveIndex
}

pub struct Assignment {
    /// Path being assigned.
    pub path: MovePathIndex,

    /// id where assignment occurs
    pub id: ast::NodeId,

    /// span of node where assignment occurs
    pub span: Span,
}

pub struct VariantMatch {
    /// downcast to the variant.
    pub path: MovePathIndex,

    /// path being downcast to the variant.
    pub base_path: MovePathIndex,

    /// id where variant's pattern occurs
    pub id: ast::NodeId,

    /// says if variant established by move (and why), by copy, or by borrow.
    pub mode: euv::MatchMode
}

#[deriving(Clone)]
pub struct MoveDataFlowOperator;

pub type MoveDataFlow<'a, 'tcx> = DataFlowContext<'a, 'tcx, MoveDataFlowOperator>;

#[deriving(Clone)]
pub struct AssignDataFlowOperator;

pub type AssignDataFlow<'a, 'tcx> = DataFlowContext<'a, 'tcx, AssignDataFlowOperator>;

fn loan_path_is_precise(loan_path: &LoanPath) -> bool {
    match loan_path.variant {
        LpVar(_) | LpUpvar(..) => {
            true
        }
        LpExtend(_, _, LpInterior(mc::InteriorElement(_))) => {
            // Paths involving element accesses do not refer to a unique
            // location, as there is no accurate tracking of the indices.
            false
        }
        LpDowncast(ref lp_base, _) |
        LpExtend(ref lp_base, _, _) => {
            loan_path_is_precise(&**lp_base)
        }
    }
}

impl Move {
    pub fn to_string(&self, move_data: &MoveData, tcx: &ty::ctxt) -> String {
        format!("Move{:s} path: {}, id: {}, kind: {:?} {:s}",
                "{",
                move_data.path_loan_path(self.path).repr(tcx),
                self.id,
                self.kind,
                "}")
    }
}

impl Assignment {
    pub fn to_string(&self, move_data: &MoveData, tcx: &ty::ctxt) -> String {
        format!("Assignment{:s} path: {}, id: {} {:s}",
                "{",
                move_data.path_loan_path(self.path).repr(tcx),
                self.id,
                "}")
    }
}

impl VariantMatch {
    pub fn to_string(&self, move_data: &MoveData, tcx: &ty::ctxt) -> String {
        format!("VariantMatch{:s} path: {}, id: {} {:s}",
                "{",
                move_data.path_loan_path(self.path).repr(tcx),
                self.id,
                "}")
    }
}

impl MoveData {
    pub fn new() -> MoveData {
        MoveData {
            paths: RefCell::new(Vec::new()),
            path_map: RefCell::new(HashMap::new()),
            moves: RefCell::new(Vec::new()),
            path_assignments: RefCell::new(Vec::new()),
            var_assignments: RefCell::new(Vec::new()),
            variant_matches: RefCell::new(Vec::new()),
            assignee_ids: RefCell::new(HashSet::new()),
            fragments: RefCell::new(Vec::new()),
            nonfragments: RefCell::new(Vec::new()),
        }
    }

    pub fn path_loan_path(&self, index: MovePathIndex) -> Rc<LoanPath> {
        self.paths.borrow().get(index.get()).loan_path.clone()
    }

    fn path_parent(&self, index: MovePathIndex) -> MovePathIndex {
        self.paths.borrow().get(index.get()).parent
    }

    fn path_first_move(&self, index: MovePathIndex) -> MoveIndex {
        self.paths.borrow().get(index.get()).first_move
    }

    /// Returns true iff `index` itself cannot be directly split into
    /// child fragments.  This means it is an atomic value (like a
    /// pointer or an integer), or it a non-downcasted enum (and so we
    /// can only split off subparts when we narrow it to a particular
    /// variant), or it is a struct whose fields are never accessed in
    /// the function being compiled.
    fn path_is_leaf(&self, index: MovePathIndex, _tcx: &ty::ctxt) -> bool {
        let first_child = self.path_first_child(index);
        if first_child == InvalidMovePathIndex {
            true
        } else {
            match self.path_loan_path(first_child).variant {
                LpDowncast(..) => true,
                LpExtend(..) => false,
                LpVar(..) | LpUpvar(..) => false,
            }
        }
    }

    /// Returns true iff `index` represents downcast to an enum variant (i.e. LpDowncast).
    fn path_is_downcast_to_variant(&self, index: MovePathIndex) -> bool {
        match self.path_loan_path(index).variant {
            LpDowncast(..) => true,
            _ => false,
        }
    }

    /// Returns the index of first child, or `InvalidMovePathIndex` if
    /// `index` is leaf.
    fn path_first_child(&self, index: MovePathIndex) -> MovePathIndex {
        self.paths.borrow().get(index.get()).first_child
    }

    fn path_next_sibling(&self, index: MovePathIndex) -> MovePathIndex {
        self.paths.borrow().get(index.get()).next_sibling
    }

    fn set_path_first_move(&self,
                           index: MovePathIndex,
                           first_move: MoveIndex) {
        self.paths.borrow_mut().get_mut(index.get()).first_move = first_move
    }

    fn set_path_first_child(&self,
                            index: MovePathIndex,
                            first_child: MovePathIndex) {
        self.paths.borrow_mut().get_mut(index.get()).first_child = first_child
    }

    fn move_next_move(&self, index: MoveIndex) -> MoveIndex {
        //! Type safe indexing operator
        self.moves.borrow().get(index.get()).next_move
    }

    fn is_var_path(&self, index: MovePathIndex) -> bool {
        //! True if `index` refers to a variable
        self.path_parent(index) == InvalidMovePathIndex
    }

    pub fn move_path(&self,
                     tcx: &ty::ctxt,
                     lp: Rc<LoanPath>) -> MovePathIndex {
        /*!
         * Returns the existing move path index for `lp`, if any,
         * and otherwise adds a new index for `lp` and any of its
         * base paths that do not yet have an index.
         */

        match self.path_map.borrow().find(&lp) {
            Some(&index) => {
                return index;
            }
            None => {}
        }

        let index = match lp.variant {
            LpVar(..) | LpUpvar(..) => {
                let index = MovePathIndex(self.paths.borrow().len());

                self.paths.borrow_mut().push(MovePath {
                    loan_path: lp.clone(),
                    parent: InvalidMovePathIndex,
                    first_move: InvalidMoveIndex,
                    first_child: InvalidMovePathIndex,
                    next_sibling: InvalidMovePathIndex,
                });

                index
            }

            LpDowncast(ref base, _) |
            LpExtend(ref base, _, _) => {
                let parent_index = self.move_path(tcx, base.clone());

                let index = MovePathIndex(self.paths.borrow().len());

                let next_sibling = self.path_first_child(parent_index);
                self.set_path_first_child(parent_index, index);

                self.paths.borrow_mut().push(MovePath {
                    loan_path: lp.clone(),
                    parent: parent_index,
                    first_move: InvalidMoveIndex,
                    first_child: InvalidMovePathIndex,
                    next_sibling: next_sibling,
                });

                index
            }
        };

        debug!("move_path(lp={}, index={:?})",
               lp.repr(tcx),
               index);

        assert_eq!(index.get(), self.paths.borrow().len() - 1);
        self.path_map.borrow_mut().insert(lp, index);
        return index;
    }

    fn existing_move_path(&self, lp: &Rc<LoanPath>)
                          -> Option<MovePathIndex> {
        self.path_map.borrow().find_copy(lp)
    }

    fn existing_base_paths(&self, lp: &Rc<LoanPath>)
                           -> Vec<MovePathIndex> {
        let mut result = vec!();
        self.add_existing_base_paths(lp, &mut result);
        result
    }

    fn add_existing_base_paths(&self, lp: &Rc<LoanPath>,
                               result: &mut Vec<MovePathIndex>) {
        /*!
         * Adds any existing move path indices for `lp` and any base
         * paths of `lp` to `result`, but does not add new move paths
         */

        match self.path_map.borrow().find_copy(lp) {
            Some(index) => {
                self.each_base_path(index, |p| {
                    result.push(p);
                    true
                });
            }
            None => {
                match lp.variant {
                    LpVar(..) | LpUpvar(..) => { }
                    LpDowncast(ref b, _) |
                    LpExtend(ref b, _, _) => {
                        self.add_existing_base_paths(b, result);
                    }
                }
            }
        }

    }

    pub fn add_move(&self,
                    tcx: &ty::ctxt,
                    lp: Rc<LoanPath>,
                    id: ast::NodeId,
                    kind: MoveKind) {
        /*!
         * Adds a new move entry for a move of `lp` that occurs at
         * location `id` with kind `kind`.
         */

        debug!("add_move(lp={}, id={:?}, kind={:?})",
               lp.repr(tcx),
               id,
               kind);

        let path_index = self.move_path(tcx, lp);
        let move_index = MoveIndex(self.moves.borrow().len());

        let next_move = self.path_first_move(path_index);
        self.set_path_first_move(path_index, move_index);

        self.moves.borrow_mut().push(Move {
            path: path_index,
            id: id,
            kind: kind,
            next_move: next_move
        });
    }

    pub fn add_assignment(&self,
                          tcx: &ty::ctxt,
                          lp: Rc<LoanPath>,
                          assign_id: ast::NodeId,
                          span: Span,
                          assignee_id: ast::NodeId,
                          mode: euv::MutateMode) {
        /*!
         * Adds a new record for an assignment to `lp` that occurs at
         * location `id` with the given `span`.
         */

        debug!("add_assignment(lp={}, assign_id={:?}, assignee_id={:?}",
               lp.repr(tcx), assign_id, assignee_id);

        let path_index = self.move_path(tcx, lp.clone());

        match mode {
            euv::Init | euv::JustWrite => {
                self.assignee_ids.borrow_mut().insert(assignee_id);
            }
            euv::WriteAndRead => { }
        }

        let assignment = Assignment {
            path: path_index,
            id: assign_id,
            span: span,
        };

        if self.is_var_path(path_index) {
            debug!("add_assignment[var](lp={}, assignment={}, path_index={:?})",
                   lp.repr(tcx), self.var_assignments.borrow().len(), path_index);

            self.var_assignments.borrow_mut().push(assignment);
        } else {
            debug!("add_assignment[path](lp={}, path_index={:?})",
                   lp.repr(tcx), path_index);

            self.path_assignments.borrow_mut().push(assignment);
        }
    }

    pub fn add_variant_match(&self,
                             tcx: &ty::ctxt,
                             lp: Rc<LoanPath>,
                             pattern_id: ast::NodeId,
                             base_lp: Rc<LoanPath>,
                             mode: euv::MatchMode) {
        /*!
         * Adds a new record for a match of `base_lp`, downcast to
         * variant `lp`, that occurs at location `pattern_id`.  (One
         * should be able to recover the span info from the
         * `pattern_id` and the ast_map, I think.)
         */
        debug!("add_variant_match(lp={}, pattern_id={:?})",
               lp.repr(tcx), pattern_id);

        let path_index = self.move_path(tcx, lp.clone());
        let base_path_index = self.move_path(tcx, base_lp.clone());

        self.nonfragments.borrow_mut().push(path_index);
        let variant_match = VariantMatch {
            path: path_index,
            base_path: base_path_index,
            id: pattern_id,
            mode: mode,
        };

        self.variant_matches.borrow_mut().push(variant_match);
    }

    fn add_gen_kills(&self,
                     tcx: &ty::ctxt,
                     dfcx_moves: &mut MoveDataFlow,
                     dfcx_assign: &mut AssignDataFlow) {
        /*!
         * Adds the gen/kills for the various moves and
         * assignments into the provided data flow contexts.
         * Moves are generated by moves and killed by assignments and
         * scoping. Assignments are generated by assignment to variables and
         * killed by scoping.  Drop obligations (aka "Needs-Drop") are
         * generated by assignments and killed by moves and scoping. by
         * See `doc.rs` for more details.
         */

        {
            let mut nonfragments = {
                let mut nonfragments = self.nonfragments.borrow_mut();
                nonfragments.sort_by(|a, b| a.get().cmp(&b.get()));
                nonfragments.dedup();
                nonfragments
            };
            let mut fragments = {
                let mut maybe_fragments = self.fragments.borrow_mut();
                maybe_fragments.sort_by(|a, b| a.get().cmp(&b.get()));
                maybe_fragments.dedup();
                maybe_fragments.retain(|f| !nonfragments.contains(f));
                maybe_fragments
            };

            for (i, &nf) in nonfragments.iter().enumerate() {
                let lp = self.path_loan_path(nf);
                debug!("add_gen_kills nonfragment {:u}: {:s}", i, lp.repr(tcx));
            }

            for (i, &f) in fragments.iter().enumerate() {
                let lp = self.path_loan_path(f);
                debug!("add_gen_kills fragment {:u}: {:s}", i, lp.repr(tcx));
            }
        }

        for (i, move) in self.moves.borrow().iter().enumerate() {
            dfcx_moves.add_gen(move.id, i);
            debug!("remove_drop_obligations move {}", move.to_string(self, tcx));
            self.remove_drop_obligations(tcx, move);
        }

        for variant_match in self.variant_matches.borrow().iter() {
            match variant_match.mode {
                euv::NonBindingMatch |
                euv::BorrowingMatch |
                euv::CopyingMatch => {}
                euv::MovingMatch => {
                    debug!("remove_drop_obligations variant_match {}", variant_match.to_string(self, tcx));
                    self.remove_drop_obligations(tcx, variant_match);
                    debug!("add_drop_obligations variant_match {}", variant_match.to_string(self, tcx));
                    self.add_drop_obligations(tcx, variant_match);
                }
            }

            debug!("add_ignored_drops variant_match {}", variant_match.to_string(self, tcx));
            self.add_ignored_drops(tcx, variant_match);
        }
 
        for (i, assignment) in self.var_assignments.borrow().iter().enumerate() {
            dfcx_assign.add_gen(assignment.id, i);
            self.kill_moves(assignment.path, assignment.id, dfcx_moves);
            debug!("add_drop_obligations var_assignment {}", assignment.to_string(self, tcx));
            self.add_drop_obligations(tcx, assignment);
        }

        for assignment in self.path_assignments.borrow().iter() {
            self.kill_moves(assignment.path, assignment.id, dfcx_moves);
            debug!("add_drop_obligations path_assignment {}", assignment.to_string(self, tcx));
            self.add_drop_obligations(tcx, assignment);
        }

        // Kill all moves and drop-obligations related to a variable `x` when
        // it goes out of scope:
        for path in self.paths.borrow().iter() {
            let kill_id = path.loan_path.kill_id(tcx);
            match path.loan_path.variant {
                LpVar(..) | LpUpvar(..) | LpDowncast(..) => {
                    let move_path_index = *self.path_map.borrow().get(&path.loan_path);
                    self.kill_moves(move_path_index, kill_id, dfcx_moves);
                    debug!("remove_drop_obligations scope {} {}",
                           kill_id, path.loan_path.repr(tcx));
                    let rm = Removed { where_: kill_id, what_path: move_path_index };
                    self.remove_drop_obligations(tcx, &rm);
                }
                LpExtend(..) => {}
            }
        }

        // Kill all assignments when the variable goes out of scope:
        for (assignment_index, assignment) in self.var_assignments.borrow().iter().enumerate() {
            let kill_id = self.path_loan_path(assignment.path).kill_id(tcx);
            dfcx_assign.add_kill(kill_id, assignment_index);
        }
    }

    fn each_base_path(&self, index: MovePathIndex, f: |MovePathIndex| -> bool)
                      -> bool {
        let mut p = index;
        while p != InvalidMovePathIndex {
            if !f(p) {
                return false;
            }
            p = self.path_parent(p);
        }
        return true;
    }

    fn each_extending_path(&self,
                           index: MovePathIndex,
                           f: |MovePathIndex| -> bool)
                           -> bool {
        if !f(index) {
            return false;
        }

        let mut p = self.path_first_child(index);
        while p != InvalidMovePathIndex {
            if !self.each_extending_path(p, |x| f(x)) {
                return false;
            }
            p = self.path_next_sibling(p);
        }

        return true;
    }

    fn each_applicable_move(&self,
                            index0: MovePathIndex,
                            f: |MoveIndex| -> bool)
                            -> bool {
        let mut ret = true;
        self.each_extending_path(index0, |index| {
            let mut p = self.path_first_move(index);
            while p != InvalidMoveIndex {
                if !f(p) {
                    ret = false;
                    break;
                }
                p = self.move_next_move(p);
            }
            ret
        });
        ret
    }

    fn kill_moves(&self,
                  path: MovePathIndex,
                  kill_id: ast::NodeId,
                  dfcx_moves: &mut MoveDataFlow) {
        // We can only perform kills for paths that refer to a unique location,
        // since otherwise we may kill a move from one location with an
        // assignment referring to another location.

        let loan_path = self.path_loan_path(path);
        if loan_path_is_precise(&*loan_path) {
            self.each_applicable_move(path, |move_index| {
                dfcx_moves.add_kill(kill_id, move_index.get());
                true
            });
        }
    }

    fn path_needs_drop(&self, tcx: &ty::ctxt, move_path_index: MovePathIndex) -> bool {
        //! Returns true iff move_path_index needs drop.
        self.path_loan_path(move_path_index).needs_drop(tcx)
    }

    fn type_moves_by_default(&self, tcx: &ty::ctxt, move_path_index: MovePathIndex) -> bool {
        //! Returns true iff move_path_index moves on assignment (rather than copies).
        let path_type = self.path_loan_path(move_path_index).to_type();
        ty::type_contents(tcx, path_type).moves_by_default(tcx)
    }

    fn for_each_leaf(&self,
                     tcx: &ty::ctxt,
                     root: MovePathIndex,
                     found_leaf: |MovePathIndex|,
                     _found_variant: |MovePathIndex|) {
        //! Here we normalize a path so that it is unraveled to its
        //! consituent droppable pieces that might be independently
        //! handled by the function being compiled: e.g. `s.a.j`
        //! unravels to `{ s.a.j.x, s.a.j.y, s.a.j.z }` (assuming the
        //! function never moves out any part of those unraveled
        //! elements).
        //!
        //! Note that the callback is only invoked on unraveled leaves
        //! that also need to be dropped.

        let root_lp = self.path_loan_path(root);
        debug!("for_each_leaf(root_lp={:s})", root_lp.repr(tcx));

        if self.path_is_leaf(root, tcx) {
            found_leaf(root);
            return;
        }

        let mut stack = vec![];
        stack.push(root);
        loop {
            let top = match stack.pop() { None => break, Some(elem) => elem };
            assert!(!self.path_is_leaf(top, tcx));
            let mut child = self.path_first_child(top);
            while child != InvalidMovePathIndex {
                {
                    let top_lp = self.path_loan_path(top);
                    let child_lp = self.path_loan_path(child);
                    debug!("for_each_leaf(root_lp={:s}){:s} top_lp={:s} child_lp={:s}",
                           root_lp.repr(tcx),
                           " ".repeat(stack.len()),
                           top_lp.repr(tcx),
                           child_lp.repr(tcx));
                }

                if self.path_is_leaf(child, tcx) {
                    found_leaf(child);
                } else {
                    stack.push(child);
                }

                child = self.path_next_sibling(child);
            }
        }
    }

    fn add_drop_obligations<A:AddNeedsDropArg>(&self,
                                               tcx: &ty::ctxt,
                                               a: &A) {
        let a_path = a.path_being_established();

        let add_gen = |move_path_index| {
            if self.path_is_downcast_to_variant(a_path) {
                debug!("add_drop_obligations(a={}) {} is variant on match arm",
                       a.to_string_(self, tcx),
                       self.path_loan_path(move_path_index).repr(tcx));
            }

            if self.path_needs_drop(tcx, move_path_index) {
                debug!("add_drop_obligations(a={}) adds {}",
                       a.to_string_(self, tcx),
                       self.path_loan_path(move_path_index).repr(tcx));
            } else {
                debug!("add_drop_obligations(a={}) skips non-drop {}",
                       a.to_string_(self, tcx),
                       self.path_loan_path(move_path_index).repr(tcx));
            }
        };

        let report_variant = |move_path_index| {
            debug!("add_drop_obligations(a={}) skips variant {}",
                   a.to_string_(self, tcx),
                   self.path_loan_path(move_path_index).repr(tcx));
        };

        self.for_each_leaf(tcx, a_path, add_gen, report_variant);
    }

    fn remove_drop_obligations<A:RemoveNeedsDropArg>(&self,
                                                     tcx: &ty::ctxt,
                                                     a: &A) {
        //! Kills all of the fragment leaves of path.
        //!
        //! Also kills all parents of path: while we do normalize a
        //! path to its fragment leaves, (e.g. `a.j` to `{a.j.x,
        //! a.j.y, a.j.z}`, an enum variant's path `(b:Variant1).x`
        //! has the parent `b` that is itself considered a "leaf" for
        //! the purposes of tracking drop obligations.

        let id = a.node_id_removing_obligation();
        let path : MovePathIndex = a.path_being_moved();

        let add_kill = |move_path_index| {
            if self.type_moves_by_default(tcx, move_path_index) {
                debug!("remove_drop_obligations(id={}) removes {}",
                       id, self.path_loan_path(move_path_index).repr(tcx));
            } else {
                debug!("remove_drop_obligations(id={}) skips copyable {}",
                       id, self.path_loan_path(move_path_index).repr(tcx));
            }
        };

        let report_variant = |move_path_index| {
            debug!("remove_drop_obligations(id={}) skips variant {}",
                   id, self.path_loan_path(move_path_index).repr(tcx));
        };

        self.for_each_leaf(tcx, path, add_kill, report_variant);
    }

    fn add_ignored_drops(&self,
                         tcx: &ty::ctxt,
                         variant_match: &VariantMatch) {
        let path_lp = self.path_loan_path(variant_match.path);
        let base_path_lp = self.path_loan_path(variant_match.base_path);

        if !self.path_needs_drop(tcx, variant_match.path) {
            debug!("add_ignored_drops(id={} lp={}) adds {}",
                   variant_match.id, path_lp.repr(tcx), base_path_lp.repr(tcx));
        } else {
            debug!("add_ignored_drops(id={} lp={}) skipped {}",
                   variant_match.id, path_lp.repr(tcx), base_path_lp.repr(tcx));
        }
    }
}

trait AddNeedsDropArg {
    fn node_id_adding_obligation(&self) -> ast::NodeId;
    fn path_being_established(&self) -> MovePathIndex;
    fn to_string_(&self, move_data: &MoveData, tcx: &ty::ctxt) -> String;
}
impl AddNeedsDropArg for Assignment {
    fn node_id_adding_obligation(&self) -> ast::NodeId { self.id }
    fn path_being_established(&self) -> MovePathIndex { self.path }
    fn to_string_(&self, md: &MoveData, tcx: &ty::ctxt) -> String { self.to_string(md, tcx) }
}
impl AddNeedsDropArg for VariantMatch {
    fn node_id_adding_obligation(&self) -> ast::NodeId { self.id }
    fn path_being_established(&self) -> MovePathIndex { self.path }
    fn to_string_(&self, md: &MoveData, tcx: &ty::ctxt) -> String { self.to_string(md, tcx) }
}

trait RemoveNeedsDropArg {
    fn node_id_removing_obligation(&self) -> ast::NodeId;
    fn path_being_moved(&self) -> MovePathIndex;
}
struct Removed { where_: ast::NodeId, what_path: MovePathIndex }
impl RemoveNeedsDropArg for Removed {
    fn node_id_removing_obligation(&self) -> ast::NodeId { self.where_ }
    fn path_being_moved(&self) -> MovePathIndex { self.what_path }
}
impl<'a> RemoveNeedsDropArg for Move {
    fn node_id_removing_obligation(&self) -> ast::NodeId { self.id }
    fn path_being_moved(&self) -> MovePathIndex { self.path }
}
impl<'a> RemoveNeedsDropArg for VariantMatch {
    fn node_id_removing_obligation(&self) -> ast::NodeId { self.id }
    fn path_being_moved(&self) -> MovePathIndex { self.base_path }
}


impl<'a, 'tcx> FlowedMoveData<'a, 'tcx> {
    pub fn new(move_data: MoveData,
               tcx: &'a ty::ctxt<'tcx>,
               cfg: &cfg::CFG,
               id_range: ast_util::IdRange,
               decl: &ast::FnDecl,
               body: &ast::Block)
               -> FlowedMoveData<'a, 'tcx> {
        let mut dfcx_moves =
            DataFlowContext::new(tcx,
                                 "flowed_move_data_moves",
                                 Some(decl),
                                 cfg,
                                 MoveDataFlowOperator,
                                 id_range,
                                 move_data.moves.borrow().len());
        let mut dfcx_assign =
            DataFlowContext::new(tcx,
                                 "flowed_move_data_assigns",
                                 Some(decl),
                                 cfg,
                                 AssignDataFlowOperator,
                                 id_range,
                                 move_data.var_assignments.borrow().len());

        move_data.add_gen_kills(tcx,
                                &mut dfcx_moves,
                                &mut dfcx_assign);

        dfcx_moves.add_kills_from_flow_exits(cfg);
        dfcx_assign.add_kills_from_flow_exits(cfg);

        dfcx_moves.propagate(cfg, body);
        dfcx_assign.propagate(cfg, body);

        FlowedMoveData {
            move_data: move_data,
            dfcx_moves: dfcx_moves,
            dfcx_assign: dfcx_assign,
        }
    }

    pub fn each_path_moved_by(&self,
                              id: ast::NodeId,
                              f: |&Move, &LoanPath| -> bool)
                              -> bool {
        /*!
         * Iterates through each path moved by `id`
         */

        self.dfcx_moves.each_gen_bit(id, |index| {
            let move = self.move_data.moves.borrow();
            let move = move.get(index);
            let moved_path = move.path;
            f(move, &*self.move_data.path_loan_path(moved_path))
        })
    }

    pub fn kind_of_move_of_path(&self,
                                id: ast::NodeId,
                                loan_path: &Rc<LoanPath>)
                                -> Option<MoveKind> {
        //! Returns the kind of a move of `loan_path` by `id`, if one exists.

        let mut ret = None;
        for loan_path_index in self.move_data.path_map.borrow().find(&*loan_path).iter() {
            self.dfcx_moves.each_gen_bit(id, |move_index| {
                let move = self.move_data.moves.borrow();
                let move = move.get(move_index);
                if move.path == **loan_path_index {
                    ret = Some(move.kind);
                    false
                } else {
                    true
                }
            });
        }
        ret
    }

    pub fn each_move_of(&self,
                        id: ast::NodeId,
                        loan_path: &Rc<LoanPath>,
                        f: |&Move, &LoanPath| -> bool)
                        -> bool {
        /*!
         * Iterates through each move of `loan_path` (or some base path
         * of `loan_path`) that *may* have occurred on entry to `id` without
         * an intervening assignment. In other words, any moves that
         * would invalidate a reference to `loan_path` at location `id`.
         */

        // Bad scenarios:
        //
        // 1. Move of `a.b.c`, use of `a.b.c`
        // 2. Move of `a.b.c`, use of `a.b.c.d`
        // 3. Move of `a.b.c`, use of `a` or `a.b`
        //
        // OK scenario:
        //
        // 4. move of `a.b.c`, use of `a.b.d`

        let base_indices = self.move_data.existing_base_paths(loan_path);
        if base_indices.is_empty() {
            return true;
        }

        let opt_loan_path_index = self.move_data.existing_move_path(loan_path);

        let mut ret = true;

        self.dfcx_moves.each_bit_on_entry(id, |index| {
            let move = self.move_data.moves.borrow();
            let move = move.get(index);
            let moved_path = move.path;
            if base_indices.iter().any(|x| x == &moved_path) {
                // Scenario 1 or 2: `loan_path` or some base path of
                // `loan_path` was moved.
                if !f(move, &*self.move_data.path_loan_path(moved_path)) {
                    ret = false;
                }
            } else {
                for &loan_path_index in opt_loan_path_index.iter() {
                    let cont = self.move_data.each_base_path(moved_path, |p| {
                        if p == loan_path_index {
                            // Scenario 3: some extension of `loan_path`
                            // was moved
                            f(move, &*self.move_data.path_loan_path(moved_path))
                        } else {
                            true
                        }
                    });
                    if !cont { ret = false; break }
                }
            }
            ret
        })
    }

    pub fn is_assignee(&self,
                       id: ast::NodeId)
                       -> bool {
        //! True if `id` is the id of the LHS of an assignment
        self.move_data.assignee_ids.borrow().iter().any(|x| x == &id)
    }

    pub fn each_assignment_of(&self,
                              id: ast::NodeId,
                              loan_path: &Rc<LoanPath>,
                              f: |&Assignment| -> bool)
                              -> bool {
        /*!
         * Iterates through every assignment to `loan_path` that
         * may have occurred on entry to `id`. `loan_path` must be
         * a single variable.
         */

        let loan_path_index = {
            match self.move_data.existing_move_path(loan_path) {
                Some(i) => i,
                None => {
                    // if there were any assignments, it'd have an index
                    return true;
                }
            }
        };

        self.dfcx_assign.each_bit_on_entry(id, |index| {
            let assignment = self.move_data.var_assignments.borrow();
            let assignment = assignment.get(index);
            if assignment.path == loan_path_index && !f(assignment) {
                false
            } else {
                true
            }
        })
    }
}

impl BitwiseOperator for MoveDataFlowOperator {
    #[inline]
    fn join(&self, succ: uint, pred: uint) -> uint {
        succ | pred // moves from both preds are in scope
    }
}

impl DataFlowOperator for MoveDataFlowOperator {
    #[inline]
    fn initial_value(&self) -> bool {
        false // no loans in scope by default
    }
}

impl BitwiseOperator for AssignDataFlowOperator {
    #[inline]
    fn join(&self, succ: uint, pred: uint) -> uint {
        succ | pred // moves from both preds are in scope
    }
}

impl DataFlowOperator for AssignDataFlowOperator {
    #[inline]
    fn initial_value(&self) -> bool {
        false // no assignments in scope by default
    }
}
