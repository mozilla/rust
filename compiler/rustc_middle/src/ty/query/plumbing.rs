//! The implementation of the query system itself. This defines the macros that
//! generate the actual methods on tcx which find and execute the provider,
//! manage the caches, and so forth.

use crate::dep_graph::{self, DepKind, DepNode, DepNodeExt, DepNodeIndex, SerializedDepNodeIndex};
use crate::ty::query::{on_disk_cache, queries, Queries, Query};
use crate::ty::tls::{self, ImplicitCtxt};
use crate::ty::{self, TyCtxt};
use rustc_query_system::dep_graph::HasDepContext;
use rustc_query_system::query::{CycleError, QueryJobId, QueryJobInfo};
use rustc_query_system::query::{QueryContext, QueryDescription};

use rustc_data_structures::fx::FxHashMap;
use rustc_data_structures::sync::Lock;
use rustc_data_structures::thin_vec::ThinVec;
use rustc_errors::{struct_span_err, Diagnostic, DiagnosticBuilder, Handler, Level};
use rustc_serialize::opaque;
use rustc_span::def_id::{DefId, LocalDefId};
use rustc_span::Span;
use std::borrow::Cow;

#[derive(Copy, Clone)]
pub struct QueryCtxt<'tcx> {
    pub tcx: TyCtxt<'tcx>,
    pub queries: &'tcx super::Queries<'tcx>,
}

impl<'tcx> std::ops::Deref for QueryCtxt<'tcx> {
    type Target = TyCtxt<'tcx>;

    fn deref(&self) -> &Self::Target {
        &self.tcx
    }
}

impl HasDepContext for QueryCtxt<'tcx> {
    type DepKind = crate::dep_graph::DepKind;
    type StableHashingContext = crate::ich::StableHashingContext<'tcx>;
    type DepContext = TyCtxt<'tcx>;

    #[inline]
    fn dep_context(&self) -> &Self::DepContext {
        &self.tcx
    }
}

impl QueryContext for QueryCtxt<'tcx> {
    type Query = Query<'tcx>;

    fn incremental_verify_ich(&self) -> bool {
        self.sess.opts.debugging_opts.incremental_verify_ich
    }
    fn verbose(&self) -> bool {
        self.sess.verbose()
    }

    fn def_path_str(&self, def_id: DefId) -> String {
        self.tcx.def_path_str(def_id)
    }

    fn current_query_job(&self) -> Option<QueryJobId<Self::DepKind>> {
        tls::with_related_context(**self, |icx| icx.query)
    }

    fn try_collect_active_jobs(
        &self,
    ) -> Option<FxHashMap<QueryJobId<Self::DepKind>, QueryJobInfo<Self::DepKind, Self::Query>>>
    {
        self.queries.try_collect_active_jobs()
    }

    fn try_load_from_on_disk_cache(&self, dep_node: &DepNode) {
        let cb = &super::QUERY_CALLBACKS[dep_node.kind as usize];
        (cb.try_load_from_on_disk_cache)(*self, dep_node)
    }

    fn try_force_from_dep_node(&self, dep_node: &DepNode) -> bool {
        // FIXME: This match is just a workaround for incremental bugs and should
        // be removed. https://github.com/rust-lang/rust/issues/62649 is one such
        // bug that must be fixed before removing this.
        match dep_node.kind {
            DepKind::hir_owner | DepKind::hir_owner_nodes => {
                if let Some(def_id) = dep_node.extract_def_id(**self) {
                    let def_id = def_id.expect_local();
                    let hir_id = self.tcx.hir().local_def_id_to_hir_id(def_id);
                    if def_id != hir_id.owner {
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

        let cb = &super::QUERY_CALLBACKS[dep_node.kind as usize];
        (cb.force_from_dep_node)(*self, dep_node)
    }

    fn has_errors_or_delayed_span_bugs(&self) -> bool {
        self.sess.has_errors_or_delayed_span_bugs()
    }

    fn diagnostic(&self) -> &rustc_errors::Handler {
        self.sess.diagnostic()
    }

    // Interactions with on_disk_cache
    fn load_diagnostics(&self, prev_dep_node_index: SerializedDepNodeIndex) -> Vec<Diagnostic> {
        self.on_disk_cache
            .as_ref()
            .map(|c| c.load_diagnostics(**self, prev_dep_node_index))
            .unwrap_or_default()
    }

    fn store_diagnostics(&self, dep_node_index: DepNodeIndex, diagnostics: ThinVec<Diagnostic>) {
        if let Some(c) = self.on_disk_cache.as_ref() {
            c.store_diagnostics(dep_node_index, diagnostics)
        }
    }

    fn store_diagnostics_for_anon_node(
        &self,
        dep_node_index: DepNodeIndex,
        diagnostics: ThinVec<Diagnostic>,
    ) {
        if let Some(c) = self.on_disk_cache.as_ref() {
            c.store_diagnostics_for_anon_node(dep_node_index, diagnostics)
        }
    }

    /// Executes a job by changing the `ImplicitCtxt` to point to the
    /// new query job while it executes. It returns the diagnostics
    /// captured during execution and the actual result.
    #[inline(always)]
    fn start_query<R>(
        &self,
        token: QueryJobId<Self::DepKind>,
        diagnostics: Option<&Lock<ThinVec<Diagnostic>>>,
        compute: impl FnOnce() -> R,
    ) -> R {
        // The `TyCtxt` stored in TLS has the same global interner lifetime
        // as `self`, so we use `with_related_context` to relate the 'tcx lifetimes
        // when accessing the `ImplicitCtxt`.
        tls::with_related_context(**self, move |current_icx| {
            // Update the `ImplicitCtxt` to point to our new query job.
            let new_icx = ImplicitCtxt {
                tcx: **self,
                query: Some(token),
                diagnostics,
                layout_depth: current_icx.layout_depth,
                task_deps: current_icx.task_deps,
            };

            // Use the `ImplicitCtxt` while we execute the query.
            tls::enter_context(&new_icx, |_| {
                rustc_data_structures::stack::ensure_sufficient_stack(compute)
            })
        })
    }
}

impl<'tcx> QueryCtxt<'tcx> {
    #[inline(never)]
    #[cold]
    pub(super) fn report_cycle(
        self,
        CycleError { usage, cycle: stack }: CycleError<Query<'tcx>>,
    ) -> DiagnosticBuilder<'tcx> {
        assert!(!stack.is_empty());

        let fix_span = |span: Span, query: &Query<'tcx>| {
            self.sess.source_map().guess_head_span(query.default_span(*self, span))
        };

        // Disable naming impls with types in this path, since that
        // sometimes cycles itself, leading to extra cycle errors.
        // (And cycle errors around impls tend to occur during the
        // collect/coherence phases anyhow.)
        ty::print::with_forced_impl_filename_line(|| {
            let span = fix_span(stack[1 % stack.len()].span, &stack[0].query);
            let mut err = struct_span_err!(
                self.sess,
                span,
                E0391,
                "cycle detected when {}",
                stack[0].query.describe(self)
            );

            for i in 1..stack.len() {
                let query = &stack[i].query;
                let span = fix_span(stack[(i + 1) % stack.len()].span, query);
                err.span_note(span, &format!("...which requires {}...", query.describe(self)));
            }

            err.note(&format!(
                "...which again requires {}, completing the cycle",
                stack[0].query.describe(self)
            ));

            if let Some((span, query)) = usage {
                err.span_note(
                    fix_span(span, &query),
                    &format!("cycle used when {}", query.describe(self)),
                );
            }

            err
        })
    }

    pub(super) fn encode_query_results(
        self,
        encoder: &mut on_disk_cache::CacheEncoder<'a, 'tcx, opaque::FileEncoder>,
        query_result_index: &mut on_disk_cache::EncodedQueryResultIndex,
    ) -> opaque::FileEncodeResult {
        macro_rules! encode_queries {
            ($($query:ident,)*) => {
                $(
                    on_disk_cache::encode_query_results::<ty::query::queries::$query<'_>>(
                        self,
                        encoder,
                        query_result_index
                    )?;
                )*
            }
        }

        rustc_cached_queries!(encode_queries!);

        Ok(())
    }
}

impl<'tcx> Queries<'tcx> {
    pub fn try_print_query_stack(
        &'tcx self,
        tcx: TyCtxt<'tcx>,
        query: Option<QueryJobId<dep_graph::DepKind>>,
        handler: &Handler,
        num_frames: Option<usize>,
    ) -> usize {
        let query_map = self.try_collect_active_jobs();

        let mut current_query = query;
        let mut i = 0;

        while let Some(query) = current_query {
            if Some(i) == num_frames {
                break;
            }
            let query_info = if let Some(info) = query_map.as_ref().and_then(|map| map.get(&query))
            {
                info
            } else {
                break;
            };
            let mut diag = Diagnostic::new(
                Level::FailureNote,
                &format!(
                    "#{} [{}] {}",
                    i,
                    query_info.info.query.name(),
                    query_info.info.query.describe(QueryCtxt { tcx, queries: self })
                ),
            );
            diag.span = tcx.sess.source_map().guess_head_span(query_info.info.span).into();
            handler.force_print_diagnostic(diag);

            current_query = query_info.job.parent;
            i += 1;
        }

        i
    }
}

/// This struct stores metadata about each Query.
///
/// Information is retrieved by indexing the `QUERIES` array using the integer value
/// of the `DepKind`. Overall, this allows to implement `QueryContext` using this manual
/// jump table instead of large matches.
pub struct QueryStruct {
    /// The red/green evaluation system will try to mark a specific DepNode in the
    /// dependency graph as green by recursively trying to mark the dependencies of
    /// that `DepNode` as green. While doing so, it will sometimes encounter a `DepNode`
    /// where we don't know if it is red or green and we therefore actually have
    /// to recompute its value in order to find out. Since the only piece of
    /// information that we have at that point is the `DepNode` we are trying to
    /// re-evaluate, we need some way to re-run a query from just that. This is what
    /// `force_from_dep_node()` implements.
    ///
    /// In the general case, a `DepNode` consists of a `DepKind` and an opaque
    /// GUID/fingerprint that will uniquely identify the node. This GUID/fingerprint
    /// is usually constructed by computing a stable hash of the query-key that the
    /// `DepNode` corresponds to. Consequently, it is not in general possible to go
    /// back from hash to query-key (since hash functions are not reversible). For
    /// this reason `force_from_dep_node()` is expected to fail from time to time
    /// because we just cannot find out, from the `DepNode` alone, what the
    /// corresponding query-key is and therefore cannot re-run the query.
    ///
    /// The system deals with this case letting `try_mark_green` fail which forces
    /// the root query to be re-evaluated.
    ///
    /// Now, if `force_from_dep_node()` would always fail, it would be pretty useless.
    /// Fortunately, we can use some contextual information that will allow us to
    /// reconstruct query-keys for certain kinds of `DepNode`s. In particular, we
    /// enforce by construction that the GUID/fingerprint of certain `DepNode`s is a
    /// valid `DefPathHash`. Since we also always build a huge table that maps every
    /// `DefPathHash` in the current codebase to the corresponding `DefId`, we have
    /// everything we need to re-run the query.
    ///
    /// Take the `mir_promoted` query as an example. Like many other queries, it
    /// just has a single parameter: the `DefId` of the item it will compute the
    /// validated MIR for. Now, when we call `force_from_dep_node()` on a `DepNode`
    /// with kind `MirValidated`, we know that the GUID/fingerprint of the `DepNode`
    /// is actually a `DefPathHash`, and can therefore just look up the corresponding
    /// `DefId` in `tcx.def_path_hash_to_def_id`.
    ///
    /// When you implement a new query, it will likely have a corresponding new
    /// `DepKind`, and you'll have to support it here in `force_from_dep_node()`. As
    /// a rule of thumb, if your query takes a `DefId` or `LocalDefId` as sole parameter,
    /// then `force_from_dep_node()` should not fail for it. Otherwise, you can just
    /// add it to the "We don't have enough information to reconstruct..." group in
    /// the match below.
    pub(crate) force_from_dep_node: fn(tcx: QueryCtxt<'_>, dep_node: &DepNode) -> bool,

    /// Invoke a query to put the on-disk cached value in memory.
    pub(crate) try_load_from_on_disk_cache: fn(QueryCtxt<'_>, &DepNode),
}

macro_rules! handle_cycle_error {
    ([][$tcx: expr, $error:expr]) => {{
        $tcx.report_cycle($error).emit();
        Value::from_cycle_error($tcx)
    }};
    ([fatal_cycle $($rest:tt)*][$tcx:expr, $error:expr]) => {{
        $tcx.report_cycle($error).emit();
        $tcx.sess.abort_if_errors();
        unreachable!()
    }};
    ([cycle_delay_bug $($rest:tt)*][$tcx:expr, $error:expr]) => {{
        $tcx.report_cycle($error).delay_as_bug();
        Value::from_cycle_error($tcx)
    }};
    ([$other:ident $(($($other_args:tt)*))* $(, $($modifiers:tt)*)*][$($args:tt)*]) => {
        handle_cycle_error!([$($($modifiers)*)*][$($args)*])
    };
}

macro_rules! is_anon {
    ([]) => {{
        false
    }};
    ([anon $($rest:tt)*]) => {{
        true
    }};
    ([$other:ident $(($($other_args:tt)*))* $(, $($modifiers:tt)*)*]) => {
        is_anon!([$($($modifiers)*)*])
    };
}

macro_rules! is_eval_always {
    ([]) => {{
        false
    }};
    ([eval_always $($rest:tt)*]) => {{
        true
    }};
    ([$other:ident $(($($other_args:tt)*))* $(, $($modifiers:tt)*)*]) => {
        is_eval_always!([$($($modifiers)*)*])
    };
}

macro_rules! query_storage {
    ([][$K:ty, $V:ty]) => {
        <DefaultCacheSelector as CacheSelector<$K, $V>>::Cache
    };
    ([storage($ty:ty) $($rest:tt)*][$K:ty, $V:ty]) => {
        <$ty as CacheSelector<$K, $V>>::Cache
    };
    ([$other:ident $(($($other_args:tt)*))* $(, $($modifiers:tt)*)*][$($args:tt)*]) => {
        query_storage!([$($($modifiers)*)*][$($args)*])
    };
}

macro_rules! hash_result {
    ([][$hcx:expr, $result:expr]) => {{
        dep_graph::hash_result($hcx, &$result)
    }};
    ([no_hash $($rest:tt)*][$hcx:expr, $result:expr]) => {{
        None
    }};
    ([$other:ident $(($($other_args:tt)*))* $(, $($modifiers:tt)*)*][$($args:tt)*]) => {
        hash_result!([$($($modifiers)*)*][$($args)*])
    };
}

macro_rules! define_queries {
    (<$tcx:tt>
     $($(#[$attr:meta])*
        [$($modifiers:tt)*] fn $name:ident($($K:tt)*) -> $V:ty,)*) => {

        use std::mem;
        use crate::{
            rustc_data_structures::stable_hasher::HashStable,
            rustc_data_structures::stable_hasher::StableHasher,
            ich::StableHashingContext
        };

        define_queries_struct! {
            tcx: $tcx,
            input: ($(([$($modifiers)*] [$($attr)*] [$name]))*)
        }

        #[allow(nonstandard_style)]
        #[derive(Clone, Debug)]
        pub enum Query<$tcx> {
            $($(#[$attr])* $name($($K)*)),*
        }

        impl<$tcx> Query<$tcx> {
            pub fn name(&self) -> &'static str {
                match *self {
                    $(Query::$name(_) => stringify!($name),)*
                }
            }

            pub(crate) fn describe(&self, tcx: QueryCtxt<$tcx>) -> Cow<'static, str> {
                let (r, name) = match *self {
                    $(Query::$name(key) => {
                        (queries::$name::describe(tcx, key), stringify!($name))
                    })*
                };
                if tcx.sess.verbose() {
                    format!("{} [{}]", r, name).into()
                } else {
                    r
                }
            }

            // FIXME(eddyb) Get more valid `Span`s on queries.
            pub fn default_span(&self, tcx: TyCtxt<$tcx>, span: Span) -> Span {
                if !span.is_dummy() {
                    return span;
                }
                // The `def_span` query is used to calculate `default_span`,
                // so exit to avoid infinite recursion.
                if let Query::def_span(..) = *self {
                    return span
                }
                match *self {
                    $(Query::$name(key) => key.default_span(tcx),)*
                }
            }
        }

        impl<'a, $tcx> HashStable<StableHashingContext<'a>> for Query<$tcx> {
            fn hash_stable(&self, hcx: &mut StableHashingContext<'a>, hasher: &mut StableHasher) {
                mem::discriminant(self).hash_stable(hcx, hasher);
                match *self {
                    $(Query::$name(key) => key.hash_stable(hcx, hasher),)*
                }
            }
        }

        #[allow(nonstandard_style)]
        pub mod queries {
            use std::marker::PhantomData;

            $(pub struct $name<$tcx> {
                data: PhantomData<&$tcx ()>
            })*
        }

        $(impl<$tcx> QueryConfig for queries::$name<$tcx> {
            type Key = $($K)*;
            type Value = $V;
            type Stored = query_stored::$name<$tcx>;
            const NAME: &'static str = stringify!($name);
        }

        impl<$tcx> QueryAccessors<QueryCtxt<$tcx>> for queries::$name<$tcx> {
            const ANON: bool = is_anon!([$($modifiers)*]);
            const EVAL_ALWAYS: bool = is_eval_always!([$($modifiers)*]);
            const DEP_KIND: dep_graph::DepKind = dep_graph::DepKind::$name;

            type Cache = query_storage!([$($modifiers)*][$($K)*, $V]);

            #[inline(always)]
            fn query_state<'a>(tcx: QueryCtxt<$tcx>) -> &'a QueryState<crate::dep_graph::DepKind, Query<$tcx>, Self::Cache> {
                &tcx.queries.$name
            }

            #[inline]
            fn compute(tcx: QueryCtxt<'tcx>, key: Self::Key) -> Self::Value {
                let provider = tcx.queries.providers.get(key.query_crate())
                    // HACK(eddyb) it's possible crates may be loaded after
                    // the query engine is created, and because crate loading
                    // is not yet integrated with the query engine, such crates
                    // would be missing appropriate entries in `providers`.
                    .unwrap_or(&tcx.queries.fallback_extern_providers)
                    .$name;
                provider(*tcx, key)
            }

            fn hash_result(
                _hcx: &mut StableHashingContext<'_>,
                _result: &Self::Value
            ) -> Option<Fingerprint> {
                hash_result!([$($modifiers)*][_hcx, _result])
            }

            fn handle_cycle_error(
                tcx: QueryCtxt<'tcx>,
                error: CycleError<Query<'tcx>>
            ) -> Self::Value {
                handle_cycle_error!([$($modifiers)*][tcx, error])
            }
        })*

        #[allow(non_upper_case_globals)]
        pub mod query_callbacks {
            use super::*;
            use crate::dep_graph::DepNode;
            use crate::ty::query::{queries, query_keys};
            use rustc_query_system::dep_graph::DepNodeParams;
            use rustc_query_system::query::{force_query, QueryDescription};

            // We use this for most things when incr. comp. is turned off.
            pub const Null: QueryStruct = QueryStruct {
                force_from_dep_node: |_, dep_node| bug!("force_from_dep_node: encountered {:?}", dep_node),
                try_load_from_on_disk_cache: |_, _| {},
            };

            pub const TraitSelect: QueryStruct = QueryStruct {
                force_from_dep_node: |_, _| false,
                try_load_from_on_disk_cache: |_, _| {},
            };

            pub const CompileCodegenUnit: QueryStruct = QueryStruct {
                force_from_dep_node: |_, _| false,
                try_load_from_on_disk_cache: |_, _| {},
            };

            $(pub const $name: QueryStruct = {
                const is_anon: bool = is_anon!([$($modifiers)*]);

                #[inline(always)]
                fn can_reconstruct_query_key() -> bool {
                    <query_keys::$name<'_> as DepNodeParams<TyCtxt<'_>>>
                        ::can_reconstruct_query_key()
                }

                fn recover<'tcx>(tcx: TyCtxt<'tcx>, dep_node: &DepNode) -> Option<query_keys::$name<'tcx>> {
                    <query_keys::$name<'_> as DepNodeParams<TyCtxt<'_>>>::recover(tcx, dep_node)
                }

                fn force_from_dep_node(tcx: QueryCtxt<'_>, dep_node: &DepNode) -> bool {
                    if is_anon {
                        return false;
                    }

                    if !can_reconstruct_query_key() {
                        return false;
                    }

                    if let Some(key) = recover(*tcx, dep_node) {
                        force_query::<queries::$name<'_>, _>(tcx, key, DUMMY_SP, *dep_node);
                        return true;
                    }

                    false
                }

                fn try_load_from_on_disk_cache(tcx: QueryCtxt<'_>, dep_node: &DepNode) {
                    if is_anon {
                        return
                    }

                    if !can_reconstruct_query_key() {
                        return
                    }

                    debug_assert!(tcx.dep_graph
                                     .node_color(dep_node)
                                     .map(|c| c.is_green())
                                     .unwrap_or(false));

                    let key = recover(*tcx, dep_node).unwrap_or_else(|| panic!("Failed to recover key for {:?} with hash {}", dep_node, dep_node.hash));
                    if queries::$name::cache_on_disk(tcx, &key, None) {
                        let _ = tcx.$name(key);
                    }
                }

                QueryStruct {
                    force_from_dep_node,
                    try_load_from_on_disk_cache,
                }
            };)*
        }

        static QUERY_CALLBACKS: &[QueryStruct] = &make_dep_kind_array!(query_callbacks);
    }
}

// FIXME(eddyb) this macro (and others?) use `$tcx` and `'tcx` interchangeably.
// We should either not take `$tcx` at all and use `'tcx` everywhere, or use
// `$tcx` everywhere (even if that isn't necessary due to lack of hygiene).
macro_rules! define_queries_struct {
    (tcx: $tcx:tt,
     input: ($(([$($modifiers:tt)*] [$($attr:tt)*] [$name:ident]))*)) => {
        pub struct Queries<$tcx> {
            providers: IndexVec<CrateNum, Providers>,
            fallback_extern_providers: Box<Providers>,

            $($(#[$attr])*  $name: QueryState<
                crate::dep_graph::DepKind,
                Query<$tcx>,
                <queries::$name<$tcx> as QueryAccessors<QueryCtxt<'tcx>>>::Cache,
            >,)*
        }

        impl<$tcx> Queries<$tcx> {
            pub fn new(
                providers: IndexVec<CrateNum, Providers>,
                fallback_extern_providers: Providers,
            ) -> Self {
                Queries {
                    providers,
                    fallback_extern_providers: Box::new(fallback_extern_providers),
                    $($name: Default::default()),*
                }
            }

            /// All self-profiling events generated by the query engine use
            /// virtual `StringId`s for their `event_id`. This method makes all
            /// those virtual `StringId`s point to actual strings.
            ///
            /// If we are recording only summary data, the ids will point to
            /// just the query names. If we are recording query keys too, we
            /// allocate the corresponding strings here.
            pub fn alloc_self_profile_query_strings(&self, tcx: TyCtxt<'tcx>) {
                use crate::ty::query::profiling_support::{
                    alloc_self_profile_query_strings_for_query_cache,
                    QueryKeyStringCache,
                };

                if !tcx.prof.enabled() {
                    return;
                }

                let mut string_cache = QueryKeyStringCache::new();

                $({
                    alloc_self_profile_query_strings_for_query_cache(
                        tcx,
                        stringify!($name),
                        &self.$name,
                        &mut string_cache,
                    );
                })*
            }

            pub(crate) fn try_collect_active_jobs(
                &self
            ) -> Option<FxHashMap<QueryJobId<crate::dep_graph::DepKind>, QueryJobInfo<crate::dep_graph::DepKind, Query<$tcx>>>> {
                let mut jobs = FxHashMap::default();

                $(
                    self.$name.try_collect_active_jobs(
                        <queries::$name<'tcx> as QueryAccessors<QueryCtxt<'tcx>>>::DEP_KIND,
                        Query::$name,
                        &mut jobs,
                    )?;
                )*

                Some(jobs)
            }

            #[cfg(parallel_compiler)]
            pub unsafe fn deadlock(&'tcx self, tcx: TyCtxt<'tcx>, registry: &rustc_rayon_core::Registry) {
                let tcx = QueryCtxt { tcx, queries: self };
                rustc_query_system::query::deadlock(tcx, registry)
            }

            pub(crate) fn encode_query_results(
                &'tcx self,
                tcx: TyCtxt<'tcx>,
                encoder: &mut on_disk_cache::CacheEncoder<'a, 'tcx, opaque::FileEncoder>,
                query_result_index: &mut on_disk_cache::EncodedQueryResultIndex,
            ) -> opaque::FileEncodeResult {
                let tcx = QueryCtxt { tcx, queries: self };
                tcx.encode_query_results(encoder, query_result_index)
            }

            fn exec_cache_promotions(&'tcx self, tcx: TyCtxt<'tcx>) {
                let tcx = QueryCtxt { tcx, queries: self };
                tcx.dep_graph.exec_cache_promotions(tcx)
            }

            $($(#[$attr])*
            #[inline(always)]
            fn $name(
                &'tcx self,
                tcx: TyCtxt<$tcx>,
                span: Span,
                key: query_keys::$name<$tcx>,
                mode: QueryMode,
            ) -> Option<query_stored::$name<$tcx>> {
                let qcx = QueryCtxt { tcx, queries: self };
                get_query::<queries::$name<$tcx>, _>(qcx, span, key, mode)
            })*
        }
    };
}

fn describe_as_module(def_id: LocalDefId, tcx: TyCtxt<'_>) -> String {
    if def_id.is_top_level_module() {
        "top-level module".to_string()
    } else {
        format!("module `{}`", tcx.def_path_str(def_id.to_def_id()))
    }
}

rustc_query_description! {}
