// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Translation Item Collection
//! ===========================
//!
//! This module is responsible for discovering all items that will contribute to
//! to code generation of the crate. The important part here is that it not only
//! needs to find syntax-level items (functions, structs, etc) but also all
//! their monomorphized instantiations. Every non-generic, non-const function
//! maps to one LLVM artifact. Every generic function can produce
//! from zero to N artifacts, depending on the sets of type arguments it
//! is instantiated with.
//! This also applies to generic items from other crates: A generic definition
//! in crate X might produce monomorphizations that are compiled into crate Y.
//! We also have to collect these here.
//!
//! The following kinds of "translation items" are handled here:
//!
//! - Functions
//! - Methods
//! - Closures
//! - Statics
//! - Drop glue
//!
//! The following things also result in LLVM artifacts, but are not collected
//! here, since we instantiate them locally on demand when needed in a given
//! codegen unit:
//!
//! - Constants
//! - Vtables
//! - Object Shims
//!
//!
//! General Algorithm
//! -----------------
//! Let's define some terms first:
//!
//! - A "translation item" is something that results in a function or global in
//!   the LLVM IR of a codegen unit. Translation items do not stand on their
//!   own, they can reference other translation items. For example, if function
//!   `foo()` calls function `bar()` then the translation item for `foo()`
//!   references the translation item for function `bar()`. In general, the
//!   definition for translation item A referencing a translation item B is that
//!   the LLVM artifact produced for A references the LLVM artifact produced
//!   for B.
//!
//! - Translation items and the references between them for a directed graph,
//!   where the translation items are the nodes and references form the edges.
//!   Let's call this graph the "translation item graph".
//!
//! - The translation item graph for a program contains all translation items
//!   that are needed in order to produce the complete LLVM IR of the program.
//!
//! The purpose of the algorithm implemented in this module is to build the
//! translation item graph for the current crate. It runs in two phases:
//!
//! 1. Discover the roots of the graph by traversing the HIR of the crate.
//! 2. Starting from the roots, find neighboring nodes by inspecting the MIR
//!    representation of the item corresponding to a given node, until no more
//!    new nodes are found.
//!
//! ### Discovering roots
//!
//! The roots of the translation item graph correspond to the non-generic
//! syntactic items in the source code. We find them by walking the HIR of the
//! crate, and whenever we hit upon a function, method, or static item, we
//! create a translation item consisting of the items DefId and, since we only
//! consider non-generic items, an empty type-substitution set.
//!
//! ### Finding neighbor nodes
//! Given a translation item node, we can discover neighbors by inspecting its
//! MIR. We walk the MIR and any time we hit upon something that signifies a
//! reference to another translation item, we have found a neighbor. Since the
//! translation item we are currently at is always monomorphic, we also know the
//! concrete type arguments of its neighbors, and so all neighbors again will be
//! monomorphic. The specific forms a reference to a neighboring node can take
//! in MIR are quite diverse. Here is an overview:
//!
//! #### Calling Functions/Methods
//! The most obvious form of one translation item referencing another is a
//! function or method call (represented by a CALL terminator in MIR). But
//! calls are not the only thing that might introduce a reference between two
//! function translation items, and as we will see below, they are just a
//! specialized of the form described next, and consequently will don't get any
//! special treatment in the algorithm.
//!
//! #### Taking a reference to a function or method
//! A function does not need to actually be called in order to be a neighbor of
//! another function. It suffices to just take a reference in order to introduce
//! an edge. Consider the following example:
//!
//! ```rust
//! fn print_val<T: Display>(x: T) {
//!     println!("{}", x);
//! }
//!
//! fn call_fn(f: &Fn(i32), x: i32) {
//!     f(x);
//! }
//!
//! fn main() {
//!     let print_i32 = print_val::<i32>;
//!     call_fn(&print_i32, 0);
//! }
//! ```
//! The MIR of none of these functions will contain an explicit call to
//! `print_val::<i32>`. Nonetheless, in order to translate this program, we need
//! an instance of this function. Thus, whenever we encounter a function or
//! method in operand position, we treat it as a neighbor of the current
//! translation item. Calls are just a special case of that.
//!
//! #### Closures
//! In a way, closures are a simple case. Since every closure object needs to be
//! constructed somewhere, we can reliably discover them by observing
//! `RValue::Aggregate` expressions with `AggregateKind::Closure`. This is also
//! true for closures inlined from other crates.
//!
//! #### Drop glue
//! Drop glue translation items are introduced by MIR drop-statements. The
//! generated translation item will again have drop-glue item neighbors if the
//! type to be dropped contains nested values that also need to be dropped. It
//! might also have a function item neighbor for the explicit `Drop::drop`
//! implementation of its type.
//!
//! #### Unsizing Casts
//! A subtle way of introducing neighbor edges is by casting to a trait object.
//! Since the resulting fat-pointer contains a reference to a vtable, we need to
//! instantiate all object-save methods of the trait, as we need to store
//! pointers to these functions even if they never get called anywhere. This can
//! be seen as a special case of taking a function reference.
//!
//! #### Boxes
//! Since `Box` expression have special compiler support, no explicit calls to
//! `exchange_malloc()` and `exchange_free()` may show up in MIR, even if the
//! compiler will generate them. We have to observe `Rvalue::Box` expressions
//! and Box-typed drop-statements for that purpose.
//!
//!
//! Interaction with Cross-Crate Inlining
//! -------------------------------------
//! The binary of a crate will not only contain machine code for the items
//! defined in the source code of that crate. It will also contain monomorphic
//! instantiations of any extern generic functions and of functions marked with
//! #[inline].
//! The collection algorithm handles this more or less transparently. When
//! constructing a neighbor node for an item, the algorithm will always call
//! `inline::get_local_instance()` before proceeding. If no local instance can
//! be acquired (e.g. for a function that is just linked to) no node is created;
//! which is exactly what we want, since no machine code should be generated in
//! the current crate for such an item. On the other hand, if we can
//! successfully inline the function, we subsequently can just treat it like a
//! local item, walking it's MIR et cetera.
//!
//!
//! Eager and Lazy Collection Mode
//! ------------------------------
//! Translation item collection can be performed in one of two modes:
//!
//! - Lazy mode means that items will only be instantiated when actually
//!   referenced. The goal is to produce the least amount of machine code
//!   possible.
//!
//! - Eager mode is meant to be used in conjunction with incremental compilation
//!   where a stable set of translation items is more important than a minimal
//!   one. Thus, eager mode will instantiate drop-glue for every drop-able type
//!   in the crate, even of no drop call for that type exists (yet). It will
//!   also instantiate default implementations of trait methods, something that
//!   otherwise is only done on demand.
//!
//!
//! Open Issues
//! -----------
//! Some things are not yet fully implemented in the current version of this
//! module.
//!
//! ### Initializers of Constants and Statics
//! Since no MIR is constructed yet for initializer expressions of constants and
//! statics we cannot inspect these properly.
//!
//! ### Const Fns
//! Ideally, no translation item should be generated for const fns unless there
//! is a call to them that cannot be evaluated at compile time. At the moment
//! this is not implemented however: a translation item will be produced
//! regardless of whether it is actually needed or not.

use rustc_front::hir;
use rustc_front::intravisit as hir_visit;

use rustc::front::map as hir_map;
use rustc::middle::def_id::DefId;
use rustc::middle::lang_items::{ExchangeFreeFnLangItem, ExchangeMallocFnLangItem};
use rustc::middle::{ty, traits};
use rustc::middle::subst::{self, Substs, Subst};
use rustc::middle::ty::adjustment::CustomCoerceUnsized;
use rustc::middle::ty::fold::TypeFoldable;
use rustc::mir::repr as mir;
use rustc::mir::visit as mir_visit;
use rustc::mir::visit::Visitor as MirVisitor;

use syntax::ast::{self, NodeId};
use syntax::codemap::DUMMY_SP;
use syntax::errors;
use syntax::parse::token;

use trans::adt;
use trans::context::CrateContext;
use trans::common::{fulfill_obligation, normalize_and_test_predicates,
                    type_is_sized};
use trans::glue;
use trans::meth;
use trans::monomorphize;
use trans::inline;
use util::nodemap::{FnvHashSet, FnvHashMap};

use std::hash::{Hash, Hasher};

#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
pub enum TransItemCollectionMode {
    Eager,
    Lazy
}

#[derive(Eq, Clone, Copy, Debug)]
pub enum TransItem<'tcx> {
    DropGlue(ty::Ty<'tcx>),
    Fn {
        node_id: NodeId,
        substs: &'tcx Substs<'tcx>,
        original_definition: DefId
    },
    Static(NodeId)
}

impl<'tcx> Hash for TransItem<'tcx> {
    fn hash<H: Hasher>(&self, s: &mut H) {
        match *self {
            TransItem::DropGlue(t) => {
                0u8.hash(s);
                t.hash(s);
            },
            TransItem::Fn { node_id, substs, .. } => {
                1u8.hash(s);
                node_id.hash(s);
                (substs as *const Substs<'tcx> as usize).hash(s);
                // ignore original_definition
            }
            TransItem::Static(node_id) => {
                3u8.hash(s);
                node_id.hash(s);
            }
        };
    }
}

impl<'tcx> PartialEq for TransItem<'tcx> {
    fn eq(&self, other: &Self) -> bool {
        match (*self, *other) {
            (TransItem::DropGlue(t1), TransItem::DropGlue(t2)) => t1 == t2,
            (TransItem::Fn { node_id: node_id1, substs: substs1, .. },
             TransItem::Fn { node_id: node_id2, substs: substs2, .. }) => {
                // anything but node_id and substs is ignored, since it's only
                // bookkeeping data
                node_id1 == node_id2 && substs1 == substs2
            },
            (TransItem::Static(node_id1), TransItem::Static(node_id2)) => {
                node_id1 == node_id2
            },
            _ => false
        }
    }
}

pub fn collect_crate_translation_items<'a, 'tcx>(ccx: &CrateContext<'a, 'tcx>,
                                                 mode: TransItemCollectionMode)
                                                 -> FnvHashSet<TransItem<'tcx>> {
    let roots = collect_roots(ccx, mode);

    debug!("Building translation item graph, beginning at roots");
    let mut visited = FnvHashSet();

    for root in roots {
        collect_items_rec(ccx, root, &mut visited);
    }

    return visited;
}

// Find all non-generic items by walking the HIR. These items serve as roots to
// start monomorphizing from.
fn collect_roots<'a, 'tcx>(ccx: &CrateContext<'a, 'tcx>,
                           mode: TransItemCollectionMode)
                           -> Vec<TransItem<'tcx>> {
    debug!("Collecting roots");
    let mut roots = Vec::new();

    {
        let mut visitor = RootCollector {
            ccx: ccx,
            mode: mode,
            output: &mut roots,
            enclosing_item: None,
            trans_empty_substs: ccx.tcx().mk_substs(Substs::trans_empty()),
        };

        ccx.tcx().map.krate().visit_all_items(&mut visitor);
    }

    roots
}

// Collect all monomorphized translation items reachable from `starting_point`
fn collect_items_rec<'a, 'tcx: 'a>(ccx: &CrateContext<'a, 'tcx>,
                                   starting_point: TransItem<'tcx>,
                                   visited: &mut FnvHashSet<TransItem<'tcx>>) {
    if !visited.insert(starting_point.clone()) {
        // We've been here already, no need to search again.
        return;
    }
    debug!("BEGIN collect_items_rec({})", starting_point.to_string(ccx));

    let mut neighbors = Vec::new();

    match starting_point {
        TransItem::DropGlue(_) |
        TransItem::Static(_) => {}
        TransItem::Fn { node_id, substs: ref param_substs, .. } => {
            // Scan the MIR in order to find function calls, closures,
            // and drop-glue
            let mir_not_found_error_message = || {
                format!("Could not find MIR for node_id: {}",
                        ccx.tcx().map.node_to_string(node_id))
            };

            let fn_ty = ccx.tcx().node_id_to_type(node_id);

            let external_mir = if let ty::TyClosure(def_id, _) = fn_ty.sty {
                if !def_id.is_local() {
                    ccx.sess()
                       .cstore
                       .maybe_get_item_mir(ccx.tcx(), def_id)
                } else {
                    None
                }
            } else {
                ccx.external_srcs()
                   .borrow()
                   .get(&node_id)
                   .map(|did| ccx.sess().cstore.maybe_get_item_mir(ccx.tcx(), *did))
                   .unwrap_or(None)
            };

            let mir_opt = match external_mir {
                Some(ref mir) => {
                    Some(mir)
                }
                None => {
                    // No external MIR found, it must be in the local MIR map.
                    ccx.mir_map().get(&node_id)
                }
            };

            let mir = errors::expect(ccx.sess().diagnostic(),
                                     mir_opt,
                                     mir_not_found_error_message);

            let mut visitor = MirNeighborCollector {
                ccx: ccx,
                mir: mir,
                output: &mut neighbors,
                param_substs: param_substs
            };

            visitor.visit_mir(mir);
        }
    }

    for neighbour in neighbors {
        collect_items_rec(ccx, neighbour, visited);
    }

    debug!("END collect_items_rec({})", starting_point.to_string(ccx));
}

struct MirNeighborCollector<'a, 'tcx: 'a> {
    ccx: &'a CrateContext<'a, 'tcx>,
    mir: &'a mir::Mir<'tcx>,
    output: &'a mut Vec<TransItem<'tcx>>,
    param_substs: &'tcx Substs<'tcx>
}

impl<'a, 'tcx> MirVisitor<'tcx> for MirNeighborCollector<'a, 'tcx> {

    fn visit_rvalue(&mut self, rvalue: &mir::Rvalue<'tcx>) {
        debug!("visiting rvalue {:?}", *rvalue);

        match *rvalue {
            mir::Rvalue::Aggregate(mir::AggregateKind::Closure(def_id,
                                                               ref substs), _) => {
                let node_id = if def_id.is_local() {
                    self.ccx.tcx().map.as_local_node_id(def_id).unwrap()
                } else {
                    self.ccx.tcx().inlined_closures.borrow()[&def_id]
                };

                let trans_item = create_trans_item_for_local_fn(self.ccx,
                                                                node_id,
                                                                substs.func_substs,
                                                                self.param_substs,
                                                                def_id);
                self.output.push(trans_item);
            }
            // When doing an cast from a regular pointer to a fat pointer, we
            // have to instantiate all methods of the trait being cast to, so we
            // can build the appropriate vtable.
            mir::Rvalue::Cast(mir::CastKind::Unsize, ref operand, target_ty) => {
                let target_ty = monomorphize::apply_param_substs(self.ccx.tcx(),
                                                                 self.param_substs,
                                                                 &target_ty);
                let source_ty = self.mir.operand_ty(self.ccx.tcx(), operand);
                let source_ty = monomorphize::apply_param_substs(self.ccx.tcx(),
                                                                 self.param_substs,
                                                                 &source_ty);
                let (source_ty, target_ty) = find_vtable_types_for_unsizing(self.ccx,
                                                                            source_ty,
                                                                            target_ty);
                // This could also be a different Unsize instruction, like
                // from a fixed sized array to a slice. But we are only
                // interested in things that produce a vtable.
                if target_ty.is_trait() && !source_ty.is_trait() {
                    create_trans_items_for_vtable_methods(self.ccx,
                                                          target_ty,
                                                          source_ty,
                                                          self.output);
                }
            }
            mir::Rvalue::Box(_) => {
                let exchange_malloc_fn_def_id =
                    self.ccx
                        .tcx()
                        .lang_items
                        .require(ExchangeMallocFnLangItem)
                        .expect("Could not find ExchangeMallocFnLangItem");

                with_local_instance_of(exchange_malloc_fn_def_id, self.ccx, |node_id| {
                    let exchange_malloc_fn_trans_item =
                        create_trans_item_for_local_fn(self.ccx,
                                                       node_id,
                                                       &Substs::trans_empty(),
                                                       self.param_substs,
                                                       exchange_malloc_fn_def_id);

                    self.output.push(exchange_malloc_fn_trans_item);
                });
            }
            _ => { /* not interesting */ }
        }

        self.super_rvalue(rvalue);
    }

    fn visit_lvalue(&mut self,
                    lvalue: &mir::Lvalue<'tcx>,
                    context: mir_visit::LvalueContext) {
        debug!("visiting lvalue {:?}", *lvalue);

        if let mir_visit::LvalueContext::Drop = context {
            let ty = self.mir.lvalue_ty(self.ccx.tcx(), lvalue)
                             .to_ty(self.ccx.tcx());

            let ty = monomorphize::apply_param_substs(self.ccx.tcx(),
                                                      self.param_substs,
                                                      &ty);
            let ty = self.ccx.tcx().erase_regions(&ty);

            create_drop_glue_trans_items(self.ccx,
                                         ty,
                                         self.param_substs,
                                         &mut self.output);
        }

        self.super_lvalue(lvalue, context);
    }

    fn visit_operand(&mut self, operand: &mir::Operand<'tcx>) {
        debug!("visiting operand {:?}", *operand);

        let callee = match *operand {
            mir::Operand::Constant(mir::Constant {
                literal: mir::Literal::Item {
                    def_id,
                    kind,
                    substs
                },
                ..
            }) if is_function_or_method(kind) => Some((def_id, substs)),
            _ => None
        };

        if let Some((callee_def_id, callee_substs)) = callee {
            debug!(" => operand is callable");

            // `callee_def_id` might refer to a trait method instead of a
            // concrete implementation, so we have to find the actual
            // implementation. For example, the call might look like
            //
            // std::cmp::partial_cmp(0i32, 1i32)
            //
            // Calling do_static_dispatch() here will map the def_id of
            // `std::cmp::partial_cmp` to the def_id of `i32::partial_cmp<i32>`
            let dispatched = do_static_dispatch(self.ccx,
                                                callee_def_id,
                                                callee_substs,
                                                self.param_substs);

            if let Some((callee_def_id, callee_substs)) = dispatched {
                // if we have a concrete impl (which we might not have
                // in the case of something compiler generated like an
                // object shim or a closure that is handled differently),
                // we try to inline the callee
                with_local_instance_of(callee_def_id, self.ccx, |node_id| {
                    if can_result_in_trans_item(self.ccx, node_id) {
                        let trans_item = create_trans_item_for_local_fn(self.ccx,
                                                                        node_id,
                                                                        callee_substs,
                                                                        self.param_substs,
                                                                        callee_def_id);
                        self.output.push(trans_item);
                    }
                });
            }
        }

        self.super_operand(operand);

        fn is_function_or_method(item_kind: mir::ItemKind) -> bool {
            match item_kind {
                mir::ItemKind::Constant => false,
                mir::ItemKind::Function |
                mir::ItemKind::Method   => true
            }
        }

        fn can_result_in_trans_item<'a, 'tcx>(ccx: &CrateContext<'a, 'tcx>,
                                              node_id: NodeId) -> bool {
            match ccx.tcx().map.get(node_id) {
                hir_map::NodeItem(&hir::Item { node: hir::ItemFn(..), .. } )                      |
                hir_map::NodeTraitItem(&hir::TraitItem { node: hir::MethodTraitItem(..), .. })    |
                hir_map::NodeImplItem(&hir::ImplItem { node: hir::ImplItemKind::Method(..), .. }) |
                hir_map::NodeExpr(&hir::Expr { node: hir::ExprClosure(..), .. }) => true,
                _ => false
            }
        }
    }
}

fn create_drop_glue_trans_items<'a, 'tcx>(ccx: &CrateContext<'a, 'tcx>,
                                          mono_ty: ty::Ty<'tcx>,
                                          param_substs: &'tcx Substs<'tcx>,
                                          output: &mut Vec<TransItem<'tcx>>)
{
    visit_types_of_owned_components(ccx,
                                    mono_ty,
                                    &mut FnvHashSet(),
                                    &mut |ty| {
        debug!("create_drop_glue_trans_items: {}", type_to_string(ccx, ty));
        // Add a translation item for the drop glue, if even this type does not
        // need to be dropped (in which case it has been mapped to i8)
        output.push(TransItem::DropGlue(ty));

        if glue::type_needs_drop(ccx.tcx(), ty) {

            // Make sure the exchange_free_fn() lang-item gets translated if
            // there is a boxed value.
            if let ty::TyBox(_) = ty.sty {

                let exchange_free_fn_def_id = ccx.tcx()
                                                 .lang_items
                                                 .require(ExchangeFreeFnLangItem)
                                                 .expect("Could not find ExchangeFreeFnLangItem");

                with_local_instance_of(exchange_free_fn_def_id, ccx, |node_id| {
                    let exchange_free_fn_trans_item =
                        create_trans_item_for_local_fn(ccx,
                                                       node_id,
                                                       &Substs::trans_empty(),
                                                       param_substs,
                                                       exchange_free_fn_def_id);

                    output.push(exchange_free_fn_trans_item);
                });
            }

            // If the type implements Drop, also add a translation item for the
            // monomorphized Drop::drop() implementation.
            let destructor_did = match ty.sty {
                ty::TyStruct(def, _) |
                ty::TyEnum(def, _)   => def.destructor(),
                _ => None
            };

            if let Some(destructor_did) = destructor_did {
                use rustc::middle::ty::ToPolyTraitRef;

                let drop_trait_def_id = ccx.tcx()
                                           .lang_items
                                           .drop_trait()
                                           .unwrap();

                with_local_instance_of(destructor_did, ccx, |node_id| {
                    let self_type_substs = ccx.tcx().mk_substs(
                        Substs::trans_empty().with_self_ty(ty));

                    let trait_ref = ty::TraitRef {
                        def_id: drop_trait_def_id,
                        substs: self_type_substs,
                    }.to_poly_trait_ref();

                    let substs = match fulfill_obligation(ccx, DUMMY_SP, trait_ref) {
                        traits::VtableImpl(data) => data.substs,
                        _ => unreachable!()
                    };

                    let cg_item = create_trans_item_for_local_fn(ccx,
                                                                 node_id,
                                                                 ccx.tcx().mk_substs(substs),
                                                                 param_substs,
                                                                 destructor_did);
                    output.push(cg_item);
                });
            }

            true
        } else {
            false
        }
    });

    fn visit_types_of_owned_components<'a, 'tcx, F>(ccx: &CrateContext<'a, 'tcx>,
                                                    ty: ty::Ty<'tcx>,
                                                    visited: &mut FnvHashSet<ty::Ty<'tcx>>,
                                                    mut f: &mut F)
        where F: FnMut(ty::Ty<'tcx>) -> bool
    {
        let ty = glue::get_drop_glue_type(ccx, ty);

        if !visited.insert(ty) {
            return;
        }

        if !f(ty) {
            // Don't recurse further
            return;
        }

        match ty.sty {
            ty::TyBool       |
            ty::TyChar       |
            ty::TyInt(_)     |
            ty::TyUint(_)    |
            ty::TyStr        |
            ty::TyFloat(_)   |
            ty::TyRawPtr(_)  |
            ty::TyRef(..)    |
            ty::TyBareFn(..) |
            ty::TySlice(_)   |
            ty::TyTrait(_)   => {
                /* nothing to do */
            }
            ty::TyStruct(ref adt_def, substs) |
            ty::TyEnum(ref adt_def, substs) => {
                for field in adt_def.all_fields() {
                    let field_type = monomorphize::apply_param_substs(ccx.tcx(),
                                                                      substs,
                                                                      &field.unsubst_ty());
                    visit_types_of_owned_components(ccx, field_type, visited, f);
                }
            }
            ty::TyClosure(_, ref substs) => {
                for upvar_ty in &substs.upvar_tys {
                    visit_types_of_owned_components(ccx, upvar_ty, visited, f);
                }
            }
            ty::TyBox(inner_type)      |
            ty::TyArray(inner_type, _) => {
                visit_types_of_owned_components(ccx, inner_type, visited, f);
            }
            ty::TyTuple(ref args) => {
                for arg in args {
                    visit_types_of_owned_components(ccx, arg, visited, f);
                }
            }
            ty::TyProjection(_) |
            ty::TyParam(_)      |
            ty::TyInfer(_)      |
            ty::TyError         => {
                ccx.sess().bug("encountered unexpected type");
            }
        }
    }
}

fn do_static_dispatch<'a, 'tcx>(ccx: &CrateContext<'a, 'tcx>,
                                fn_def_id: DefId,
                                fn_substs: &'tcx Substs<'tcx>,
                                param_substs: &'tcx Substs<'tcx>)
                                -> Option<(DefId, &'tcx Substs<'tcx>)> {
    debug!("do_static_dispatch(fn_def_id={}, fn_substs={:?}, param_substs={:?})",
           def_id_to_string(ccx, fn_def_id, None),
           fn_substs,
           param_substs);

    let is_trait_method = ccx.tcx().trait_of_item(fn_def_id).is_some();

    if is_trait_method {
        match ccx.tcx().impl_or_trait_item(fn_def_id) {
            ty::MethodTraitItem(ref method) => {
                match method.container {
                    ty::TraitContainer(trait_def_id) => {
                        debug!(" => trait method, attempting to find impl");
                        do_static_trait_method_dispatch(ccx,
                                                        method,
                                                        trait_def_id,
                                                        fn_substs,
                                                        param_substs)
                    }
                    ty::ImplContainer(_) => {
                        // This is already a concrete implementation
                        debug!(" => impl method");
                        Some((fn_def_id, fn_substs))
                    }
                }
            }
            _ => unreachable!()
        }
    } else {
        debug!(" => regular function");
        // The function is not part of an impl or trait, no dispatching
        // to be done
        Some((fn_def_id, fn_substs))
    }
}

// Given a trait-method and substitution information, find out the actual
// implementation of the trait method.
// Note: This code duplicates quite a bit of logic from
//       trans::meth::trans_static_method_callee(). Maybe later, this could be
//       refactored so that this method stores the computed dispatching
//       information somewhere so it can easily be looked up later.
fn do_static_trait_method_dispatch<'a, 'tcx>(ccx: &CrateContext<'a, 'tcx>,
                                             trait_method: &ty::Method,
                                             trait_id: DefId,
                                             callee_substs: &'tcx Substs<'tcx>,
                                             param_substs: &'tcx Substs<'tcx>)
                                             -> Option<(DefId, &'tcx Substs<'tcx>)> {
    let tcx = ccx.tcx();

    debug!("do_static_trait_method_dispatch(trait_method={}, \
                                            trait_id={}, \
                                            callee_substs={:?}, \
                                            param_substs={:?}",
           def_id_to_string(ccx, trait_method.def_id, None),
           def_id_to_string(ccx, trait_id, None),
           callee_substs,
           param_substs);

    // Find the substitutions for the fn itself. This includes
    // type parameters that belong to the trait but also some that
    // belong to the method:
    let rcvr_substs = monomorphize::apply_param_substs(tcx,
                                                       param_substs,
                                                       callee_substs);
    let subst::SeparateVecsPerParamSpace {
        types: rcvr_type,
        selfs: rcvr_self,
        fns: rcvr_method
    } = rcvr_substs.types.split();

    // Lookup the precise impl being called. To do that, we need to
    // create a trait reference identifying the self type and other
    // input type parameters. To create that trait reference, we have
    // to pick apart the type parameters to identify just those that
    // pertain to the trait. This is easiest to explain by example:
    //
    //     trait Convert {
    //         fn from<U:Foo>(n: U) -> Option<Self>;
    //     }
    //     ...
    //     let f = <Vec<int> as Convert>::from::<String>(...)
    //
    // Here, in this call, which I've written with explicit UFCS
    // notation, the set of type parameters will be:
    //
    //     rcvr_type: [] <-- nothing declared on the trait itself
    //     rcvr_self: [Vec<int>] <-- the self type
    //     rcvr_method: [String] <-- method type parameter
    //
    // So we create a trait reference using the first two,
    // basically corresponding to `<Vec<int> as Convert>`.
    // The remaining type parameters (`rcvr_method`) will be used below.
    let trait_substs = Substs::erased(subst::VecPerParamSpace::new(rcvr_type,
                                                                   rcvr_self,
                                                                   Vec::new()));
    let trait_substs = tcx.mk_substs(trait_substs);
    debug!(" => trait_substs={:?}", trait_substs);
    let trait_ref = ty::Binder(ty::TraitRef::new(trait_id, trait_substs));
    let vtbl = fulfill_obligation(ccx, DUMMY_SP, trait_ref);

    // Now that we know which impl is being used, we can dispatch to
    // the actual function:
    match vtbl {
        traits::VtableImpl(traits::VtableImplData {
            impl_def_id: impl_did,
            substs: impl_substs,
            nested: _ }) =>
        {
            assert!(!impl_substs.types.needs_infer());

            // Create the substitutions that are in scope. This combines
            // the type parameters from the impl with those declared earlier.
            // To see what I mean, consider a possible impl:
            //
            //    impl<T> Convert for Vec<T> {
            //        fn from<U:Foo>(n: U) { ... }
            //    }
            //
            // Recall that we matched `<Vec<int> as Convert>`. Trait
            // resolution will have given us a substitution
            // containing `impl_substs=[[T=int],[],[]]` (the type
            // parameters defined on the impl). We combine
            // that with the `rcvr_method` from before, which tells us
            // the type parameters from the *method*, to yield
            // `callee_substs=[[T=int],[],[U=String]]`.
            let subst::SeparateVecsPerParamSpace {
                types: impl_type,
                selfs: impl_self,
                fns: _
            } = impl_substs.types.split();
            let callee_substs =
                Substs::erased(subst::VecPerParamSpace::new(impl_type,
                                                            impl_self,
                                                            rcvr_method));
            let impl_method = tcx.get_impl_method(impl_did,
                                                  callee_substs,
                                                  trait_method.name);

            Some((impl_method.method.def_id, tcx.mk_substs(impl_method.substs)))
        }
        // If we have a closure or a function pointer, we will also encounter
        // the concrete closure/function somewhere else (during closure or fn
        // pointer construction). That's where we track those things.
        traits::VtableClosure(..) |
        traits::VtableFnPointer(..) |
        traits::VtableObject(..) => {
            None
        }
        _ => {
            tcx.sess.bug(&format!("static call to invalid vtable: {:?}", vtbl));
        }
    }
}

/// For given pair of source and target type that occur in an unsizing coercion,
/// this function finds the pair of types that determines the vtable linking
/// them.
///
/// For example, the source type might be `&SomeStruct` and the target type\
/// might be `&SomeTrait` in a cast like:
///
/// let src: &SomeStruct = ...;
/// let target = src as &SomeTrait;
///
/// Then the output of this function would be (SomeStruct, SomeTrait) since for
/// constructing the `target` fat-pointer we need the vtable for that pair.
///
/// Things can get more complicated though because there's also the case where
/// the unsized type occurs as a field:
///
/// struct ComplexStruct<T: ?Sized> {
///    a: u32,
///    b: f64,
///    c: T
/// }
///
/// In this case, if `T` is sized, `&SomeStruct` is a thin pointer. If `T` is
/// unsized, `&SomeStruct` is a fat pointer, and the vtable it points to is for
/// the pair of `T` (which is a trait) and the concrete type that `T` was
/// originally coerced from:
///
/// let src: &ComplexStruct<SomeStruct> = ...;
/// let target = src as &ComplexStruct<SomeTrait>;
///
/// Again, we want this `find_vtable_types_for_unsizing()` to provide the pair
/// `(SomeStruct, SomeTrait)`.
///
/// Finally, there is also the case of custom unsizing coercions, e.g. for
/// smart pointers such as `Rc` and `Arc`.
fn find_vtable_types_for_unsizing<'a, 'tcx>(ccx: &CrateContext<'a, 'tcx>,
                                            source_ty: ty::Ty<'tcx>,
                                            target_ty: ty::Ty<'tcx>)
                                            -> (ty::Ty<'tcx>, ty::Ty<'tcx>) {
    match (&source_ty.sty, &target_ty.sty) {
        (&ty::TyBox(a), &ty::TyBox(b)) |
        (&ty::TyRef(_, ty::TypeAndMut { ty: a, .. }),
         &ty::TyRef(_, ty::TypeAndMut { ty: b, .. })) |
        (&ty::TyRef(_, ty::TypeAndMut { ty: a, .. }),
         &ty::TyRawPtr(ty::TypeAndMut { ty: b, .. })) |
        (&ty::TyRawPtr(ty::TypeAndMut { ty: a, .. }),
         &ty::TyRawPtr(ty::TypeAndMut { ty: b, .. })) => {
            let (inner_source, inner_target) = (a, b);

            if !type_is_sized(ccx.tcx(), inner_source) {
                (inner_source, inner_target)
            } else {
                ccx.tcx().struct_lockstep_tails(inner_source, inner_target)
            }
        }

        (&ty::TyStruct(def_id_a, _), &ty::TyStruct(def_id_b, _)) => {
            assert_eq!(def_id_a, def_id_b);

            let trait_substs = Substs::erased(
                subst::VecPerParamSpace::new(vec![target_ty],
                                             vec![source_ty],
                                             Vec::new()));

            let trait_ref = ty::Binder(ty::TraitRef {
                def_id: ccx.tcx().lang_items.coerce_unsized_trait().unwrap(),
                substs: ccx.tcx().mk_substs(trait_substs)
            });

            let kind = match fulfill_obligation(ccx, DUMMY_SP, trait_ref) {
                traits::VtableImpl(traits::VtableImplData { impl_def_id, .. }) => {
                    ccx.tcx().custom_coerce_unsized_kind(impl_def_id)
                }
                vtable => {
                    ccx.sess().bug(&format!("invalid CoerceUnsized vtable: {:?}",
                                            vtable));
                }
            };

            let repr_source = adt::represent_type(ccx, source_ty);
            let src_fields = match &*repr_source {
                &adt::Repr::Univariant(ref s, _) => &s.fields,
                _ => ccx.sess().bug(&format!("Non univariant struct? (repr_source: {:?})",
                                              repr_source)),
            };
            let repr_target = adt::represent_type(ccx, target_ty);
            let target_fields = match &*repr_target {
                &adt::Repr::Univariant(ref s, _) => &s.fields,
                _ => ccx.sess().bug(&format!("Non univariant struct? (repr_target: {:?})",
                                              repr_target)),
            };

            let coerce_index = match kind {
                CustomCoerceUnsized::Struct(i) => i
            };
            assert!(coerce_index < src_fields.len() &&
                    src_fields.len() == target_fields.len());

            find_vtable_types_for_unsizing(ccx,
                                           src_fields[coerce_index],
                                           target_fields[coerce_index])
        }
        _ => ccx.sess()
                .bug(&format!("find_vtable_types_for_unsizing: invalid coercion {:?} -> {:?}",
                               source_ty,
                               target_ty))
    }
}

fn create_trans_item_for_local_fn<'a, 'tcx>(ccx: &CrateContext<'a, 'tcx>,
                                            node_id: NodeId,
                                            fn_substs: &Substs<'tcx>,
                                            param_substs: &Substs<'tcx>,
                                            original_definition: DefId)
                                            -> TransItem<'tcx>
{
    debug!("create_trans_item_for_local_fn(node_id={}, \
                                           fn_substs={:?}, \
                                           param_substs={:?}, \
                                           original_definition: {})",
            def_id_to_string(ccx, ccx.tcx().map.local_def_id(node_id), None),
            fn_substs,
            param_substs,
            def_id_to_string(ccx, original_definition, None));

    // We only get here, if fn_def_id either designates a local item or
    // an inlineable external item. Non-inlineable external items are
    // ignored because we don't want to generate any code for them.
    let concrete_substs = monomorphize::apply_param_substs(ccx.tcx(),
                                                           param_substs,
                                                           fn_substs);
    let concrete_substs = ccx.tcx().erase_regions(&concrete_substs);

    let trans_item = TransItem::Fn {
        node_id: node_id,
        substs: ccx.tcx().mk_substs(concrete_substs),
        original_definition: original_definition
    };

    return trans_item;
}

/// Creates a `TransItem` for each method that is referenced by the vtable for
/// the given trait/impl pair.
fn create_trans_items_for_vtable_methods<'a, 'tcx>(ccx: &CrateContext<'a, 'tcx>,
                                                   trait_ty: ty::Ty<'tcx>,
                                                   impl_ty: ty::Ty<'tcx>,
                                                   output: &mut Vec<TransItem<'tcx>>) {
    assert!(!trait_ty.needs_subst() && !impl_ty.needs_subst());

    if let ty::TyTrait(ref trait_ty) = trait_ty.sty {
        let poly_trait_ref = trait_ty.principal_trait_ref_with_self_ty(ccx.tcx(),
                                                                       impl_ty);

        // Walk all methods of the trait, including those of its supertraits
        for trait_ref in traits::supertraits(ccx.tcx(), poly_trait_ref) {
            let vtable = fulfill_obligation(ccx, DUMMY_SP, trait_ref);
            match vtable {
                traits::VtableImpl(
                    traits::VtableImplData {
                        impl_def_id,
                        substs,
                        nested: _ }) => {
                    let items = meth::get_vtable_methods(ccx, impl_def_id, substs)
                        .into_iter()
                        // filter out None values
                        .filter_map(|opt_impl_method| opt_impl_method)
                        // create translation items
                        .filter_map(|impl_method| {
                            let substs = ccx.tcx().mk_substs(impl_method.substs);
                            let original_definition = impl_method.method.def_id;
                            with_local_instance_of(original_definition, ccx, |node_id| {
                                create_trans_item_for_local_fn(ccx,
                                                               node_id,
                                                               substs,
                                                               &Substs::trans_empty(),
                                                               original_definition)
                            })
                        })
                        .collect::<Vec<_>>();

                    output.extend(items.into_iter());
                }
                _ => { /* */ }
            }
        }
    }
}

fn with_local_instance_of<'a, 'tcx, F, T>(def_id: DefId,
                                          ccx: &CrateContext<'a, 'tcx>, f: F)
                                          -> Option<T>
    where F: FnOnce(NodeId) -> T
{
    if let Some(local_def_id) = inline::get_local_instance(ccx, def_id) {
        let node_id = ccx.tcx().map.as_local_node_id(local_def_id).unwrap();
        Some(f(node_id))
    }
    else {
        None
    }
}

//=-----------------------------------------------------------------------------
// Root Collection
//=-----------------------------------------------------------------------------

struct RootCollector<'b, 'a: 'b, 'tcx: 'a + 'b> {
    ccx: &'b CrateContext<'a, 'tcx>,
    mode: TransItemCollectionMode,
    output: &'b mut Vec<TransItem<'tcx>>,
    enclosing_item: Option<&'tcx hir::Item>,
    trans_empty_substs: &'tcx Substs<'tcx>
}

impl<'b, 'a, 'v> hir_visit::Visitor<'v> for RootCollector<'b, 'a, 'v> {
    fn visit_item(&mut self, item: &'v hir::Item) {
        let old_enclosing_item = self.enclosing_item;
        self.enclosing_item = Some(item);

        match item.node {
            hir::ItemExternCrate(..) |
            hir::ItemUse(..)         |
            hir::ItemForeignMod(..)  |
            hir::ItemTy(..)          |
            hir::ItemDefaultImpl(..) |
            hir::ItemTrait(..)       |
            hir::ItemConst(..)       |
            hir::ItemMod(..)         => {
                // Nothing to do, just keep recursing...
            }

            hir::ItemImpl(..) => {
                if self.mode == TransItemCollectionMode::Eager {
                    create_trans_items_for_default_impls(self.ccx,
                                                         item,
                                                         self.trans_empty_substs,
                                                         self.output);
                }
            }

            hir::ItemEnum(_, ref generics)        |
            hir::ItemStruct(_, ref generics)      => {
                if !generics.is_parameterized() {
                    let ty = {
                        let tables = self.ccx.tcx().tables.borrow();
                        tables.node_types[&item.id]
                    };

                    if self.mode == TransItemCollectionMode::Eager {
                        debug!("RootCollector: ADT drop-glue for {}",
                               def_id_to_string(self.ccx,
                                                self.ccx.tcx().map.local_def_id(item.id),
                                                None));

                        create_drop_glue_trans_items(self.ccx,
                                                     ty,
                                                     self.trans_empty_substs,
                                                     self.output);
                    }
                }
            }
            hir::ItemStatic(..) => {
                debug!("RootCollector: ItemStatic({})",
                       def_id_to_string(self.ccx,
                                        self.ccx.tcx().map.local_def_id(item.id),
                                        None));
                self.output.push(TransItem::Static(item.id));
            }
            hir::ItemFn(_, _, constness, _, ref generics, _) => {
                if !generics.is_type_parameterized() &&
                   constness == hir::Constness::NotConst {
                    let def_id = self.ccx.tcx().map.local_def_id(item.id);

                    debug!("RootCollector: ItemFn({})",
                           def_id_to_string(self.ccx, def_id, None));

                    self.output.push(TransItem::Fn {
                        node_id: item.id,
                        substs: self.trans_empty_substs,
                        original_definition: def_id
                    });
                }
            }
        }

        hir_visit::walk_item(self, item);
        self.enclosing_item = old_enclosing_item;
    }

    fn visit_impl_item(&mut self, ii: &'v hir::ImplItem) {
        match ii.node {
            hir::ImplItemKind::Method(hir::MethodSig {
                ref generics,
                constness,
                ..
            }, _) if constness == hir::Constness::NotConst => {
                let hir_map = &self.ccx.tcx().map;
                let parent_node_id = hir_map.get_parent_node(ii.id);
                let is_impl_generic = match hir_map.expect_item(parent_node_id) {
                    &hir::Item {
                        node: hir::ItemImpl(_, _, ref generics, _, _, _),
                        ..
                    } => {
                        generics.is_type_parameterized()
                    }
                    _ => {
                        unreachable!()
                    }
                };

                if !generics.is_type_parameterized() && !is_impl_generic {
                    let def_id = self.ccx.tcx().map.local_def_id(ii.id);

                    debug!("RootCollector: MethodImplItem({})",
                           def_id_to_string(self.ccx, def_id, None));

                    self.output.push(TransItem::Fn {
                        node_id: ii.id,
                        substs: self.trans_empty_substs,
                        original_definition: def_id
                    });
                }
            }
            _ => { /* Nothing to do here */ }
        }

        hir_visit::walk_impl_item(self, ii)
    }
}

fn create_trans_items_for_default_impls<'a, 'tcx>(ccx: &CrateContext<'a, 'tcx>,
                                                    item: &'tcx hir::Item,
                                                    trans_empty_substs: &'tcx Substs<'tcx>,
                                                    output: &mut Vec<TransItem<'tcx>>) {
    match item.node {
        hir::ItemImpl(_,
                      _,
                      ref generics,
                      _,
                      _,
                      ref items) => {
            if generics.is_type_parameterized() {
                return
            }

            let tcx = ccx.tcx();
            let impl_def_id = tcx.map.local_def_id(item.id);

            debug!("create_trans_items_for_default_impls(item={})",
                   def_id_to_string(ccx, impl_def_id, None));

            if let Some(trait_ref) = tcx.impl_trait_ref(impl_def_id) {
                let default_impls = tcx.provided_trait_methods(trait_ref.def_id);
                let callee_substs = tcx.mk_substs(tcx.erase_regions(trait_ref.substs));
                let overridden_methods: FnvHashSet<_> = items.iter()
                                                             .map(|item| item.name)
                                                             .collect();
                for default_impl in default_impls {
                    if overridden_methods.contains(&default_impl.name) {
                        continue;
                    }

                    if default_impl.generics.has_type_params(subst::FnSpace) {
                        continue;
                    }

                    // The substitutions we have are on the impl, so we grab
                    // the method type from the impl to substitute into.
                    let mth = tcx.get_impl_method(impl_def_id,
                                                  callee_substs.clone(),
                                                  default_impl.name);

                    assert!(mth.is_provided);

                    let predicates = mth.method.predicates.predicates.subst(tcx, &mth.substs);
                    if !normalize_and_test_predicates(ccx, predicates.into_vec()) {
                        continue;
                    }

                    with_local_instance_of(default_impl.def_id, ccx, |node_id| {
                        let item = create_trans_item_for_local_fn(ccx,
                                                                  node_id,
                                                                  callee_substs,
                                                                  trans_empty_substs,
                                                                  default_impl.def_id);
                        output.push(item);
                    });
                }
            }
        }
        _ => {
            unreachable!()
        }
    }
}

//=-----------------------------------------------------------------------------
// TransItem String Keys
//=-----------------------------------------------------------------------------

// The code below allows for producing a unique string key for a trans item.
// These keys are used by the handwritten auto-tests, so they need to be
// predictable and human-readable.
//
// Note: A lot of this could looks very similar to what's already in the
//       ppaux module. It would be good to refactor things so we only have one
//       parameterizable implementation for printing types.

/// Same as `unique_type_name()` but with the result pushed onto the given
/// `output` parameter.
pub fn push_unique_type_name<'a, 'tcx>(cx: &CrateContext<'a, 'tcx>,
                                       t: ty::Ty<'tcx>,
                                       output: &mut String) {
    match t.sty {
        ty::TyBool              => output.push_str("bool"),
        ty::TyChar              => output.push_str("char"),
        ty::TyStr               => output.push_str("str"),
        ty::TyInt(ast::TyIs)    => output.push_str("isize"),
        ty::TyInt(ast::TyI8)    => output.push_str("i8"),
        ty::TyInt(ast::TyI16)   => output.push_str("i16"),
        ty::TyInt(ast::TyI32)   => output.push_str("i32"),
        ty::TyInt(ast::TyI64)   => output.push_str("i64"),
        ty::TyUint(ast::TyUs)   => output.push_str("usize"),
        ty::TyUint(ast::TyU8)   => output.push_str("u8"),
        ty::TyUint(ast::TyU16)  => output.push_str("u16"),
        ty::TyUint(ast::TyU32)  => output.push_str("u32"),
        ty::TyUint(ast::TyU64)  => output.push_str("u64"),
        ty::TyFloat(ast::TyF32) => output.push_str("f32"),
        ty::TyFloat(ast::TyF64) => output.push_str("f64"),
        ty::TyStruct(adt_def, substs) |
        ty::TyEnum(adt_def, substs) => {
            push_item_name(cx, adt_def.did, output);
            push_type_params(cx, substs, &[], output);
        },
        ty::TyTuple(ref component_types) => {
            output.push('(');
            for &component_type in component_types {
                push_unique_type_name(cx, component_type, output);
                output.push_str(", ");
            }
            if !component_types.is_empty() {
                output.pop();
                output.pop();
            }
            output.push(')');
        },
        ty::TyBox(inner_type) => {
            output.push_str("Box<");
            push_unique_type_name(cx, inner_type, output);
            output.push('>');
        },
        ty::TyRawPtr(ty::TypeAndMut { ty: inner_type, mutbl } ) => {
            output.push('*');
            match mutbl {
                hir::MutImmutable => output.push_str("const "),
                hir::MutMutable => output.push_str("mut "),
            }

            push_unique_type_name(cx, inner_type, output);
        },
        ty::TyRef(_, ty::TypeAndMut { ty: inner_type, mutbl }) => {
            output.push('&');
            if mutbl == hir::MutMutable {
                output.push_str("mut ");
            }

            push_unique_type_name(cx, inner_type, output);
        },
        ty::TyArray(inner_type, len) => {
            output.push('[');
            push_unique_type_name(cx, inner_type, output);
            output.push_str(&format!("; {}", len));
            output.push(']');
        },
        ty::TySlice(inner_type) => {
            output.push('[');
            push_unique_type_name(cx, inner_type, output);
            output.push(']');
        },
        ty::TyTrait(ref trait_data) => {
            push_item_name(cx, trait_data.principal.skip_binder().def_id, output);
            push_type_params(cx,
                             &trait_data.principal.skip_binder().substs,
                             &trait_data.bounds.projection_bounds,
                             output);
        },
        ty::TyBareFn(_, &ty::BareFnTy{ unsafety, abi, ref sig } ) => {
            if unsafety == hir::Unsafety::Unsafe {
                output.push_str("unsafe ");
            }

            if abi != ::syntax::abi::Rust {
                output.push_str("extern \"");
                output.push_str(abi.name());
                output.push_str("\" ");
            }

            output.push_str("fn(");

            let sig = cx.tcx().erase_late_bound_regions(sig);
            if !sig.inputs.is_empty() {
                for &parameter_type in &sig.inputs {
                    push_unique_type_name(cx, parameter_type, output);
                    output.push_str(", ");
                }
                output.pop();
                output.pop();
            }

            if sig.variadic {
                if !sig.inputs.is_empty() {
                    output.push_str(", ...");
                } else {
                    output.push_str("...");
                }
            }

            output.push(')');

            match sig.output {
                ty::FnConverging(result_type) if result_type.is_nil() => {}
                ty::FnConverging(result_type) => {
                    output.push_str(" -> ");
                    push_unique_type_name(cx, result_type, output);
                }
                ty::FnDiverging => {
                    output.push_str(" -> !");
                }
            }
        },
        ty::TyClosure(def_id, ref closure_substs) => {
            push_item_name(cx, def_id, output);
            output.push_str("{");
            output.push_str(&format!("{}:{}", def_id.krate, def_id.index.as_usize()));
            output.push_str("}");
            push_type_params(cx, closure_substs.func_substs, &[], output);
        }
        ty::TyError |
        ty::TyInfer(_) |
        ty::TyProjection(..) |
        ty::TyParam(_) => {
            cx.sess().bug(&format!("debuginfo: Trying to create type name for \
                unexpected type: {:?}", t));
        }
    }
}

fn push_item_name(ccx: &CrateContext,
                  def_id: DefId,
                  output: &mut String) {
    if def_id.is_local() {
        let node_id = ccx.tcx().map.as_local_node_id(def_id).unwrap();
        let inlined_from = ccx.external_srcs()
                              .borrow()
                              .get(&node_id)
                              .map(|def_id| *def_id);

        if let Some(extern_def_id) = inlined_from {
            push_item_name(ccx, extern_def_id, output);
            return;
        }

        output.push_str(&ccx.link_meta().crate_name);
        output.push_str("::");
    }

    for part in ccx.tcx().def_path(def_id) {
        output.push_str(&format!("{}[{}]::",
                        part.data.as_interned_str(),
                        part.disambiguator));
    }

    output.pop();
    output.pop();
}

fn push_type_params<'a, 'tcx>(cx: &CrateContext<'a, 'tcx>,
                              substs: &Substs<'tcx>,
                              projections: &[ty::PolyProjectionPredicate<'tcx>],
                              output: &mut String) {
    if substs.types.is_empty() && projections.is_empty() {
        return;
    }

    output.push('<');

    for &type_parameter in &substs.types {
        push_unique_type_name(cx, type_parameter, output);
        output.push_str(", ");
    }

    for projection in projections {
        let projection = projection.skip_binder();
        let name = token::get_ident_interner().get(projection.projection_ty.item_name);
        output.push_str(&name[..]);
        output.push_str("=");
        push_unique_type_name(cx, projection.ty, output);
        output.push_str(", ");
    }

    output.pop();
    output.pop();

    output.push('>');
}

fn push_def_id_as_string<'a, 'tcx>(ccx: &CrateContext<'a, 'tcx>,
                              def_id: DefId,
                              substs: Option<&Substs<'tcx>>,
                              output: &mut String) {
    push_item_name(ccx, def_id, output);

    if let Some(substs) = substs {
        push_type_params(ccx, substs, &[], output);
    }
}

fn def_id_to_string<'a, 'tcx>(ccx: &CrateContext<'a, 'tcx>,
                              def_id: DefId,
                              substs: Option<&Substs<'tcx>>)
                              -> String {
    let mut output = String::new();
    push_def_id_as_string(ccx, def_id, substs, &mut output);
    output
}

fn type_to_string<'a, 'tcx>(ccx: &CrateContext<'a, 'tcx>,
                            ty: ty::Ty<'tcx>)
                            -> String {
    let mut output = String::new();
    push_unique_type_name(ccx, ty, &mut output);
    output
}

impl<'tcx> TransItem<'tcx> {

    pub fn to_string<'a>(&self, ccx: &CrateContext<'a, 'tcx>) -> String {
        let hir_map = &ccx.tcx().map;

        return match *self {
            TransItem::DropGlue(t) => {
                let mut s = String::with_capacity(32);
                s.push_str("drop-glue ");
                push_unique_type_name(ccx, t, &mut s);
                s
            }
            TransItem::Fn { node_id, ref substs, .. } => {
                let def_id = match ccx.tcx().node_id_to_type(node_id).sty {
                    ty::TyClosure(def_id, _) => def_id,
                    _ => hir_map.local_def_id(node_id)
                };

                to_string_internal(ccx, "fn ", def_id, Some(substs))
            },
            TransItem::Static(node_id) => {
                let def_id = hir_map.local_def_id(node_id);
                to_string_internal(ccx, "static ", def_id, None)
            },
        };

        fn to_string_internal<'a, 'tcx>(ccx: &CrateContext<'a, 'tcx>,
                                        prefix: &str,
                                        def_id: DefId,
                                        substs: Option<&Substs<'tcx>>)
                                        -> String {
            let mut result = String::with_capacity(32);
            result.push_str(prefix);
            push_def_id_as_string(ccx, def_id, substs, &mut result);
            result
        }
    }

    fn to_raw_string(&self) -> String {
        match *self {
            TransItem::DropGlue(t) => {
                format!("DropGlue({})", t as *const _ as usize)
            }
            TransItem::Fn { node_id, substs, original_definition } => {
                format!("Fn({:?}[{:?}], {})",
                         node_id,
                         original_definition,
                         substs as *const _ as usize)
            }
            TransItem::Static(id) => {
                format!("Static({:?})", id)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransItemState {
    PredictedAndGenerated,
    PredictedButNotGenerated,
    NotPredictedButGenerated,
}

pub fn print_collection_results<'a, 'tcx>(ccx: &CrateContext<'a, 'tcx>) {
    use std::hash::{Hash, SipHasher, Hasher};

    if !cfg!(debug_assertions) {
        return;
    }

    if ccx.sess().opts.debugging_opts.print_trans_items.is_none() {
        return;
    }

    fn hash<T: Hash>(t: &T) -> u64 {
        let mut s = SipHasher::new();
        t.hash(&mut s);
        s.finish()
    }

    let trans_items = ccx.translation_items().borrow();

    {
        // Check for duplicate item keys
        let mut item_keys = FnvHashMap();

        for (item, item_state) in trans_items.iter() {
            let k = item.to_string(&ccx);

            if item_keys.contains_key(&k) {
                let prev: (TransItem, TransItemState) = item_keys[&k];
                debug!("DUPLICATE KEY: {}", k);
                debug!(" (1) {:?}, {:?}, hash: {}, raw: {}",
                       prev.0,
                       prev.1,
                       hash(&prev.0),
                       prev.0.to_raw_string());

                debug!(" (2) {:?}, {:?}, hash: {}, raw: {}",
                       *item,
                       *item_state,
                       hash(item),
                       item.to_raw_string());
            } else {
                item_keys.insert(k, (*item, *item_state));
            }
        }
    }

    let mut predicted_but_not_generated = FnvHashSet();
    let mut not_predicted_but_generated = FnvHashSet();
    let mut predicted = FnvHashSet();
    let mut generated = FnvHashSet();

    for (item, item_state) in trans_items.iter() {
        let item_key = item.to_string(&ccx);

        match *item_state {
            TransItemState::PredictedAndGenerated => {
                predicted.insert(item_key.clone());
                generated.insert(item_key);
            }
            TransItemState::PredictedButNotGenerated => {
                predicted_but_not_generated.insert(item_key.clone());
                predicted.insert(item_key);
            }
            TransItemState::NotPredictedButGenerated => {
                not_predicted_but_generated.insert(item_key.clone());
                generated.insert(item_key);
            }
        }
    }

    debug!("Total number of translation items predicted: {}", predicted.len());
    debug!("Total number of translation items generated: {}", generated.len());
    debug!("Total number of translation items predicted but not generated: {}",
           predicted_but_not_generated.len());
    debug!("Total number of translation items not predicted but generated: {}",
           not_predicted_but_generated.len());

    if generated.len() > 0 {
        debug!("Failed to predict {}% of translation items",
               (100 * not_predicted_but_generated.len()) / generated.len());
    }
    if generated.len() > 0 {
        debug!("Predict {}% too many translation items",
               (100 * predicted_but_not_generated.len()) / generated.len());
    }

    debug!("");
    debug!("Not predicted but generated:");
    debug!("============================");
    for item in not_predicted_but_generated {
        debug!(" - {}", item);
    }

    debug!("");
    debug!("Predicted but not generated:");
    debug!("============================");
    for item in predicted_but_not_generated {
        debug!(" - {}", item);
    }
}
