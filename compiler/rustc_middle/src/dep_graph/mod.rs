//! Nodes in the dependency graph.
//!
//! A node in the dependency graph is represented by a [`DepNode`].[^1]
//! A `DepNode` consists of a [`DepKind`] (which
//! specifies the kind of thing it represents, like a piece of HIR, MIR, etc)
//! and a [`Fingerprint`], a 128-bit hash value the exact meaning of which
//! depends on the node's `DepKind`. Together, the kind and the fingerprint
//! fully identify a dependency node, even across multiple compilation sessions.
//! In other words, the value of the fingerprint does not depend on anything
//! that is specific to a given compilation session, like an unpredictable
//! interning key (e.g., `NodeId`, `DefId`, `Symbol`) or the numeric value of a
//! pointer. The concept behind this could be compared to how git commit hashes
//! uniquely identify a given commit.
//!
//! The fingerprinting approach and has some benefits:
//!
//! * A `DepNode` can simply be serialized to disk and loaded in another session
//!   without the need to do any "rebasing" (like we have to do for Spans and
//!   NodeIds) or "retracing" (like we had to do for `DefId` in earlier
//!   implementations of the dependency graph).
//! * A `Fingerprint` is just a bunch of bits, which allows `DepNode` to
//!   implement `Copy`, `Sync`, `Send`, `Freeze`, etc.
//! * Since we just have a bit pattern, `DepNode` can be mapped from disk into
//!   memory without any post-processing (e.g., "abomination-style" pointer
//!   reconstruction).
//! * Because a `DepNode` is self-contained, we can instantiate `DepNodes` that
//!   refer to things that do not exist anymore. In previous implementations
//!   `DepNode` contained a `DefId`. A `DepNode` referring to something that
//!   had been removed between the previous and the current compilation session
//!   could not be instantiated because the current compilation session
//!   contained no `DefId` for thing that had been removed.
//!
//! ## `DepNode` creation and "inference"
//!
// Where is the `define_dep_nodes!()` macro?
// How much of this section is outdated?
//! `DepNode` definition happens in the `define_dep_nodes!()` macro. This macro
//! defines the `DepKind` enum and a corresponding [`DepConstructor`] enum. The
//! `DepConstructor` enum links a `DepKind` to the parameters that are needed at
//! runtime in order to construct a valid `DepNode` fingerprint.
//!
//! Because the macro sees what parameters a given `DepKind` requires, it can
//! "infer" some properties for each kind of `DepNode`:
//!
//! * Whether a `DepNode` of a given kind has any parameters at all. Some
//!   `DepNode`s could represent global concepts with only one value.
//! * Whether it is possible, in principle, to reconstruct a query key from a
//!   given `DepNode`. Many `DepKind`s only require a single `DefId` parameter,
//!   in which case it is possible to map the node's fingerprint back to the
//!   `DefId` it was computed from. In other cases, too much information gets
//!   lost during fingerprint computation.
//!
//! The `DepConstructor` enum, together with `DepNode::new()`, ensures that only
//! valid `DepNode` instances can be constructed. For example, the API does not
//! allow for constructing parameterless `DepNode`s with anything other
//! than a zeroed out fingerprint. More generally speaking, it relieves the
//! user of the `DepNode` API of having to know how to compute the expected
//! fingerprint for a given set of node parameters.
//!
//! [^1]: For an overview of the dependency graph and its role in
//!   the query system and incremental compilation, see
//!   [the rustc-dev-guide](https://rustc-dev-guide.rust-lang.org/query.html).
//!
//! [`Fingerprint`]: rustc_data_structures::fingerprint::Fingerprint

use crate::ich::StableHashingContext;
use crate::ty::{self, TyCtxt};
use rustc_data_structures::profiling::SelfProfilerRef;
use rustc_data_structures::sync::Lock;
use rustc_data_structures::thin_vec::ThinVec;
use rustc_errors::Diagnostic;
use rustc_hir::def_id::LocalDefId;

mod dep_node;

pub use rustc_query_system::dep_graph::{
    debug, hash_result, DepContext, DepNodeColor, DepNodeIndex, SerializedDepNodeIndex,
    WorkProduct, WorkProductId,
};

crate use dep_node::make_compile_codegen_unit;
pub use dep_node::{label_strs, DepKind, DepNode, DepNodeExt};

pub type DepGraph = rustc_query_system::dep_graph::DepGraph<DepKind>;
pub type TaskDeps = rustc_query_system::dep_graph::TaskDeps<DepKind>;
pub type DepGraphQuery = rustc_query_system::dep_graph::DepGraphQuery<DepKind>;
pub type PreviousDepGraph = rustc_query_system::dep_graph::PreviousDepGraph<DepKind>;
pub type SerializedDepGraph = rustc_query_system::dep_graph::SerializedDepGraph<DepKind>;

impl rustc_query_system::dep_graph::DepKind for DepKind {
    const NULL: Self = DepKind::Null;

    #[inline(always)]
    fn can_reconstruct_query_key(&self) -> bool {
        DepKind::can_reconstruct_query_key(self)
    }

    #[inline(always)]
    fn is_eval_always(&self) -> bool {
        self.is_eval_always
    }

    #[inline(always)]
    fn has_params(&self) -> bool {
        self.has_params
    }

    fn debug_node(node: &DepNode, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", node.kind)?;

        if !node.kind.has_params && !node.kind.is_anon {
            return Ok(());
        }

        write!(f, "(")?;

        ty::tls::with_opt(|opt_tcx| {
            if let Some(tcx) = opt_tcx {
                if let Some(def_id) = node.extract_def_id(tcx) {
                    write!(f, "{}", tcx.def_path_debug_str(def_id))?;
                } else if let Some(ref s) = tcx.dep_graph.dep_node_debug_str(*node) {
                    write!(f, "{}", s)?;
                } else {
                    write!(f, "{}", node.hash)?;
                }
            } else {
                write!(f, "{}", node.hash)?;
            }
            Ok(())
        })?;

        write!(f, ")")
    }

    fn with_deps<OP, R>(task_deps: Option<&Lock<TaskDeps>>, op: OP) -> R
    where
        OP: FnOnce() -> R,
    {
        ty::tls::with_context(|icx| {
            let icx = ty::tls::ImplicitCtxt { task_deps, ..icx.clone() };

            ty::tls::enter_context(&icx, |_| op())
        })
    }

    fn read_deps<OP>(op: OP)
    where
        OP: for<'a> FnOnce(Option<&'a Lock<TaskDeps>>),
    {
        ty::tls::with_context_opt(|icx| {
            let icx = if let Some(icx) = icx { icx } else { return };
            op(icx.task_deps)
        })
    }
}

impl<'tcx> DepContext for TyCtxt<'tcx> {
    type DepKind = DepKind;
    type StableHashingContext = StableHashingContext<'tcx>;

    fn register_reused_dep_node(&self, dep_node: &DepNode) {
        if let Some(cache) = self.queries.on_disk_cache.as_ref() {
            cache.register_reused_dep_node(*self, dep_node)
        }
    }

    fn create_stable_hashing_context(&self) -> Self::StableHashingContext {
        TyCtxt::create_stable_hashing_context(*self)
    }

    fn debug_dep_tasks(&self) -> bool {
        self.sess.opts.debugging_opts.dep_tasks
    }
    fn debug_dep_node(&self) -> bool {
        self.sess.opts.debugging_opts.incremental_info
            || self.sess.opts.debugging_opts.query_dep_graph
    }

    fn try_force_from_dep_node(&self, dep_node: &DepNode) -> bool {
        // FIXME: This match is just a workaround for incremental bugs and should
        // be removed. https://github.com/rust-lang/rust/issues/62649 is one such
        // bug that must be fixed before removing this.
        match dep_node.kind {
            DepKind::hir_owner | DepKind::hir_owner_nodes => {
                if let Some(def_id) = dep_node.extract_def_id(*self) {
                    if !def_id_corresponds_to_hir_dep_node(*self, def_id.expect_local()) {
                        // This `DefPath` does not have a
                        // corresponding `DepNode` (e.g. a
                        // struct field), and the ` DefPath`
                        // collided with the `DefPath` of a
                        // proper item that existed in the
                        // previous compilation session.
                        //
                        // Since the given `DefPath` does not
                        // denote the item that previously
                        // existed, we just fail to mark green.
                        return false;
                    }
                } else {
                    // If the node does not exist anymore, we
                    // just fail to mark green.
                    return false;
                }
            }
            _ => {
                // For other kinds of nodes it's OK to be
                // forced.
            }
        }

        debug!("try_force_from_dep_node({:?}) --- trying to force", dep_node);

        // We must avoid ever having to call `force_from_dep_node()` for a
        // `DepNode::codegen_unit`:
        // Since we cannot reconstruct the query key of a `DepNode::codegen_unit`, we
        // would always end up having to evaluate the first caller of the
        // `codegen_unit` query that *is* reconstructible. This might very well be
        // the `compile_codegen_unit` query, thus re-codegenning the whole CGU just
        // to re-trigger calling the `codegen_unit` query with the right key. At
        // that point we would already have re-done all the work we are trying to
        // avoid doing in the first place.
        // The solution is simple: Just explicitly call the `codegen_unit` query for
        // each CGU, right after partitioning. This way `try_mark_green` will always
        // hit the cache instead of having to go through `force_from_dep_node`.
        // This assertion makes sure, we actually keep applying the solution above.
        debug_assert!(
            dep_node.kind != DepKind::codegen_unit,
            "calling force_from_dep_node() on DepKind::codegen_unit"
        );

        (dep_node.kind.force_from_dep_node)(*self, dep_node)
    }

    fn has_errors_or_delayed_span_bugs(&self) -> bool {
        self.sess.has_errors_or_delayed_span_bugs()
    }

    fn diagnostic(&self) -> &rustc_errors::Handler {
        self.sess.diagnostic()
    }

    // Interactions with on_disk_cache
    fn try_load_from_on_disk_cache(&self, dep_node: &DepNode) {
        (dep_node.kind.try_load_from_on_disk_cache)(*self, dep_node)
    }

    fn load_diagnostics(&self, prev_dep_node_index: SerializedDepNodeIndex) -> Vec<Diagnostic> {
        self.queries
            .on_disk_cache
            .as_ref()
            .map(|c| c.load_diagnostics(*self, prev_dep_node_index))
            .unwrap_or_default()
    }

    fn store_diagnostics(&self, dep_node_index: DepNodeIndex, diagnostics: ThinVec<Diagnostic>) {
        if let Some(c) = self.queries.on_disk_cache.as_ref() {
            c.store_diagnostics(dep_node_index, diagnostics)
        }
    }

    fn store_diagnostics_for_anon_node(
        &self,
        dep_node_index: DepNodeIndex,
        diagnostics: ThinVec<Diagnostic>,
    ) {
        if let Some(c) = self.queries.on_disk_cache.as_ref() {
            c.store_diagnostics_for_anon_node(dep_node_index, diagnostics)
        }
    }

    fn profiler(&self) -> &SelfProfilerRef {
        &self.prof
    }
}

fn def_id_corresponds_to_hir_dep_node(tcx: TyCtxt<'_>, def_id: LocalDefId) -> bool {
    let hir_id = tcx.hir().local_def_id_to_hir_id(def_id);
    def_id == hir_id.owner
}
