use crate::check::FnCtxt;
use rustc_data_structures::{
    fx::FxHashMap,
    graph::WithSuccessors,
    graph::{iterate::DepthFirstSearch, vec_graph::VecGraph},
    stable_set::FxHashSet,
};
use rustc_middle::traits;
use rustc_middle::ty::{self, ToPredicate, Ty, WithConstness};
use rustc_trait_selection::traits::query::evaluate_obligation::InferCtxtExt;

impl<'tcx> FnCtxt<'_, 'tcx> {
    pub(super) fn type_inference_fallback(&self) {
        // All type checking constraints were added, try to fallback unsolved variables.
        self.select_obligations_where_possible(false, |_| {});
        let mut fallback_has_occurred = false;

        // Check if we have any unsolved varibales. If not, no need for fallback.
        let unsolved_variables = self.unsolved_variables();
        if unsolved_variables.is_empty() {
            return;
        }

        let diverging_fallback = self.calculate_diverging_fallback(&unsolved_variables);

        // We do fallback in two passes, to try to generate
        // better error messages.
        // The first time, we do *not* replace opaque types.
        for ty in unsolved_variables {
            debug!("unsolved_variable = {:?}", ty);
            fallback_has_occurred |= self.fallback_if_possible(ty, &diverging_fallback);
        }

        // We now see if we can make progress. This might cause us to
        // unify inference variables for opaque types, since we may
        // have unified some other type variables during the first
        // phase of fallback.  This means that we only replace
        // inference variables with their underlying opaque types as a
        // last resort.
        //
        // In code like this:
        //
        // ```rust
        // type MyType = impl Copy;
        // fn produce() -> MyType { true }
        // fn bad_produce() -> MyType { panic!() }
        // ```
        //
        // we want to unify the opaque inference variable in `bad_produce`
        // with the diverging fallback for `panic!` (e.g. `()` or `!`).
        // This will produce a nice error message about conflicting concrete
        // types for `MyType`.
        //
        // If we had tried to fallback the opaque inference variable to `MyType`,
        // we will generate a confusing type-check error that does not explicitly
        // refer to opaque types.
        self.select_obligations_where_possible(fallback_has_occurred, |_| {});

        // We now run fallback again, but this time we allow it to replace
        // unconstrained opaque type variables, in addition to performing
        // other kinds of fallback.
        for ty in &self.unsolved_variables() {
            fallback_has_occurred |= self.fallback_opaque_type_vars(ty);
        }

        // See if we can make any more progress.
        self.select_obligations_where_possible(fallback_has_occurred, |_| {});
    }

    // Tries to apply a fallback to `ty` if it is an unsolved variable.
    //
    // - Unconstrained ints are replaced with `i32`.
    //
    // - Unconstrained floats are replaced with with `f64`.
    //
    // - Non-numerics may get replaced with `()` or `!`, depending on
    //   how they were categorized by `calculate_diverging_fallback`.
    //
    // Fallback becomes very dubious if we have encountered
    // type-checking errors.  In that case, fallback to Error.
    //
    // The return value indicates whether fallback has occurred.
    fn fallback_if_possible(
        &self,
        ty: Ty<'tcx>,
        diverging_fallback: &FxHashMap<Ty<'tcx>, Ty<'tcx>>,
    ) -> bool {
        // Careful: we do NOT shallow-resolve `ty`. We know that `ty`
        // is an unsolved variable, and we determine its fallback
        // based solely on how it was created, not what other type
        // variables it may have been unified with since then.
        //
        // The reason this matters is that other attempts at fallback
        // may (in principle) conflict with this fallback, and we wish
        // to generate a type error in that case. (However, this
        // actually isn't true right now, because we're only using the
        // builtin fallback rules. This would be true if we were using
        // user-supplied fallbacks. But it's still useful to write the
        // code to detect bugs.)
        //
        // (Note though that if we have a general type variable `?T`
        // that is then unified with an integer type variable `?I`
        // that ultimately never gets resolved to a special integral
        // type, `?T` is not considered unsolved, but `?I` is. The
        // same is true for float variables.)
        let fallback = match ty.kind() {
            _ if self.is_tainted_by_errors() => self.tcx.ty_error(),
            ty::Infer(ty::IntVar(_)) => self.tcx.types.i32,
            ty::Infer(ty::FloatVar(_)) => self.tcx.types.f64,
            _ => match diverging_fallback.get(&ty) {
                Some(&fallback_ty) => fallback_ty,
                None => return false,
            },
        };
        debug!("fallback_if_possible(ty={:?}): defaulting to `{:?}`", ty, fallback);

        let span = self
            .infcx
            .type_var_origin(ty)
            .map(|origin| origin.span)
            .unwrap_or(rustc_span::DUMMY_SP);
        self.demand_eqtype(span, ty, fallback);
        true
    }

    /// Second round of fallback: Unconstrained type variables created
    /// from the instantiation of an opaque type fall back to the
    /// opaque type itself. This is a somewhat incomplete attempt to
    /// manage "identity passthrough" for `impl Trait` types.
    ///
    /// For example, in this code:
    ///
    ///```
    /// type MyType = impl Copy;
    /// fn defining_use() -> MyType { true }
    /// fn other_use() -> MyType { defining_use() }
    /// ```
    ///
    /// `defining_use` will constrain the instantiated inference
    /// variable to `bool`, while `other_use` will constrain
    /// the instantiated inference variable to `MyType`.
    ///
    /// When we process opaque types during writeback, we
    /// will handle cases like `other_use`, and not count
    /// them as defining usages
    ///
    /// However, we also need to handle cases like this:
    ///
    /// ```rust
    /// pub type Foo = impl Copy;
    /// fn produce() -> Option<Foo> {
    ///     None
    ///  }
    ///  ```
    ///
    /// In the above snippet, the inference variable created by
    /// instantiating `Option<Foo>` will be completely unconstrained.
    /// We treat this as a non-defining use by making the inference
    /// variable fall back to the opaque type itself.
    fn fallback_opaque_type_vars(&self, ty: Ty<'tcx>) -> bool {
        let span = self
            .infcx
            .type_var_origin(ty)
            .map(|origin| origin.span)
            .unwrap_or(rustc_span::DUMMY_SP);
        if let Some(&opaque_ty) = self.opaque_types_vars.borrow().get(ty) {
            debug!(
                "fallback_opaque_type_vars(ty={:?}): falling back to opaque type {:?}",
                ty, opaque_ty
            );
            self.demand_eqtype(span, ty, opaque_ty);
            true
        } else {
            return false;
        }
    }

    /// The "diverging fallback" system is rather complicated. This is
    /// a result of our need to balance 'do the right thing' with
    /// backwards compatibility.
    ///
    /// "Diverging" type variables are variables created when we
    /// coerce a `!` type into an unbound type variable `?X`. If they
    /// never wind up being constrained, the "right and natural" thing
    /// is that `?X` should "fallback" to `!`. This means that e.g. an
    /// expression like `Some(return)` will ultimately wind up with a
    /// type like `Option<!>` (presuming it is not assigned or
    /// constrained to have some other type).
    ///
    /// However, the fallback used to be `()` (before the `!` type was
    /// added).  Moreover, there are cases where the `!` type 'leaks
    /// out' from dead code into type variables that affect live
    /// code. The most common case is something like this:
    ///
    /// ```rust
    /// match foo() {
    ///     22 => Default::default(), // call this type `?D`
    ///     _ => return, // return has type `!`
    /// } // call the type of this match `?M`
    /// ```
    ///
    /// Here, coercing the type `!` into `?M` will create a diverging
    /// type variable `?X` where `?X <: ?M`.  We also have that `?D <:
    /// ?M`. If `?M` winds up unconstrained, then `?X` will
    /// fallback. If it falls back to `!`, then all the type variables
    /// will wind up equal to `!` -- this includes the type `?D`
    /// (since `!` doesn't implement `Default`, we wind up a "trait
    /// not implemented" error in code like this). But since the
    /// original fallback was `()`, this code used to compile with `?D
    /// = ()`. This is somewhat surprising, since `Default::default()`
    /// on its own would give an error because the types are
    /// insufficiently constrained.
    ///
    /// Our solution to this dilemma is to modify diverging variables
    /// so that they can *either* fallback to `!` (the default) or to
    /// `()` (the backwards compatibility case). We decide which
    /// fallback to use based on whether there is a coercion pattern
    /// like this:
    ///
    /// ```
    /// ?Diverging -> ?V
    /// ?NonDiverging -> ?V
    /// ?V != ?NonDiverging
    /// ```
    ///
    /// Here `?Diverging` represents some diverging type variable and
    /// `?NonDiverging` represents some non-diverging type
    /// variable. `?V` can be any type variable (diverging or not), so
    /// long as it is not equal to `?NonDiverging`.
    ///
    /// Intuitively, what we are looking for is a case where a
    /// "non-diverging" type variable (like `?M` in our example above)
    /// is coerced *into* some variable `?V` that would otherwise
    /// fallback to `!`. In that case, we make `?V` fallback to `!`,
    /// along with anything that would flow into `?V`.
    ///
    /// The algorithm we use:
    /// * Identify all variables that are coerced *into* by a
    ///   diverging variable.  Do this by iterating over each
    ///   diverging, unsolved variable and finding all variables
    ///   reachable from there. Call that set `D`.
    /// * Walk over all unsolved, non-diverging variables, and find
    ///   any variable that has an edge into `D`.
    fn calculate_diverging_fallback(
        &self,
        unsolved_variables: &[Ty<'tcx>],
    ) -> FxHashMap<Ty<'tcx>, Ty<'tcx>> {
        debug!("calculate_diverging_fallback({:?})", unsolved_variables);

        // Construct a coercion graph where an edge `A -> B` indicates
        // a type variable is that is coerced
        let coercion_graph = self.create_coercion_graph();

        // Extract the unsolved type inference variable vids; note that some
        // unsolved variables are integer/float variables and are excluded.
        let unsolved_vids = unsolved_variables.iter().filter_map(|ty| ty.ty_vid());

        // Compute the diverging root vids D -- that is, the root vid of
        // those type variables that (a) are the target of a coercion from
        // a `!` type and (b) have not yet been solved.
        //
        // These variables are the ones that are targets for fallback to
        // either `!` or `()`.
        let diverging_roots: FxHashSet<ty::TyVid> = self
            .diverging_type_vars
            .borrow()
            .iter()
            .map(|&ty| self.infcx.shallow_resolve(ty))
            .filter_map(|ty| ty.ty_vid())
            .map(|vid| self.infcx.root_var(vid))
            .collect();
        debug!(
            "calculate_diverging_fallback: diverging_type_vars={:?}",
            self.diverging_type_vars.borrow()
        );
        debug!("calculate_diverging_fallback: diverging_roots={:?}", diverging_roots);

        // Find all type variables that are reachable from a diverging
        // type variable. These will typically default to `!`, unless
        // we find later that they are *also* reachable from some
        // other type variable outside this set.
        let mut roots_reachable_from_diverging = DepthFirstSearch::new(&coercion_graph);
        let mut diverging_vids = vec![];
        let mut non_diverging_vids = vec![];
        for unsolved_vid in unsolved_vids {
            let root_vid = self.infcx.root_var(unsolved_vid);
            debug!(
                "calculate_diverging_fallback: unsolved_vid={:?} root_vid={:?} diverges={:?}",
                unsolved_vid,
                root_vid,
                diverging_roots.contains(&root_vid),
            );
            if diverging_roots.contains(&root_vid) {
                diverging_vids.push(unsolved_vid);
                roots_reachable_from_diverging.push_start_node(root_vid);

                debug!(
                    "calculate_diverging_fallback: root_vid={:?} reaches {:?}",
                    root_vid,
                    coercion_graph.depth_first_search(root_vid).collect::<Vec<_>>()
                );

                // drain the iterator to visit all nodes reachable from this node
                roots_reachable_from_diverging.complete_search();
            } else {
                non_diverging_vids.push(unsolved_vid);
            }
        }

        debug!(
            "calculate_diverging_fallback: roots_reachable_from_diverging={:?}",
            roots_reachable_from_diverging,
        );

        // Find all type variables N0 that are not reachable from a
        // diverging variable, and then compute the set reachable from
        // N0, which we call N. These are the *non-diverging* type
        // variables. (Note that this set consists of "root variables".)
        let mut roots_reachable_from_non_diverging = DepthFirstSearch::new(&coercion_graph);
        for &non_diverging_vid in &non_diverging_vids {
            let root_vid = self.infcx.root_var(non_diverging_vid);
            if roots_reachable_from_diverging.visited(root_vid) {
                continue;
            }
            roots_reachable_from_non_diverging.push_start_node(root_vid);
            roots_reachable_from_non_diverging.complete_search();
        }
        debug!(
            "calculate_diverging_fallback: roots_reachable_from_non_diverging={:?}",
            roots_reachable_from_non_diverging,
        );

        // For each diverging variable, figure out whether it can
        // reach a member of N. If so, it falls back to `()`. Else
        // `!`.
        let mut diverging_fallback = FxHashMap::default();
        diverging_fallback.reserve(diverging_vids.len());
        'outer: for &diverging_vid in &diverging_vids {
            let diverging_ty = self.tcx.mk_ty_var(diverging_vid);
            let root_vid = self.infcx.root_var(diverging_vid);
            let can_reach_non_diverging = coercion_graph
                .depth_first_search(root_vid)
                .any(|n| roots_reachable_from_non_diverging.visited(n));

            for obligation in self.fulfillment_cx.borrow_mut().pending_obligations() {
                // We need to check if this obligation is a trait bound like
                // `root_vid: Foo`, and then we check:
                //
                // If `(): Foo` may hold, then fallback to (),
                // otherwise continue on.
                if let ty::PredicateKind::Trait(predicate, constness) =
                    obligation.predicate.kind().skip_binder()
                {
                    if predicate.trait_ref.def_id
                        == self.infcx.tcx.require_lang_item(rustc_hir::LangItem::Sized, None)
                    {
                        // Skip sized obligations, those are not usually
                        // 'intentional', satisfied by both ! and () though.
                        continue;
                    }

                    // If this trait bound is on the current root_vid...
                    if self.root_vid(predicate.self_ty()) == Some(root_vid) {
                        // fixme: copy of mk_trait_obligation_with_new_self_ty
                        let new_self_ty = self.infcx.tcx.types.unit;

                        let trait_ref = ty::TraitRef {
                            substs: self
                                .infcx
                                .tcx
                                .mk_substs_trait(new_self_ty, &predicate.trait_ref.substs[1..]),
                            ..predicate.trait_ref
                        };

                        // Then contstruct a new obligation with Self = () added
                        // to the ParamEnv, and see if it holds.
                        let o = rustc_infer::traits::Obligation::new(
                            traits::ObligationCause::dummy(),
                            obligation.param_env,
                            // FIXME: this drops the binder on the floor that
                            // previously existed?
                            trait_ref.with_constness(constness).to_predicate(self.infcx.tcx),
                        );
                        if self.infcx.predicate_may_hold(&o) {
                            // If we might hold for (), then fallback to ().
                            debug!("fallback to () as {:?} may hold: {:?}", o, diverging_vid);
                            diverging_fallback.insert(diverging_ty, self.tcx.types.unit);
                            continue 'outer;
                        }
                    }
                }
            }

            if can_reach_non_diverging {
                debug!("fallback to () - reached non-diverging: {:?}", diverging_vid);
                diverging_fallback.insert(diverging_ty, self.tcx.types.unit);
            } else {
                debug!("fallback to ! - all diverging: {:?}", diverging_vid);
                diverging_fallback.insert(diverging_ty, self.tcx.mk_diverging_default());
            }
        }

        diverging_fallback
    }

    /// Returns a graph whose nodes are (unresolved) inference variables and where
    /// an edge `?A -> ?B` indicates that the variable `?A` is coerced to `?B`.
    fn create_coercion_graph(&self) -> VecGraph<ty::TyVid> {
        let pending_obligations = self.fulfillment_cx.borrow_mut().pending_obligations();
        debug!("create_coercion_graph: pending_obligations={:?}", pending_obligations);
        let coercion_edges: Vec<(ty::TyVid, ty::TyVid)> = pending_obligations
            .into_iter()
            .filter_map(|obligation| {
                // The predicates we are looking for look like `Coerce(?A -> ?B)`.
                // They will have no bound variables.
                obligation.predicate.kind().no_bound_vars()
            })
            .filter_map(|atom| {
                if let ty::PredicateKind::Coerce(ty::CoercePredicate { a, b }) = atom {
                    let a_vid = self.root_vid(a)?;
                    let b_vid = self.root_vid(b)?;
                    Some((a_vid, b_vid))
                } else {
                    None
                }
            })
            .collect();
        debug!("create_coercion_graph: coercion_edges={:?}", coercion_edges);
        let num_ty_vars = self.infcx.num_ty_vars();
        VecGraph::new(num_ty_vars, coercion_edges)
    }

    /// If `ty` is an unresolved type variable, returns its root vid.
    fn root_vid(&self, ty: Ty<'tcx>) -> Option<ty::TyVid> {
        Some(self.infcx.root_var(self.infcx.shallow_resolve(ty).ty_vid()?))
    }
}
