//! A classic liveness analysis based on dataflow over the AST. Computes,
//! for each local variable in a function, whether that variable is live
//! at a given point. Program execution points are identified by their
//! IDs.
//!
//! # Basic idea
//!
//! The basic model is that each local variable is assigned an index. We
//! represent sets of local variables using a vector indexed by this
//! index. The value in the vector is either 0, indicating the variable
//! is dead, or the ID of an expression that uses the variable.
//!
//! We conceptually walk over the AST in reverse execution order. If we
//! find a use of a variable, we add it to the set of live variables. If
//! we find an assignment to a variable, we remove it from the set of live
//! variables. When we have to merge two flows, we take the union of
//! those two flows -- if the variable is live on both paths, we simply
//! pick one ID. In the event of loops, we continue doing this until a
//! fixed point is reached.
//!
//! ## Checking initialization
//!
//! At the function entry point, all variables must be dead. If this is
//! not the case, we can report an error using the ID found in the set of
//! live variables, which identifies a use of the variable which is not
//! dominated by an assignment.
//!
//! ## Checking moves
//!
//! After each explicit move, the variable must be dead.
//!
//! ## Computing last uses
//!
//! Any use of the variable where the variable is dead afterwards is a
//! last use.
//!
//! # Implementation details
//!
//! The actual implementation contains two (nested) walks over the AST.
//! The outer walk has the job of building up the ir_maps instance for the
//! enclosing function. On the way down the tree, it identifies those AST
//! nodes and variable IDs that will be needed for the liveness analysis
//! and assigns them contiguous IDs. The liveness ID for an AST node is
//! called a `live_node` (it's a newtype'd `u32`) and the ID for a variable
//! is called a `variable` (another newtype'd `u32`).
//!
//! On the way back up the tree, as we are about to exit from a function
//! declaration we allocate a `liveness` instance. Now that we know
//! precisely how many nodes and variables we need, we can allocate all
//! the various arrays that we will need to precisely the right size. We then
//! perform the actual propagation on the `liveness` instance.
//!
//! This propagation is encoded in the various `propagate_through_*()`
//! methods. It effectively does a reverse walk of the AST; whenever we
//! reach a loop node, we iterate until a fixed point is reached.
//!
//! ## The `RWU` struct
//!
//! At each live node `N`, we track three pieces of information for each
//! variable `V` (these are encapsulated in the `RWU` struct):
//!
//! - `reader`: the `LiveNode` ID of some node which will read the value
//!    that `V` holds on entry to `N`. Formally: a node `M` such
//!    that there exists a path `P` from `N` to `M` where `P` does not
//!    write `V`. If the `reader` is `None`, then the current
//!    value will never be read (the variable is dead, essentially).
//!
//! - `writer`: the `LiveNode` ID of some node which will write the
//!    variable `V` and which is reachable from `N`. Formally: a node `M`
//!    such that there exists a path `P` from `N` to `M` and `M` writes
//!    `V`. If the `writer` is `None`, then there is no writer
//!    of `V` that follows `N`.
//!
//! - `used`: a boolean value indicating whether `V` is *used*. We
//!   distinguish a *read* from a *use* in that a *use* is some read that
//!   is not just used to generate a new value. For example, `x += 1` is
//!   a read but not a use. This is used to generate better warnings.
//!
//! ## Special nodes and variables
//!
//! We generate various special nodes for various, well, special purposes.
//! These are described in the `Liveness` struct.

use self::LiveNodeKind::*;
use self::VarKind::*;

use rustc_ast::InlineAsmOptions;
use rustc_data_structures::fx::FxIndexMap;
use rustc_errors::Applicability;
use rustc_hir as hir;
use rustc_hir::def::*;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, NestedVisitorMap, Visitor};
use rustc_hir::{Expr, HirId, HirIdMap, HirIdSet};
use rustc_index::vec::IndexVec;
use rustc_middle::hir::map::Map;
use rustc_middle::ty::query::Providers;
use rustc_middle::ty::{self, DefIdTree, TyCtxt};
use rustc_session::lint;
use rustc_span::symbol::{kw, sym, Symbol};
use rustc_span::Span;

use std::collections::VecDeque;
use std::io;
use std::io::prelude::*;
use std::rc::Rc;

rustc_index::newtype_index! {
    pub struct Variable {
        DEBUG_FORMAT = "v({})",
    }
}

rustc_index::newtype_index! {
    pub struct LiveNode {
        DEBUG_FORMAT = "ln({})",
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
enum LiveNodeKind {
    UpvarNode(Span),
    ExprNode(Span),
    VarDefNode(Span),
    ClosureNode,
    ExitNode,
}

fn live_node_kind_to_string(lnk: LiveNodeKind, tcx: TyCtxt<'_>) -> String {
    let sm = tcx.sess.source_map();
    match lnk {
        UpvarNode(s) => format!("Upvar node [{}]", sm.span_to_string(s)),
        ExprNode(s) => format!("Expr node [{}]", sm.span_to_string(s)),
        VarDefNode(s) => format!("Var def node [{}]", sm.span_to_string(s)),
        ClosureNode => "Closure node".to_owned(),
        ExitNode => "Exit node".to_owned(),
    }
}

fn check_mod_liveness(tcx: TyCtxt<'_>, module_def_id: LocalDefId) {
    tcx.hir().visit_item_likes_in_module(module_def_id, &mut IrMaps::new(tcx).as_deep_visitor());
}

pub fn provide(providers: &mut Providers) {
    *providers = Providers { check_mod_liveness, ..*providers };
}

// ______________________________________________________________________
// Creating ir_maps
//
// This is the first pass and the one that drives the main
// computation.  It walks up and down the IR once.  On the way down,
// we count for each function the number of variables as well as
// liveness nodes.  A liveness node is basically an expression or
// capture clause that does something of interest: either it has
// interesting control flow or it uses/defines a local variable.
//
// On the way back up, at each function node we create liveness sets
// (we now know precisely how big to make our various vectors and so
// forth) and then do the data-flow propagation to compute the set
// of live variables at each program point.
//
// Finally, we run back over the IR one last time and, using the
// computed liveness, check various safety conditions.  For example,
// there must be no live nodes at the definition site for a variable
// unless it has an initializer.  Similarly, each non-mutable local
// variable must not be assigned if there is some successor
// assignment.  And so forth.

struct CaptureInfo {
    ln: LiveNode,
    var_hid: HirId,
}

#[derive(Copy, Clone, Debug)]
struct LocalInfo {
    id: HirId,
    name: Symbol,
    is_shorthand: bool,
}

#[derive(Copy, Clone, Debug)]
enum VarKind {
    Param(HirId, Symbol),
    Local(LocalInfo),
    Upvar(HirId, Symbol),
}

struct IrMaps<'tcx> {
    tcx: TyCtxt<'tcx>,
    live_node_map: HirIdMap<LiveNode>,
    variable_map: HirIdMap<Variable>,
    capture_info_map: HirIdMap<Rc<Vec<CaptureInfo>>>,
    var_kinds: IndexVec<Variable, VarKind>,
    lnks: IndexVec<LiveNode, LiveNodeKind>,
}

impl IrMaps<'tcx> {
    fn new(tcx: TyCtxt<'tcx>) -> IrMaps<'tcx> {
        IrMaps {
            tcx,
            live_node_map: HirIdMap::default(),
            variable_map: HirIdMap::default(),
            capture_info_map: Default::default(),
            var_kinds: IndexVec::new(),
            lnks: IndexVec::new(),
        }
    }

    fn add_live_node(&mut self, lnk: LiveNodeKind) -> LiveNode {
        let ln = self.lnks.push(lnk);

        debug!("{:?} is of kind {}", ln, live_node_kind_to_string(lnk, self.tcx));

        ln
    }

    fn add_live_node_for_node(&mut self, hir_id: HirId, lnk: LiveNodeKind) {
        let ln = self.add_live_node(lnk);
        self.live_node_map.insert(hir_id, ln);

        debug!("{:?} is node {:?}", ln, hir_id);
    }

    fn add_variable(&mut self, vk: VarKind) -> Variable {
        let v = self.var_kinds.push(vk);

        match vk {
            Local(LocalInfo { id: node_id, .. }) | Param(node_id, _) | Upvar(node_id, _) => {
                self.variable_map.insert(node_id, v);
            }
        }

        debug!("{:?} is {:?}", v, vk);

        v
    }

    fn variable(&self, hir_id: HirId, span: Span) -> Variable {
        match self.variable_map.get(&hir_id) {
            Some(&var) => var,
            None => {
                span_bug!(span, "no variable registered for id {:?}", hir_id);
            }
        }
    }

    fn variable_name(&self, var: Variable) -> Symbol {
        match self.var_kinds[var] {
            Local(LocalInfo { name, .. }) | Param(_, name) | Upvar(_, name) => name,
        }
    }

    fn variable_is_shorthand(&self, var: Variable) -> bool {
        match self.var_kinds[var] {
            Local(LocalInfo { is_shorthand, .. }) => is_shorthand,
            Param(..) | Upvar(..) => false,
        }
    }

    fn set_captures(&mut self, hir_id: HirId, cs: Vec<CaptureInfo>) {
        self.capture_info_map.insert(hir_id, Rc::new(cs));
    }

    fn add_from_pat(&mut self, pat: &hir::Pat<'tcx>) {
        // For struct patterns, take note of which fields used shorthand
        // (`x` rather than `x: x`).
        let mut shorthand_field_ids = HirIdSet::default();
        let mut pats = VecDeque::new();
        pats.push_back(pat);
        while let Some(pat) = pats.pop_front() {
            use rustc_hir::PatKind::*;
            match &pat.kind {
                Binding(.., inner_pat) => {
                    pats.extend(inner_pat.iter());
                }
                Struct(_, fields, _) => {
                    let ids = fields.iter().filter(|f| f.is_shorthand).map(|f| f.pat.hir_id);
                    shorthand_field_ids.extend(ids);
                }
                Ref(inner_pat, _) | Box(inner_pat) => {
                    pats.push_back(inner_pat);
                }
                TupleStruct(_, inner_pats, _) | Tuple(inner_pats, _) | Or(inner_pats) => {
                    pats.extend(inner_pats.iter());
                }
                Slice(pre_pats, inner_pat, post_pats) => {
                    pats.extend(pre_pats.iter());
                    pats.extend(inner_pat.iter());
                    pats.extend(post_pats.iter());
                }
                _ => {}
            }
        }

        pat.each_binding(|_, hir_id, _, ident| {
            self.add_live_node_for_node(hir_id, VarDefNode(ident.span));
            self.add_variable(Local(LocalInfo {
                id: hir_id,
                name: ident.name,
                is_shorthand: shorthand_field_ids.contains(&hir_id),
            }));
        });
    }
}

impl<'tcx> Visitor<'tcx> for IrMaps<'tcx> {
    type Map = Map<'tcx>;

    fn nested_visit_map(&mut self) -> NestedVisitorMap<Self::Map> {
        NestedVisitorMap::OnlyBodies(self.tcx.hir())
    }

    fn visit_body(&mut self, body: &'tcx hir::Body<'tcx>) {
        debug!("visit_body {:?}", body.id());

        // swap in a new set of IR maps for this body
        let mut maps = IrMaps::new(self.tcx);
        let hir_id = maps.tcx.hir().body_owner(body.id());
        let def_id = maps.tcx.hir().local_def_id(hir_id);

        // Don't run unused pass for #[derive()]
        if let Some(parent) = self.tcx.parent(def_id.to_def_id()) {
            if let DefKind::Impl = self.tcx.def_kind(parent.expect_local()) {
                if self.tcx.has_attr(parent, sym::automatically_derived) {
                    return;
                }
            }
        }

        if let Some(upvars) = maps.tcx.upvars_mentioned(def_id) {
            for (&var_hir_id, _upvar) in upvars {
                let var_name = maps.tcx.hir().name(var_hir_id);
                maps.add_variable(Upvar(var_hir_id, var_name));
            }
        }

        // gather up the various local variables, significant expressions,
        // and so forth:
        intravisit::walk_body(&mut maps, body);

        // compute liveness
        let mut lsets = Liveness::new(&mut maps, def_id);
        let entry_ln = lsets.compute(&body, hir_id);
        lsets.log_liveness(entry_ln, body.id().hir_id);

        // check for various error conditions
        lsets.visit_body(body);
        lsets.warn_about_unused_upvars(entry_ln);
        lsets.warn_about_unused_args(body, entry_ln);
    }

    fn visit_local(&mut self, local: &'tcx hir::Local<'tcx>) {
        self.add_from_pat(&local.pat);
        intravisit::walk_local(self, local);
    }

    fn visit_arm(&mut self, arm: &'tcx hir::Arm<'tcx>) {
        self.add_from_pat(&arm.pat);
        intravisit::walk_arm(self, arm);
    }

    fn visit_param(&mut self, param: &'tcx hir::Param<'tcx>) {
        let is_shorthand = match param.pat.kind {
            rustc_hir::PatKind::Struct(..) => true,
            _ => false,
        };
        param.pat.each_binding(|_bm, hir_id, _x, ident| {
            let var = if is_shorthand {
                Local(LocalInfo { id: hir_id, name: ident.name, is_shorthand: true })
            } else {
                Param(hir_id, ident.name)
            };
            self.add_variable(var);
        });
        intravisit::walk_param(self, param);
    }

    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        match expr.kind {
            // live nodes required for uses or definitions of variables:
            hir::ExprKind::Path(hir::QPath::Resolved(_, ref path)) => {
                debug!("expr {}: path that leads to {:?}", expr.hir_id, path.res);
                if let Res::Local(_var_hir_id) = path.res {
                    self.add_live_node_for_node(expr.hir_id, ExprNode(expr.span));
                }
                intravisit::walk_expr(self, expr);
            }
            hir::ExprKind::Closure(..) => {
                // Interesting control flow (for loops can contain labeled
                // breaks or continues)
                self.add_live_node_for_node(expr.hir_id, ExprNode(expr.span));

                // Make a live_node for each captured variable, with the span
                // being the location that the variable is used.  This results
                // in better error messages than just pointing at the closure
                // construction site.
                let mut call_caps = Vec::new();
                let closure_def_id = self.tcx.hir().local_def_id(expr.hir_id);
                if let Some(upvars) = self.tcx.upvars_mentioned(closure_def_id) {
                    call_caps.extend(upvars.iter().map(|(&var_id, upvar)| {
                        let upvar_ln = self.add_live_node(UpvarNode(upvar.span));
                        CaptureInfo { ln: upvar_ln, var_hid: var_id }
                    }));
                }
                self.set_captures(expr.hir_id, call_caps);
                intravisit::walk_expr(self, expr);
            }

            // live nodes required for interesting control flow:
            hir::ExprKind::Match(..) | hir::ExprKind::Loop(..) => {
                self.add_live_node_for_node(expr.hir_id, ExprNode(expr.span));
                intravisit::walk_expr(self, expr);
            }
            hir::ExprKind::Binary(op, ..) if op.node.is_lazy() => {
                self.add_live_node_for_node(expr.hir_id, ExprNode(expr.span));
                intravisit::walk_expr(self, expr);
            }

            // otherwise, live nodes are not required:
            hir::ExprKind::Index(..)
            | hir::ExprKind::Field(..)
            | hir::ExprKind::Array(..)
            | hir::ExprKind::Call(..)
            | hir::ExprKind::MethodCall(..)
            | hir::ExprKind::Tup(..)
            | hir::ExprKind::Binary(..)
            | hir::ExprKind::AddrOf(..)
            | hir::ExprKind::Cast(..)
            | hir::ExprKind::DropTemps(..)
            | hir::ExprKind::Unary(..)
            | hir::ExprKind::Break(..)
            | hir::ExprKind::Continue(_)
            | hir::ExprKind::Lit(_)
            | hir::ExprKind::ConstBlock(..)
            | hir::ExprKind::Ret(..)
            | hir::ExprKind::Block(..)
            | hir::ExprKind::Assign(..)
            | hir::ExprKind::AssignOp(..)
            | hir::ExprKind::Struct(..)
            | hir::ExprKind::Repeat(..)
            | hir::ExprKind::InlineAsm(..)
            | hir::ExprKind::LlvmInlineAsm(..)
            | hir::ExprKind::Box(..)
            | hir::ExprKind::Yield(..)
            | hir::ExprKind::Type(..)
            | hir::ExprKind::Err
            | hir::ExprKind::Path(hir::QPath::TypeRelative(..))
            | hir::ExprKind::Path(hir::QPath::LangItem(..)) => {
                intravisit::walk_expr(self, expr);
            }
        }
    }
}

// ______________________________________________________________________
// Computing liveness sets
//
// Actually we compute just a bit more than just liveness, but we use
// the same basic propagation framework in all cases.

#[derive(Clone, Copy)]
struct RWU {
    reader: Option<LiveNode>,
    writer: Option<LiveNode>,
    used: bool,
}

/// Conceptually, this is like a `Vec<RWU>`. But the number of `RWU`s can get
/// very large, so it uses a more compact representation that takes advantage
/// of the fact that when the number of `RWU`s is large, most of them have an
/// invalid reader and an invalid writer.
struct RWUTable {
    /// Each entry in `packed_rwus` is either INV_INV_FALSE, INV_INV_TRUE, or
    /// an index into `unpacked_rwus`. In the common cases, this compacts the
    /// 65 bits of data into 32; in the uncommon cases, it expands the 65 bits
    /// in 96.
    ///
    /// More compact representations are possible -- e.g., use only 2 bits per
    /// packed `RWU` and make the secondary table a HashMap that maps from
    /// indices to `RWU`s -- but this one strikes a good balance between size
    /// and speed.
    packed_rwus: Vec<u32>,
    unpacked_rwus: Vec<RWU>,
}

// A constant representing `RWU { reader: None; writer: None; used: false }`.
const INV_INV_FALSE: u32 = u32::MAX;

// A constant representing `RWU { reader: None; writer: None; used: true }`.
const INV_INV_TRUE: u32 = u32::MAX - 1;

impl RWUTable {
    fn new(num_rwus: usize) -> RWUTable {
        Self { packed_rwus: vec![INV_INV_FALSE; num_rwus], unpacked_rwus: vec![] }
    }

    fn get(&self, idx: usize) -> RWU {
        let packed_rwu = self.packed_rwus[idx];
        match packed_rwu {
            INV_INV_FALSE => RWU { reader: None, writer: None, used: false },
            INV_INV_TRUE => RWU { reader: None, writer: None, used: true },
            _ => self.unpacked_rwus[packed_rwu as usize],
        }
    }

    fn get_reader(&self, idx: usize) -> Option<LiveNode> {
        let packed_rwu = self.packed_rwus[idx];
        match packed_rwu {
            INV_INV_FALSE | INV_INV_TRUE => None,
            _ => self.unpacked_rwus[packed_rwu as usize].reader,
        }
    }

    fn get_writer(&self, idx: usize) -> Option<LiveNode> {
        let packed_rwu = self.packed_rwus[idx];
        match packed_rwu {
            INV_INV_FALSE | INV_INV_TRUE => None,
            _ => self.unpacked_rwus[packed_rwu as usize].writer,
        }
    }

    fn get_used(&self, idx: usize) -> bool {
        let packed_rwu = self.packed_rwus[idx];
        match packed_rwu {
            INV_INV_FALSE => false,
            INV_INV_TRUE => true,
            _ => self.unpacked_rwus[packed_rwu as usize].used,
        }
    }

    #[inline]
    fn copy_packed(&mut self, dst_idx: usize, src_idx: usize) {
        self.packed_rwus[dst_idx] = self.packed_rwus[src_idx];
    }

    fn assign_unpacked(&mut self, idx: usize, rwu: RWU) {
        if rwu.reader == None && rwu.writer == None {
            // When we overwrite an indexing entry in `self.packed_rwus` with
            // `INV_INV_{TRUE,FALSE}` we don't remove the corresponding entry
            // from `self.unpacked_rwus`; it's not worth the effort, and we
            // can't have entries shifting around anyway.
            self.packed_rwus[idx] = if rwu.used { INV_INV_TRUE } else { INV_INV_FALSE }
        } else {
            // Add a new RWU to `unpacked_rwus` and make `packed_rwus[idx]`
            // point to it.
            self.packed_rwus[idx] = self.unpacked_rwus.len() as u32;
            self.unpacked_rwus.push(rwu);
        }
    }

    fn assign_inv_inv(&mut self, idx: usize) {
        self.packed_rwus[idx] = if self.get_used(idx) { INV_INV_TRUE } else { INV_INV_FALSE };
    }
}

const ACC_READ: u32 = 1;
const ACC_WRITE: u32 = 2;
const ACC_USE: u32 = 4;

struct Liveness<'a, 'tcx> {
    ir: &'a mut IrMaps<'tcx>,
    body_owner: LocalDefId,
    typeck_results: &'a ty::TypeckResults<'tcx>,
    param_env: ty::ParamEnv<'tcx>,
    upvars: Option<&'tcx FxIndexMap<hir::HirId, hir::Upvar>>,
    successors: IndexVec<LiveNode, Option<LiveNode>>,
    rwu_table: RWUTable,

    /// A live node representing a point of execution before closure entry &
    /// after closure exit. Used to calculate liveness of captured variables
    /// through calls to the same closure. Used for Fn & FnMut closures only.
    closure_ln: LiveNode,
    /// A live node representing every 'exit' from the function, whether it be
    /// by explicit return, panic, or other means.
    exit_ln: LiveNode,

    // mappings from loop node ID to LiveNode
    // ("break" label should map to loop node ID,
    // it probably doesn't now)
    break_ln: HirIdMap<LiveNode>,
    cont_ln: HirIdMap<LiveNode>,
}

impl<'a, 'tcx> Liveness<'a, 'tcx> {
    fn new(ir: &'a mut IrMaps<'tcx>, body_owner: LocalDefId) -> Liveness<'a, 'tcx> {
        let typeck_results = ir.tcx.typeck(body_owner);
        let param_env = ir.tcx.param_env(body_owner);
        let upvars = ir.tcx.upvars_mentioned(body_owner);

        let closure_ln = ir.add_live_node(ClosureNode);
        let exit_ln = ir.add_live_node(ExitNode);

        let num_live_nodes = ir.lnks.len();
        let num_vars = ir.var_kinds.len();

        Liveness {
            ir,
            body_owner,
            typeck_results,
            param_env,
            upvars,
            successors: IndexVec::from_elem_n(None, num_live_nodes),
            rwu_table: RWUTable::new(num_live_nodes * num_vars),
            closure_ln,
            exit_ln,
            break_ln: Default::default(),
            cont_ln: Default::default(),
        }
    }

    fn live_node(&self, hir_id: HirId, span: Span) -> LiveNode {
        match self.ir.live_node_map.get(&hir_id) {
            Some(&ln) => ln,
            None => {
                // This must be a mismatch between the ir_map construction
                // above and the propagation code below; the two sets of
                // code have to agree about which AST nodes are worth
                // creating liveness nodes for.
                span_bug!(span, "no live node registered for node {:?}", hir_id);
            }
        }
    }

    fn variable(&self, hir_id: HirId, span: Span) -> Variable {
        self.ir.variable(hir_id, span)
    }

    fn define_bindings_in_pat(&mut self, pat: &hir::Pat<'_>, mut succ: LiveNode) -> LiveNode {
        // In an or-pattern, only consider the first pattern; any later patterns
        // must have the same bindings, and we also consider the first pattern
        // to be the "authoritative" set of ids.
        pat.each_binding_or_first(&mut |_, hir_id, pat_sp, ident| {
            let ln = self.live_node(hir_id, pat_sp);
            let var = self.variable(hir_id, ident.span);
            self.init_from_succ(ln, succ);
            self.define(ln, var);
            succ = ln;
        });
        succ
    }

    fn idx(&self, ln: LiveNode, var: Variable) -> usize {
        ln.index() * self.ir.var_kinds.len() + var.index()
    }

    fn live_on_entry(&self, ln: LiveNode, var: Variable) -> Option<LiveNodeKind> {
        if let Some(reader) = self.rwu_table.get_reader(self.idx(ln, var)) {
            Some(self.ir.lnks[reader])
        } else {
            None
        }
    }

    // Is this variable live on entry to any of its successor nodes?
    fn live_on_exit(&self, ln: LiveNode, var: Variable) -> Option<LiveNodeKind> {
        let successor = self.successors[ln].unwrap();
        self.live_on_entry(successor, var)
    }

    fn used_on_entry(&self, ln: LiveNode, var: Variable) -> bool {
        self.rwu_table.get_used(self.idx(ln, var))
    }

    fn assigned_on_entry(&self, ln: LiveNode, var: Variable) -> Option<LiveNodeKind> {
        if let Some(writer) = self.rwu_table.get_writer(self.idx(ln, var)) {
            Some(self.ir.lnks[writer])
        } else {
            None
        }
    }

    fn assigned_on_exit(&self, ln: LiveNode, var: Variable) -> Option<LiveNodeKind> {
        let successor = self.successors[ln].unwrap();
        self.assigned_on_entry(successor, var)
    }

    fn indices2<F>(&mut self, ln: LiveNode, succ_ln: LiveNode, mut op: F)
    where
        F: FnMut(&mut Liveness<'a, 'tcx>, usize, usize),
    {
        let node_base_idx = self.idx(ln, Variable::from(0u32));
        let succ_base_idx = self.idx(succ_ln, Variable::from(0u32));
        for var_idx in 0..self.ir.var_kinds.len() {
            op(self, node_base_idx + var_idx, succ_base_idx + var_idx);
        }
    }

    fn write_vars<F>(&self, wr: &mut dyn Write, ln: LiveNode, mut test: F) -> io::Result<()>
    where
        F: FnMut(usize) -> bool,
    {
        let node_base_idx = self.idx(ln, Variable::from(0u32));
        for var_idx in 0..self.ir.var_kinds.len() {
            let idx = node_base_idx + var_idx;
            if test(idx) {
                write!(wr, " {:?}", Variable::from(var_idx))?;
            }
        }
        Ok(())
    }

    #[allow(unused_must_use)]
    fn ln_str(&self, ln: LiveNode) -> String {
        let mut wr = Vec::new();
        {
            let wr = &mut wr as &mut dyn Write;
            write!(wr, "[{:?} of kind {:?} reads", ln, self.ir.lnks[ln]);
            self.write_vars(wr, ln, |idx| self.rwu_table.get_reader(idx).is_some());
            write!(wr, "  writes");
            self.write_vars(wr, ln, |idx| self.rwu_table.get_writer(idx).is_some());
            write!(wr, "  uses");
            self.write_vars(wr, ln, |idx| self.rwu_table.get_used(idx));

            write!(wr, "  precedes {:?}]", self.successors[ln]);
        }
        String::from_utf8(wr).unwrap()
    }

    fn log_liveness(&self, entry_ln: LiveNode, hir_id: hir::HirId) {
        // hack to skip the loop unless debug! is enabled:
        debug!(
            "^^ liveness computation results for body {} (entry={:?})",
            {
                for ln_idx in 0..self.ir.lnks.len() {
                    debug!("{:?}", self.ln_str(LiveNode::from(ln_idx)));
                }
                hir_id
            },
            entry_ln
        );
    }

    fn init_empty(&mut self, ln: LiveNode, succ_ln: LiveNode) {
        self.successors[ln] = Some(succ_ln);

        // It is not necessary to initialize the RWUs here because they are all
        // set to INV_INV_FALSE when they are created, and the sets only grow
        // during iterations.
    }

    fn init_from_succ(&mut self, ln: LiveNode, succ_ln: LiveNode) {
        // more efficient version of init_empty() / merge_from_succ()
        self.successors[ln] = Some(succ_ln);

        self.indices2(ln, succ_ln, |this, idx, succ_idx| {
            this.rwu_table.copy_packed(idx, succ_idx);
        });
        debug!("init_from_succ(ln={}, succ={})", self.ln_str(ln), self.ln_str(succ_ln));
    }

    fn merge_from_succ(&mut self, ln: LiveNode, succ_ln: LiveNode, first_merge: bool) -> bool {
        if ln == succ_ln {
            return false;
        }

        let mut any_changed = false;
        self.indices2(ln, succ_ln, |this, idx, succ_idx| {
            // This is a special case, pulled out from the code below, where we
            // don't have to do anything. It occurs about 60-70% of the time.
            if this.rwu_table.packed_rwus[succ_idx] == INV_INV_FALSE {
                return;
            }

            let mut changed = false;
            let mut rwu = this.rwu_table.get(idx);
            let succ_rwu = this.rwu_table.get(succ_idx);
            if succ_rwu.reader.is_some() && rwu.reader.is_none() {
                rwu.reader = succ_rwu.reader;
                changed = true
            }

            if succ_rwu.writer.is_some() && rwu.writer.is_none() {
                rwu.writer = succ_rwu.writer;
                changed = true
            }

            if succ_rwu.used && !rwu.used {
                rwu.used = true;
                changed = true;
            }

            if changed {
                this.rwu_table.assign_unpacked(idx, rwu);
                any_changed = true;
            }
        });

        debug!(
            "merge_from_succ(ln={:?}, succ={}, first_merge={}, changed={})",
            ln,
            self.ln_str(succ_ln),
            first_merge,
            any_changed
        );
        any_changed
    }

    // Indicates that a local variable was *defined*; we know that no
    // uses of the variable can precede the definition (resolve checks
    // this) so we just clear out all the data.
    fn define(&mut self, writer: LiveNode, var: Variable) {
        let idx = self.idx(writer, var);
        self.rwu_table.assign_inv_inv(idx);

        debug!("{:?} defines {:?} (idx={}): {}", writer, var, idx, self.ln_str(writer));
    }

    // Either read, write, or both depending on the acc bitset
    fn acc(&mut self, ln: LiveNode, var: Variable, acc: u32) {
        debug!("{:?} accesses[{:x}] {:?}: {}", ln, acc, var, self.ln_str(ln));

        let idx = self.idx(ln, var);
        let mut rwu = self.rwu_table.get(idx);

        if (acc & ACC_WRITE) != 0 {
            rwu.reader = None;
            rwu.writer = Some(ln);
        }

        // Important: if we both read/write, must do read second
        // or else the write will override.
        if (acc & ACC_READ) != 0 {
            rwu.reader = Some(ln);
        }

        if (acc & ACC_USE) != 0 {
            rwu.used = true;
        }

        self.rwu_table.assign_unpacked(idx, rwu);
    }

    fn compute(&mut self, body: &hir::Body<'_>, hir_id: HirId) -> LiveNode {
        debug!("compute: for body {:?}", body.id().hir_id);

        // # Liveness of captured variables
        //
        // When computing the liveness for captured variables we take into
        // account how variable is captured (ByRef vs ByValue) and what is the
        // closure kind (Generator / FnOnce vs Fn / FnMut).
        //
        // Variables captured by reference are assumed to be used on the exit
        // from the closure.
        //
        // In FnOnce closures, variables captured by value are known to be dead
        // on exit since it is impossible to call the closure again.
        //
        // In Fn / FnMut closures, variables captured by value are live on exit
        // if they are live on the entry to the closure, since only the closure
        // itself can access them on subsequent calls.

        if let Some(upvars) = self.upvars {
            // Mark upvars captured by reference as used after closure exits.
            for (&var_hir_id, upvar) in upvars.iter().rev() {
                let upvar_id = ty::UpvarId {
                    var_path: ty::UpvarPath { hir_id: var_hir_id },
                    closure_expr_id: self.body_owner,
                };
                match self.typeck_results.upvar_capture(upvar_id) {
                    ty::UpvarCapture::ByRef(_) => {
                        let var = self.variable(var_hir_id, upvar.span);
                        self.acc(self.exit_ln, var, ACC_READ | ACC_USE);
                    }
                    ty::UpvarCapture::ByValue(_) => {}
                }
            }
        }

        let succ = self.propagate_through_expr(&body.value, self.exit_ln);

        if self.upvars.is_none() {
            // Either not a closure, or closure without any captured variables.
            // No need to determine liveness of captured variables, since there
            // are none.
            return succ;
        }

        let ty = self.typeck_results.node_type(hir_id);
        match ty.kind() {
            ty::Closure(_def_id, substs) => match substs.as_closure().kind() {
                ty::ClosureKind::Fn => {}
                ty::ClosureKind::FnMut => {}
                ty::ClosureKind::FnOnce => return succ,
            },
            ty::Generator(..) => return succ,
            _ => {
                span_bug!(
                    body.value.span,
                    "{} has upvars so it should have a closure type: {:?}",
                    hir_id,
                    ty
                );
            }
        };

        // Propagate through calls to the closure.
        let mut first_merge = true;
        loop {
            self.init_from_succ(self.closure_ln, succ);
            for param in body.params {
                param.pat.each_binding(|_bm, hir_id, _x, ident| {
                    let var = self.variable(hir_id, ident.span);
                    self.define(self.closure_ln, var);
                })
            }

            if !self.merge_from_succ(self.exit_ln, self.closure_ln, first_merge) {
                break;
            }
            first_merge = false;
            assert_eq!(succ, self.propagate_through_expr(&body.value, self.exit_ln));
        }

        succ
    }

    fn propagate_through_block(&mut self, blk: &hir::Block<'_>, succ: LiveNode) -> LiveNode {
        if blk.targeted_by_break {
            self.break_ln.insert(blk.hir_id, succ);
        }
        let succ = self.propagate_through_opt_expr(blk.expr.as_deref(), succ);
        blk.stmts.iter().rev().fold(succ, |succ, stmt| self.propagate_through_stmt(stmt, succ))
    }

    fn propagate_through_stmt(&mut self, stmt: &hir::Stmt<'_>, succ: LiveNode) -> LiveNode {
        match stmt.kind {
            hir::StmtKind::Local(ref local) => {
                // Note: we mark the variable as defined regardless of whether
                // there is an initializer.  Initially I had thought to only mark
                // the live variable as defined if it was initialized, and then we
                // could check for uninit variables just by scanning what is live
                // at the start of the function. But that doesn't work so well for
                // immutable variables defined in a loop:
                //     loop { let x; x = 5; }
                // because the "assignment" loops back around and generates an error.
                //
                // So now we just check that variables defined w/o an
                // initializer are not live at the point of their
                // initialization, which is mildly more complex than checking
                // once at the func header but otherwise equivalent.

                let succ = self.propagate_through_opt_expr(local.init.as_deref(), succ);
                self.define_bindings_in_pat(&local.pat, succ)
            }
            hir::StmtKind::Item(..) => succ,
            hir::StmtKind::Expr(ref expr) | hir::StmtKind::Semi(ref expr) => {
                self.propagate_through_expr(&expr, succ)
            }
        }
    }

    fn propagate_through_exprs(&mut self, exprs: &[Expr<'_>], succ: LiveNode) -> LiveNode {
        exprs.iter().rev().fold(succ, |succ, expr| self.propagate_through_expr(&expr, succ))
    }

    fn propagate_through_opt_expr(
        &mut self,
        opt_expr: Option<&Expr<'_>>,
        succ: LiveNode,
    ) -> LiveNode {
        opt_expr.map_or(succ, |expr| self.propagate_through_expr(expr, succ))
    }

    fn propagate_through_expr(&mut self, expr: &Expr<'_>, succ: LiveNode) -> LiveNode {
        debug!("propagate_through_expr: {:?}", expr);

        match expr.kind {
            // Interesting cases with control flow or which gen/kill
            hir::ExprKind::Path(hir::QPath::Resolved(_, ref path)) => {
                self.access_path(expr.hir_id, path, succ, ACC_READ | ACC_USE)
            }

            hir::ExprKind::Field(ref e, _) => self.propagate_through_expr(&e, succ),

            hir::ExprKind::Closure(..) => {
                debug!("{:?} is an ExprKind::Closure", expr);

                // the construction of a closure itself is not important,
                // but we have to consider the closed over variables.
                let caps = self
                    .ir
                    .capture_info_map
                    .get(&expr.hir_id)
                    .cloned()
                    .unwrap_or_else(|| span_bug!(expr.span, "no registered caps"));

                caps.iter().rev().fold(succ, |succ, cap| {
                    self.init_from_succ(cap.ln, succ);
                    let var = self.variable(cap.var_hid, expr.span);
                    self.acc(cap.ln, var, ACC_READ | ACC_USE);
                    cap.ln
                })
            }

            // Note that labels have been resolved, so we don't need to look
            // at the label ident
            hir::ExprKind::Loop(ref blk, _, _) => self.propagate_through_loop(expr, &blk, succ),

            hir::ExprKind::Match(ref e, arms, _) => {
                //
                //      (e)
                //       |
                //       v
                //     (expr)
                //     / | \
                //    |  |  |
                //    v  v  v
                //   (..arms..)
                //    |  |  |
                //    v  v  v
                //   (  succ  )
                //
                //
                let ln = self.live_node(expr.hir_id, expr.span);
                self.init_empty(ln, succ);
                let mut first_merge = true;
                for arm in arms {
                    let body_succ = self.propagate_through_expr(&arm.body, succ);

                    let guard_succ = self.propagate_through_opt_expr(
                        arm.guard.as_ref().map(|hir::Guard::If(e)| *e),
                        body_succ,
                    );
                    let arm_succ = self.define_bindings_in_pat(&arm.pat, guard_succ);
                    self.merge_from_succ(ln, arm_succ, first_merge);
                    first_merge = false;
                }
                self.propagate_through_expr(&e, ln)
            }

            hir::ExprKind::Ret(ref o_e) => {
                // Ignore succ and subst exit_ln.
                self.propagate_through_opt_expr(o_e.as_ref().map(|e| &**e), self.exit_ln)
            }

            hir::ExprKind::Break(label, ref opt_expr) => {
                // Find which label this break jumps to
                let target = match label.target_id {
                    Ok(hir_id) => self.break_ln.get(&hir_id),
                    Err(err) => span_bug!(expr.span, "loop scope error: {}", err),
                }
                .cloned();

                // Now that we know the label we're going to,
                // look it up in the break loop nodes table

                match target {
                    Some(b) => self.propagate_through_opt_expr(opt_expr.as_ref().map(|e| &**e), b),
                    None => span_bug!(expr.span, "`break` to unknown label"),
                }
            }

            hir::ExprKind::Continue(label) => {
                // Find which label this expr continues to
                let sc = label
                    .target_id
                    .unwrap_or_else(|err| span_bug!(expr.span, "loop scope error: {}", err));

                // Now that we know the label we're going to,
                // look it up in the continue loop nodes table
                self.cont_ln
                    .get(&sc)
                    .cloned()
                    .unwrap_or_else(|| span_bug!(expr.span, "continue to unknown label"))
            }

            hir::ExprKind::Assign(ref l, ref r, _) => {
                // see comment on places in
                // propagate_through_place_components()
                let succ = self.write_place(&l, succ, ACC_WRITE);
                let succ = self.propagate_through_place_components(&l, succ);
                self.propagate_through_expr(&r, succ)
            }

            hir::ExprKind::AssignOp(_, ref l, ref r) => {
                // an overloaded assign op is like a method call
                if self.typeck_results.is_method_call(expr) {
                    let succ = self.propagate_through_expr(&l, succ);
                    self.propagate_through_expr(&r, succ)
                } else {
                    // see comment on places in
                    // propagate_through_place_components()
                    let succ = self.write_place(&l, succ, ACC_WRITE | ACC_READ);
                    let succ = self.propagate_through_expr(&r, succ);
                    self.propagate_through_place_components(&l, succ)
                }
            }

            // Uninteresting cases: just propagate in rev exec order
            hir::ExprKind::Array(ref exprs) => self.propagate_through_exprs(exprs, succ),

            hir::ExprKind::Struct(_, ref fields, ref with_expr) => {
                let succ = self.propagate_through_opt_expr(with_expr.as_ref().map(|e| &**e), succ);
                fields
                    .iter()
                    .rev()
                    .fold(succ, |succ, field| self.propagate_through_expr(&field.expr, succ))
            }

            hir::ExprKind::Call(ref f, ref args) => {
                let m = self.ir.tcx.parent_module(expr.hir_id).to_def_id();
                let succ = if self.ir.tcx.is_ty_uninhabited_from(
                    m,
                    self.typeck_results.expr_ty(expr),
                    self.param_env,
                ) {
                    self.exit_ln
                } else {
                    succ
                };
                let succ = self.propagate_through_exprs(args, succ);
                self.propagate_through_expr(&f, succ)
            }

            hir::ExprKind::MethodCall(.., ref args, _) => {
                let m = self.ir.tcx.parent_module(expr.hir_id).to_def_id();
                let succ = if self.ir.tcx.is_ty_uninhabited_from(
                    m,
                    self.typeck_results.expr_ty(expr),
                    self.param_env,
                ) {
                    self.exit_ln
                } else {
                    succ
                };

                self.propagate_through_exprs(args, succ)
            }

            hir::ExprKind::Tup(ref exprs) => self.propagate_through_exprs(exprs, succ),

            hir::ExprKind::Binary(op, ref l, ref r) if op.node.is_lazy() => {
                let r_succ = self.propagate_through_expr(&r, succ);

                let ln = self.live_node(expr.hir_id, expr.span);
                self.init_from_succ(ln, succ);
                self.merge_from_succ(ln, r_succ, false);

                self.propagate_through_expr(&l, ln)
            }

            hir::ExprKind::Index(ref l, ref r) | hir::ExprKind::Binary(_, ref l, ref r) => {
                let r_succ = self.propagate_through_expr(&r, succ);
                self.propagate_through_expr(&l, r_succ)
            }

            hir::ExprKind::Box(ref e)
            | hir::ExprKind::AddrOf(_, _, ref e)
            | hir::ExprKind::Cast(ref e, _)
            | hir::ExprKind::Type(ref e, _)
            | hir::ExprKind::DropTemps(ref e)
            | hir::ExprKind::Unary(_, ref e)
            | hir::ExprKind::Yield(ref e, _)
            | hir::ExprKind::Repeat(ref e, _) => self.propagate_through_expr(&e, succ),

            hir::ExprKind::InlineAsm(ref asm) => {
                // Handle non-returning asm
                let mut succ = if asm.options.contains(InlineAsmOptions::NORETURN) {
                    self.exit_ln
                } else {
                    succ
                };

                // Do a first pass for writing outputs only
                for op in asm.operands.iter().rev() {
                    match op {
                        hir::InlineAsmOperand::In { .. }
                        | hir::InlineAsmOperand::Const { .. }
                        | hir::InlineAsmOperand::Sym { .. } => {}
                        hir::InlineAsmOperand::Out { expr, .. } => {
                            if let Some(expr) = expr {
                                succ = self.write_place(expr, succ, ACC_WRITE);
                            }
                        }
                        hir::InlineAsmOperand::InOut { expr, .. } => {
                            succ = self.write_place(expr, succ, ACC_READ | ACC_WRITE | ACC_USE);
                        }
                        hir::InlineAsmOperand::SplitInOut { out_expr, .. } => {
                            if let Some(expr) = out_expr {
                                succ = self.write_place(expr, succ, ACC_WRITE);
                            }
                        }
                    }
                }

                // Then do a second pass for inputs
                let mut succ = succ;
                for op in asm.operands.iter().rev() {
                    match op {
                        hir::InlineAsmOperand::In { expr, .. }
                        | hir::InlineAsmOperand::Const { expr, .. }
                        | hir::InlineAsmOperand::Sym { expr, .. } => {
                            succ = self.propagate_through_expr(expr, succ)
                        }
                        hir::InlineAsmOperand::Out { expr, .. } => {
                            if let Some(expr) = expr {
                                succ = self.propagate_through_place_components(expr, succ);
                            }
                        }
                        hir::InlineAsmOperand::InOut { expr, .. } => {
                            succ = self.propagate_through_place_components(expr, succ);
                        }
                        hir::InlineAsmOperand::SplitInOut { in_expr, out_expr, .. } => {
                            if let Some(expr) = out_expr {
                                succ = self.propagate_through_place_components(expr, succ);
                            }
                            succ = self.propagate_through_expr(in_expr, succ);
                        }
                    }
                }
                succ
            }

            hir::ExprKind::LlvmInlineAsm(ref asm) => {
                let ia = &asm.inner;
                let outputs = asm.outputs_exprs;
                let inputs = asm.inputs_exprs;
                let succ = ia.outputs.iter().zip(outputs).rev().fold(succ, |succ, (o, output)| {
                    // see comment on places
                    // in propagate_through_place_components()
                    if o.is_indirect {
                        self.propagate_through_expr(output, succ)
                    } else {
                        let acc = if o.is_rw { ACC_WRITE | ACC_READ } else { ACC_WRITE };
                        let succ = self.write_place(output, succ, acc);
                        self.propagate_through_place_components(output, succ)
                    }
                });

                // Inputs are executed first. Propagate last because of rev order
                self.propagate_through_exprs(inputs, succ)
            }

            hir::ExprKind::Lit(..)
            | hir::ExprKind::ConstBlock(..)
            | hir::ExprKind::Err
            | hir::ExprKind::Path(hir::QPath::TypeRelative(..))
            | hir::ExprKind::Path(hir::QPath::LangItem(..)) => succ,

            // Note that labels have been resolved, so we don't need to look
            // at the label ident
            hir::ExprKind::Block(ref blk, _) => self.propagate_through_block(&blk, succ),
        }
    }

    fn propagate_through_place_components(&mut self, expr: &Expr<'_>, succ: LiveNode) -> LiveNode {
        // # Places
        //
        // In general, the full flow graph structure for an
        // assignment/move/etc can be handled in one of two ways,
        // depending on whether what is being assigned is a "tracked
        // value" or not. A tracked value is basically a local
        // variable or argument.
        //
        // The two kinds of graphs are:
        //
        //    Tracked place          Untracked place
        // ----------------------++-----------------------
        //                       ||
        //         |             ||           |
        //         v             ||           v
        //     (rvalue)          ||       (rvalue)
        //         |             ||           |
        //         v             ||           v
        // (write of place)     ||   (place components)
        //         |             ||           |
        //         v             ||           v
        //      (succ)           ||        (succ)
        //                       ||
        // ----------------------++-----------------------
        //
        // I will cover the two cases in turn:
        //
        // # Tracked places
        //
        // A tracked place is a local variable/argument `x`.  In
        // these cases, the link_node where the write occurs is linked
        // to node id of `x`.  The `write_place()` routine generates
        // the contents of this node.  There are no subcomponents to
        // consider.
        //
        // # Non-tracked places
        //
        // These are places like `x[5]` or `x.f`.  In that case, we
        // basically ignore the value which is written to but generate
        // reads for the components---`x` in these two examples.  The
        // components reads are generated by
        // `propagate_through_place_components()` (this fn).
        //
        // # Illegal places
        //
        // It is still possible to observe assignments to non-places;
        // these errors are detected in the later pass borrowck.  We
        // just ignore such cases and treat them as reads.

        match expr.kind {
            hir::ExprKind::Path(_) => succ,
            hir::ExprKind::Field(ref e, _) => self.propagate_through_expr(&e, succ),
            _ => self.propagate_through_expr(expr, succ),
        }
    }

    // see comment on propagate_through_place()
    fn write_place(&mut self, expr: &Expr<'_>, succ: LiveNode, acc: u32) -> LiveNode {
        match expr.kind {
            hir::ExprKind::Path(hir::QPath::Resolved(_, ref path)) => {
                self.access_path(expr.hir_id, path, succ, acc)
            }

            // We do not track other places, so just propagate through
            // to their subcomponents.  Also, it may happen that
            // non-places occur here, because those are detected in the
            // later pass borrowck.
            _ => succ,
        }
    }

    fn access_var(
        &mut self,
        hir_id: HirId,
        var_hid: HirId,
        succ: LiveNode,
        acc: u32,
        span: Span,
    ) -> LiveNode {
        let ln = self.live_node(hir_id, span);
        if acc != 0 {
            self.init_from_succ(ln, succ);
            let var = self.variable(var_hid, span);
            self.acc(ln, var, acc);
        }
        ln
    }

    fn access_path(
        &mut self,
        hir_id: HirId,
        path: &hir::Path<'_>,
        succ: LiveNode,
        acc: u32,
    ) -> LiveNode {
        match path.res {
            Res::Local(hid) => self.access_var(hir_id, hid, succ, acc, path.span),
            _ => succ,
        }
    }

    fn propagate_through_loop(
        &mut self,
        expr: &Expr<'_>,
        body: &hir::Block<'_>,
        succ: LiveNode,
    ) -> LiveNode {
        /*
        We model control flow like this:

              (expr) <-+
                |      |
                v      |
              (body) --+

        Note that a `continue` expression targeting the `loop` will have a successor of `expr`.
        Meanwhile, a `break` expression will have a successor of `succ`.
        */

        // first iteration:
        let mut first_merge = true;
        let ln = self.live_node(expr.hir_id, expr.span);
        self.init_empty(ln, succ);
        debug!("propagate_through_loop: using id for loop body {} {:?}", expr.hir_id, body);

        self.break_ln.insert(expr.hir_id, succ);

        self.cont_ln.insert(expr.hir_id, ln);

        let body_ln = self.propagate_through_block(body, ln);

        // repeat until fixed point is reached:
        while self.merge_from_succ(ln, body_ln, first_merge) {
            first_merge = false;
            assert_eq!(body_ln, self.propagate_through_block(body, ln));
        }

        ln
    }
}

// _______________________________________________________________________
// Checking for error conditions

impl<'a, 'tcx> Visitor<'tcx> for Liveness<'a, 'tcx> {
    type Map = intravisit::ErasedMap<'tcx>;

    fn nested_visit_map(&mut self) -> NestedVisitorMap<Self::Map> {
        NestedVisitorMap::None
    }

    fn visit_local(&mut self, local: &'tcx hir::Local<'tcx>) {
        self.check_unused_vars_in_pat(&local.pat, None, |spans, hir_id, ln, var| {
            if local.init.is_some() {
                self.warn_about_dead_assign(spans, hir_id, ln, var);
            }
        });

        intravisit::walk_local(self, local);
    }

    fn visit_expr(&mut self, ex: &'tcx Expr<'tcx>) {
        check_expr(self, ex);
    }

    fn visit_arm(&mut self, arm: &'tcx hir::Arm<'tcx>) {
        self.check_unused_vars_in_pat(&arm.pat, None, |_, _, _, _| {});
        intravisit::walk_arm(self, arm);
    }
}

fn check_expr<'tcx>(this: &mut Liveness<'_, 'tcx>, expr: &'tcx Expr<'tcx>) {
    match expr.kind {
        hir::ExprKind::Assign(ref l, ..) => {
            this.check_place(&l);
        }

        hir::ExprKind::AssignOp(_, ref l, _) => {
            if !this.typeck_results.is_method_call(expr) {
                this.check_place(&l);
            }
        }

        hir::ExprKind::InlineAsm(ref asm) => {
            for op in asm.operands {
                match op {
                    hir::InlineAsmOperand::Out { expr, .. } => {
                        if let Some(expr) = expr {
                            this.check_place(expr);
                        }
                    }
                    hir::InlineAsmOperand::InOut { expr, .. } => {
                        this.check_place(expr);
                    }
                    hir::InlineAsmOperand::SplitInOut { out_expr, .. } => {
                        if let Some(out_expr) = out_expr {
                            this.check_place(out_expr);
                        }
                    }
                    _ => {}
                }
            }
        }

        hir::ExprKind::LlvmInlineAsm(ref asm) => {
            for input in asm.inputs_exprs {
                this.visit_expr(input);
            }

            // Output operands must be places
            for (o, output) in asm.inner.outputs.iter().zip(asm.outputs_exprs) {
                if !o.is_indirect {
                    this.check_place(output);
                }
                this.visit_expr(output);
            }
        }

        // no correctness conditions related to liveness
        hir::ExprKind::Call(..)
        | hir::ExprKind::MethodCall(..)
        | hir::ExprKind::Match(..)
        | hir::ExprKind::Loop(..)
        | hir::ExprKind::Index(..)
        | hir::ExprKind::Field(..)
        | hir::ExprKind::Array(..)
        | hir::ExprKind::Tup(..)
        | hir::ExprKind::Binary(..)
        | hir::ExprKind::Cast(..)
        | hir::ExprKind::DropTemps(..)
        | hir::ExprKind::Unary(..)
        | hir::ExprKind::Ret(..)
        | hir::ExprKind::Break(..)
        | hir::ExprKind::Continue(..)
        | hir::ExprKind::Lit(_)
        | hir::ExprKind::ConstBlock(..)
        | hir::ExprKind::Block(..)
        | hir::ExprKind::AddrOf(..)
        | hir::ExprKind::Struct(..)
        | hir::ExprKind::Repeat(..)
        | hir::ExprKind::Closure(..)
        | hir::ExprKind::Path(_)
        | hir::ExprKind::Yield(..)
        | hir::ExprKind::Box(..)
        | hir::ExprKind::Type(..)
        | hir::ExprKind::Err => {}
    }

    intravisit::walk_expr(this, expr);
}

impl<'tcx> Liveness<'_, 'tcx> {
    fn check_place(&mut self, expr: &'tcx Expr<'tcx>) {
        match expr.kind {
            hir::ExprKind::Path(hir::QPath::Resolved(_, ref path)) => {
                if let Res::Local(var_hid) = path.res {
                    // Assignment to an immutable variable or argument: only legal
                    // if there is no later assignment. If this local is actually
                    // mutable, then check for a reassignment to flag the mutability
                    // as being used.
                    let ln = self.live_node(expr.hir_id, expr.span);
                    let var = self.variable(var_hid, expr.span);
                    self.warn_about_dead_assign(vec![expr.span], expr.hir_id, ln, var);
                }
            }
            _ => {
                // For other kinds of places, no checks are required,
                // and any embedded expressions are actually rvalues
                intravisit::walk_expr(self, expr);
            }
        }
    }

    fn should_warn(&self, var: Variable) -> Option<String> {
        let name = self.ir.variable_name(var);
        if name == kw::Invalid {
            return None;
        }
        let name: &str = &name.as_str();
        if name.as_bytes()[0] == b'_' {
            return None;
        }
        Some(name.to_owned())
    }

    fn warn_about_unused_upvars(&self, entry_ln: LiveNode) {
        let upvars = match self.upvars {
            None => return,
            Some(upvars) => upvars,
        };
        for (&var_hir_id, upvar) in upvars.iter() {
            let var = self.variable(var_hir_id, upvar.span);
            let upvar_id = ty::UpvarId {
                var_path: ty::UpvarPath { hir_id: var_hir_id },
                closure_expr_id: self.body_owner,
            };
            match self.typeck_results.upvar_capture(upvar_id) {
                ty::UpvarCapture::ByValue(_) => {}
                ty::UpvarCapture::ByRef(..) => continue,
            };
            if self.used_on_entry(entry_ln, var) {
                if self.live_on_entry(entry_ln, var).is_none() {
                    if let Some(name) = self.should_warn(var) {
                        self.ir.tcx.struct_span_lint_hir(
                            lint::builtin::UNUSED_ASSIGNMENTS,
                            var_hir_id,
                            vec![upvar.span],
                            |lint| {
                                lint.build(&format!("value captured by `{}` is never read", name))
                                    .help("did you mean to capture by reference instead?")
                                    .emit();
                            },
                        );
                    }
                }
            } else {
                if let Some(name) = self.should_warn(var) {
                    self.ir.tcx.struct_span_lint_hir(
                        lint::builtin::UNUSED_VARIABLES,
                        var_hir_id,
                        vec![upvar.span],
                        |lint| {
                            lint.build(&format!("unused variable: `{}`", name))
                                .help("did you mean to capture by reference instead?")
                                .emit();
                        },
                    );
                }
            }
        }
    }

    fn warn_about_unused_args(&self, body: &hir::Body<'_>, entry_ln: LiveNode) {
        for p in body.params {
            self.check_unused_vars_in_pat(&p.pat, Some(entry_ln), |spans, hir_id, ln, var| {
                if self.live_on_entry(ln, var).is_none() {
                    self.report_unsed_assign(hir_id, spans, var, |name| {
                        format!("value passed to `{}` is never read", name)
                    });
                }
            });
        }
    }

    fn check_unused_vars_in_pat(
        &self,
        pat: &hir::Pat<'_>,
        entry_ln: Option<LiveNode>,
        on_used_on_entry: impl Fn(Vec<Span>, HirId, LiveNode, Variable),
    ) {
        // In an or-pattern, only consider the variable; any later patterns must have the same
        // bindings, and we also consider the first pattern to be the "authoritative" set of ids.
        // However, we should take the ids and spans of variables with the same name from the later
        // patterns so the suggestions to prefix with underscores will apply to those too.
        let mut vars: FxIndexMap<Symbol, (LiveNode, Variable, Vec<(HirId, Span)>)> = <_>::default();

        pat.each_binding(|_, hir_id, pat_sp, ident| {
            let ln = entry_ln.unwrap_or_else(|| self.live_node(hir_id, pat_sp));
            let var = self.variable(hir_id, ident.span);
            let id_and_sp = (hir_id, pat_sp);
            vars.entry(self.ir.variable_name(var))
                .and_modify(|(.., hir_ids_and_spans)| hir_ids_and_spans.push(id_and_sp))
                .or_insert_with(|| (ln, var, vec![id_and_sp]));
        });

        for (_, (ln, var, hir_ids_and_spans)) in vars {
            if self.used_on_entry(ln, var) {
                let id = hir_ids_and_spans[0].0;
                let spans = hir_ids_and_spans.into_iter().map(|(_, sp)| sp).collect();
                on_used_on_entry(spans, id, ln, var);
            } else {
                self.report_unused(hir_ids_and_spans, ln, var);
            }
        }
    }

    fn report_unused(&self, hir_ids_and_spans: Vec<(HirId, Span)>, ln: LiveNode, var: Variable) {
        let first_hir_id = hir_ids_and_spans[0].0;

        if let Some(name) = self.should_warn(var).filter(|name| name != "self") {
            // annoying: for parameters in funcs like `fn(x: i32)
            // {ret}`, there is only one node, so asking about
            // assigned_on_exit() is not meaningful.
            let is_assigned =
                if ln == self.exit_ln { false } else { self.assigned_on_exit(ln, var).is_some() };

            if is_assigned {
                self.ir.tcx.struct_span_lint_hir(
                    lint::builtin::UNUSED_VARIABLES,
                    first_hir_id,
                    hir_ids_and_spans.into_iter().map(|(_, sp)| sp).collect::<Vec<_>>(),
                    |lint| {
                        lint.build(&format!("variable `{}` is assigned to, but never used", name))
                            .note(&format!("consider using `_{}` instead", name))
                            .emit();
                    },
                )
            } else {
                self.ir.tcx.struct_span_lint_hir(
                    lint::builtin::UNUSED_VARIABLES,
                    first_hir_id,
                    hir_ids_and_spans.iter().map(|(_, sp)| *sp).collect::<Vec<_>>(),
                    |lint| {
                        let mut err = lint.build(&format!("unused variable: `{}`", name));

                        let (shorthands, non_shorthands): (Vec<_>, Vec<_>) =
                            hir_ids_and_spans.into_iter().partition(|(hir_id, span)| {
                                let var = self.variable(*hir_id, *span);
                                self.ir.variable_is_shorthand(var)
                            });

                        let mut shorthands = shorthands
                            .into_iter()
                            .map(|(_, span)| (span, format!("{}: _", name)))
                            .collect::<Vec<_>>();

                        // If we have both shorthand and non-shorthand, prefer the "try ignoring
                        // the field" message, and suggest `_` for the non-shorthands. If we only
                        // have non-shorthand, then prefix with an underscore instead.
                        if !shorthands.is_empty() {
                            shorthands.extend(
                                non_shorthands
                                    .into_iter()
                                    .map(|(_, span)| (span, "_".to_string()))
                                    .collect::<Vec<_>>(),
                            );

                            err.multipart_suggestion(
                                "try ignoring the field",
                                shorthands,
                                Applicability::MachineApplicable,
                            );
                        } else {
                            err.multipart_suggestion(
                                "if this is intentional, prefix it with an underscore",
                                non_shorthands
                                    .into_iter()
                                    .map(|(_, span)| (span, format!("_{}", name)))
                                    .collect::<Vec<_>>(),
                                Applicability::MachineApplicable,
                            );
                        }

                        err.emit()
                    },
                );
            }
        }
    }

    fn warn_about_dead_assign(&self, spans: Vec<Span>, hir_id: HirId, ln: LiveNode, var: Variable) {
        if self.live_on_exit(ln, var).is_none() {
            self.report_unsed_assign(hir_id, spans, var, |name| {
                format!("value assigned to `{}` is never read", name)
            });
        }
    }

    fn report_unsed_assign(
        &self,
        hir_id: HirId,
        spans: Vec<Span>,
        var: Variable,
        message: impl Fn(&str) -> String,
    ) {
        if let Some(name) = self.should_warn(var) {
            self.ir.tcx.struct_span_lint_hir(
                lint::builtin::UNUSED_ASSIGNMENTS,
                hir_id,
                spans,
                |lint| {
                    lint.build(&message(&name))
                        .help("maybe it is overwritten before being read?")
                        .emit();
                },
            )
        }
    }
}
