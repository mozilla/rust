// Copyright 2012-2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.


use rustc::ty::{self, TyCtxt};
use rustc::mir::*;
use rustc::util::nodemap::FxHashMap;
use rustc_data_structures::indexed_vec::{IndexVec};
use syntax_pos::{Span};

use std::fmt;
use std::ops::{Index, IndexMut};

use self::abs_domain::{AbstractElem, Lift};

mod abs_domain;

// This submodule holds some newtype'd Index wrappers that are using
// NonZero to ensure that Option<Index> occupies only a single word.
// They are in a submodule to impose privacy restrictions; namely, to
// ensure that other code does not accidentally access `index.0`
// (which is likely to yield a subtle off-by-one error).
pub(crate) mod indexes {
    use std::fmt;
    use core::nonzero::NonZero;
    use rustc_data_structures::indexed_vec::Idx;

    macro_rules! new_index {
        ($Index:ident, $debug_name:expr) => {
            #[derive(Copy, Clone, PartialEq, Eq, Hash)]
            pub struct $Index(NonZero<usize>);

            impl Idx for $Index {
                fn new(idx: usize) -> Self {
                    $Index(NonZero::new(idx + 1).unwrap())
                }
                fn index(self) -> usize {
                    self.0.get() - 1
                }
            }

            impl fmt::Debug for $Index {
                fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                    write!(fmt, "{}{}", $debug_name, self.index())
                }
            }
        }
    }

    /// Index into MovePathData.move_paths
    new_index!(MovePathIndex, "mp");

    /// Index into MoveData.moves.
    new_index!(MoveOutIndex, "mo");

    /// Index into Borrows.locations
    new_index!(BorrowIndex, "bw");
}

pub use self::indexes::MovePathIndex;
pub use self::indexes::MoveOutIndex;

impl MoveOutIndex {
    pub fn move_path_index(&self, move_data: &MoveData) -> MovePathIndex {
        move_data.moves[*self].path
    }
}

/// `MovePath` is a canonicalized representation of a path that is
/// moved or assigned to.
///
/// It follows a tree structure.
///
/// Given `struct X { m: M, n: N }` and `x: X`, moves like `drop x.m;`
/// move *out* of the l-value `x.m`.
///
/// The MovePaths representing `x.m` and `x.n` are siblings (that is,
/// one of them will link to the other via the `next_sibling` field,
/// and the other will have no entry in its `next_sibling` field), and
/// they both have the MovePath representing `x` as their parent.
#[derive(Clone)]
pub struct MovePath<'tcx> {
    pub next_sibling: Option<MovePathIndex>,
    pub first_child: Option<MovePathIndex>,
    pub parent: Option<MovePathIndex>,
    pub lvalue: Lvalue<'tcx>,
}

impl<'tcx> fmt::Debug for MovePath<'tcx> {
    fn fmt(&self, w: &mut fmt::Formatter) -> fmt::Result {
        write!(w, "MovePath {{")?;
        if let Some(parent) = self.parent {
            write!(w, " parent: {:?},", parent)?;
        }
        if let Some(first_child) = self.first_child {
            write!(w, " first_child: {:?},", first_child)?;
        }
        if let Some(next_sibling) = self.next_sibling {
            write!(w, " next_sibling: {:?}", next_sibling)?;
        }
        write!(w, " lvalue: {:?} }}", self.lvalue)
    }
}

impl<'tcx> fmt::Display for MovePath<'tcx> {
    fn fmt(&self, w: &mut fmt::Formatter) -> fmt::Result {
        write!(w, "{:?}", self.lvalue)
    }
}

#[derive(Debug)]
pub struct MoveData<'tcx> {
    pub move_paths: IndexVec<MovePathIndex, MovePath<'tcx>>,
    pub moves: IndexVec<MoveOutIndex, MoveOut>,
    /// Each Location `l` is mapped to the MoveOut's that are effects
    /// of executing the code at `l`. (There can be multiple MoveOut's
    /// for a given `l` because each MoveOut is associated with one
    /// particular path being moved.)
    pub loc_map: LocationMap<Vec<MoveOutIndex>>,
    pub path_map: IndexVec<MovePathIndex, Vec<MoveOutIndex>>,
    pub rev_lookup: MovePathLookup<'tcx>,
}

impl<'tcx> MoveData<'tcx> {
    pub fn all_paths_from<F>(&self, root: MovePathIndex, mut each_path: F)
        where F: FnMut(MovePathIndex) -> bool
    {
        fn all_paths_from<'tcx, F>(move_data: &MoveData<'tcx>,
        root: MovePathIndex, each_path: &mut F)
            where F: FnMut(MovePathIndex) -> bool
        {
            if !each_path(root) {
                return;
            }

            let mut next_child_index = move_data.move_paths[root].first_child;
            while let Some(child_index) = next_child_index {
                all_paths_from(move_data, child_index, each_path);
                next_child_index = move_data.move_paths[child_index].next_sibling;
            }
        }

        all_paths_from(self, root, &mut each_path);
    }
}

pub trait HasMoveData<'tcx> {
    fn move_data(&self) -> &MoveData<'tcx>;
}

#[derive(Debug)]
pub struct LocationMap<T> {
    /// Location-indexed (BasicBlock for outer index, index within BB
    /// for inner index) map.
    pub(crate) map: IndexVec<BasicBlock, Vec<T>>,
}

impl<T> Index<Location> for LocationMap<T> {
    type Output = T;
    fn index(&self, index: Location) -> &Self::Output {
        &self.map[index.block][index.statement_index]
    }
}

impl<T> IndexMut<Location> for LocationMap<T> {
    fn index_mut(&mut self, index: Location) -> &mut Self::Output {
        &mut self.map[index.block][index.statement_index]
    }
}

impl<T> LocationMap<T> where T: Default + Clone {
    fn new(mir: &Mir) -> Self {
        LocationMap {
            map: mir.basic_blocks().iter().map(|block| {
                vec![T::default(); block.statements.len()+1]
            }).collect()
        }
    }
}

/// `MoveOut` represents a point in a program that moves out of some
/// L-value; i.e., "creates" uninitialized memory.
///
/// With respect to dataflow analysis:
/// - Generated by moves and declaration of uninitialized variables.
/// - Killed by assignments to the memory.
#[derive(Copy, Clone)]
pub struct MoveOut {
    /// path being moved
    pub path: MovePathIndex,
    /// location of move
    pub source: Location,
}

impl fmt::Debug for MoveOut {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{:?}@{:?}", self.path, self.source)
    }
}

/// Tables mapping from an l-value to its MovePathIndex.
#[derive(Debug)]
pub struct MovePathLookup<'tcx> {
    locals: IndexVec<Local, MovePathIndex>,

    /// projections are made from a base-lvalue and a projection
    /// elem. The base-lvalue will have a unique MovePathIndex; we use
    /// the latter as the index into the outer vector (narrowing
    /// subsequent search so that it is solely relative to that
    /// base-lvalue). For the remaining lookup, we map the projection
    /// elem to the associated MovePathIndex.
    projections: FxHashMap<(MovePathIndex, AbstractElem<'tcx>), MovePathIndex>
}

mod builder;

#[derive(Copy, Clone, Debug)]
pub enum LookupResult {
    Exact(MovePathIndex),
    Parent(Option<MovePathIndex>)
}

impl<'tcx> MovePathLookup<'tcx> {
    // Unlike the builder `fn move_path_for` below, this lookup
    // alternative will *not* create a MovePath on the fly for an
    // unknown l-value, but will rather return the nearest available
    // parent.
    pub fn find(&self, lval: &Lvalue<'tcx>) -> LookupResult {
        match *lval {
            Lvalue::Local(local) => LookupResult::Exact(self.locals[local]),
            Lvalue::Static(..) => LookupResult::Parent(None),
            Lvalue::Projection(ref proj) => {
                match self.find(&proj.base) {
                    LookupResult::Exact(base_path) => {
                        match self.projections.get(&(base_path, proj.elem.lift())) {
                            Some(&subpath) => LookupResult::Exact(subpath),
                            None => LookupResult::Parent(Some(base_path))
                        }
                    }
                    inexact => inexact
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct IllegalMoveOrigin<'tcx> {
    pub(crate) span: Span,
    pub(crate) kind: IllegalMoveOriginKind<'tcx>,
}

#[derive(Debug)]
pub(crate) enum IllegalMoveOriginKind<'tcx> {
    Static,
    BorrowedContent,
    InteriorOfTypeWithDestructor { container_ty: ty::Ty<'tcx> },
    InteriorOfSliceOrArray { ty: ty::Ty<'tcx>, is_index: bool, },
}

#[derive(Debug)]
pub enum MoveError<'tcx> {
    IllegalMove { cannot_move_out_of: IllegalMoveOrigin<'tcx> },
    UnionMove { path: MovePathIndex },
}

impl<'tcx> MoveError<'tcx> {
    fn cannot_move_out_of(span: Span, kind: IllegalMoveOriginKind<'tcx>) -> Self {
        let origin = IllegalMoveOrigin { span, kind };
        MoveError::IllegalMove { cannot_move_out_of: origin }
    }
}

impl<'a, 'gcx, 'tcx> MoveData<'tcx> {
    pub fn gather_moves(mir: &Mir<'tcx>,
                        tcx: TyCtxt<'a, 'gcx, 'tcx>,
                        param_env: ty::ParamEnv<'gcx>,
                        move_check: bool)
                        -> Result<Self, (Self, Vec<MoveError<'tcx>>)> {
        builder::gather_moves(mir, tcx, param_env, move_check)
    }
}
