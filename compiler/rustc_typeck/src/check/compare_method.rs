use crate::errors::LifetimesOrBoundsMismatchOnTrait;
use rustc_errors::{pluralize, struct_span_err, Applicability, DiagnosticId, ErrorReported};
use rustc_hir as hir;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::intravisit;
use rustc_hir::{GenericParamKind, ImplItemKind, TraitItemKind};
use rustc_infer::infer::{self, InferOk, TyCtxtInferExt};
use rustc_infer::traits::util;
use rustc_middle::ty;
use rustc_middle::ty::error::{ExpectedFound, TypeError};
use rustc_middle::ty::subst::{InternalSubsts, Subst};
use rustc_middle::ty::util::ExplicitSelf;
use rustc_middle::ty::{GenericParamDefKind, ToPredicate, TyCtxt};
use rustc_span::Span;
use rustc_trait_selection::traits::error_reporting::InferCtxtExt;
use rustc_trait_selection::traits::{self, ObligationCause, ObligationCauseCode, Reveal};
use std::iter;

use super::{potentially_plural_count, FnCtxt, Inherited};

/// Checks that a method from an impl conforms to the signature of
/// the same method as declared in the trait.
///
/// # Parameters
///
/// - `impl_m`: type of the method we are checking
/// - `impl_m_span`: span to use for reporting errors
/// - `trait_m`: the method in the trait
/// - `impl_trait_ref`: the TraitRef corresponding to the trait implementation

crate fn compare_impl_method<'tcx>(
    tcx: TyCtxt<'tcx>,
    impl_m: &ty::AssocItem,
    impl_m_span: Span,
    trait_m: &ty::AssocItem,
    impl_trait_ref: ty::TraitRef<'tcx>,
    trait_item_span: Option<Span>,
) {
    debug!("compare_impl_method(impl_trait_ref={:?})", impl_trait_ref);

    let impl_m_span = tcx.sess.source_map().guess_head_span(impl_m_span);

    if let Err(ErrorReported) = compare_self_type(tcx, impl_m, impl_m_span, trait_m, impl_trait_ref)
    {
        return;
    }

    if let Err(ErrorReported) =
        compare_number_of_generics(tcx, impl_m, impl_m_span, trait_m, trait_item_span)
    {
        return;
    }

    if let Err(ErrorReported) =
        compare_number_of_method_arguments(tcx, impl_m, impl_m_span, trait_m, trait_item_span)
    {
        return;
    }

    if let Err(ErrorReported) = compare_synthetic_generics(tcx, impl_m, trait_m) {
        return;
    }

    if let Err(ErrorReported) =
        compare_predicate_entailment(tcx, impl_m, impl_m_span, trait_m, impl_trait_ref)
    {
        return;
    }
}

fn compare_predicate_entailment<'tcx>(
    tcx: TyCtxt<'tcx>,
    impl_m: &ty::AssocItem,
    impl_m_span: Span,
    trait_m: &ty::AssocItem,
    impl_trait_ref: ty::TraitRef<'tcx>,
) -> Result<(), ErrorReported> {
    let trait_to_impl_substs = impl_trait_ref.substs;

    // This node-id should be used for the `body_id` field on each
    // `ObligationCause` (and the `FnCtxt`). This is what
    // `regionck_item` expects.
    let impl_m_hir_id = tcx.hir().local_def_id_to_hir_id(impl_m.def_id.expect_local());

    // We sometimes modify the span further down.
    let mut cause = ObligationCause::new(
        impl_m_span,
        impl_m_hir_id,
        ObligationCauseCode::CompareImplMethodObligation {
            item_name: impl_m.ident.name,
            impl_item_def_id: impl_m.def_id,
            trait_item_def_id: trait_m.def_id,
        },
    );

    // This code is best explained by example. Consider a trait:
    //
    //     trait Trait<'t, T> {
    //         fn method<'a, M>(t: &'t T, m: &'a M) -> Self;
    //     }
    //
    // And an impl:
    //
    //     impl<'i, 'j, U> Trait<'j, &'i U> for Foo {
    //          fn method<'b, N>(t: &'j &'i U, m: &'b N) -> Foo;
    //     }
    //
    // We wish to decide if those two method types are compatible.
    //
    // We start out with trait_to_impl_substs, that maps the trait
    // type parameters to impl type parameters. This is taken from the
    // impl trait reference:
    //
    //     trait_to_impl_substs = {'t => 'j, T => &'i U, Self => Foo}
    //
    // We create a mapping `dummy_substs` that maps from the impl type
    // parameters to fresh types and regions. For type parameters,
    // this is the identity transform, but we could as well use any
    // placeholder types. For regions, we convert from bound to free
    // regions (Note: but only early-bound regions, i.e., those
    // declared on the impl or used in type parameter bounds).
    //
    //     impl_to_placeholder_substs = {'i => 'i0, U => U0, N => N0 }
    //
    // Now we can apply placeholder_substs to the type of the impl method
    // to yield a new function type in terms of our fresh, placeholder
    // types:
    //
    //     <'b> fn(t: &'i0 U0, m: &'b) -> Foo
    //
    // We now want to extract and substitute the type of the *trait*
    // method and compare it. To do so, we must create a compound
    // substitution by combining trait_to_impl_substs and
    // impl_to_placeholder_substs, and also adding a mapping for the method
    // type parameters. We extend the mapping to also include
    // the method parameters.
    //
    //     trait_to_placeholder_substs = { T => &'i0 U0, Self => Foo, M => N0 }
    //
    // Applying this to the trait method type yields:
    //
    //     <'a> fn(t: &'i0 U0, m: &'a) -> Foo
    //
    // This type is also the same but the name of the bound region ('a
    // vs 'b).  However, the normal subtyping rules on fn types handle
    // this kind of equivalency just fine.
    //
    // We now use these substitutions to ensure that all declared bounds are
    // satisfied by the implementation's method.
    //
    // We do this by creating a parameter environment which contains a
    // substitution corresponding to impl_to_placeholder_substs. We then build
    // trait_to_placeholder_substs and use it to convert the predicates contained
    // in the trait_m.generics to the placeholder form.
    //
    // Finally we register each of these predicates as an obligation in
    // a fresh FulfillmentCtxt, and invoke select_all_or_error.

    // Create mapping from impl to placeholder.
    let impl_to_placeholder_substs = InternalSubsts::identity_for_item(tcx, impl_m.def_id);

    // Create mapping from trait to placeholder.
    let trait_to_placeholder_substs =
        impl_to_placeholder_substs.rebase_onto(tcx, impl_m.container.id(), trait_to_impl_substs);
    debug!("compare_impl_method: trait_to_placeholder_substs={:?}", trait_to_placeholder_substs);

    let impl_m_generics = tcx.generics_of(impl_m.def_id);
    let trait_m_generics = tcx.generics_of(trait_m.def_id);
    let impl_m_predicates = tcx.predicates_of(impl_m.def_id);
    let trait_m_predicates = tcx.predicates_of(trait_m.def_id);

    // Check region bounds.
    check_region_bounds_on_impl_item(
        tcx,
        impl_m_span,
        impl_m,
        trait_m,
        &trait_m_generics,
        &impl_m_generics,
    )?;

    // Create obligations for each predicate declared by the impl
    // definition in the context of the trait's parameter
    // environment. We can't just use `impl_env.caller_bounds`,
    // however, because we want to replace all late-bound regions with
    // region variables.
    let impl_predicates = tcx.predicates_of(impl_m_predicates.parent.unwrap());
    let mut hybrid_preds = impl_predicates.instantiate_identity(tcx);

    debug!("compare_impl_method: impl_bounds={:?}", hybrid_preds);

    // This is the only tricky bit of the new way we check implementation methods
    // We need to build a set of predicates where only the method-level bounds
    // are from the trait and we assume all other bounds from the implementation
    // to be previously satisfied.
    //
    // We then register the obligations from the impl_m and check to see
    // if all constraints hold.
    hybrid_preds
        .predicates
        .extend(trait_m_predicates.instantiate_own(tcx, trait_to_placeholder_substs).predicates);

    // Construct trait parameter environment and then shift it into the placeholder viewpoint.
    // The key step here is to update the caller_bounds's predicates to be
    // the new hybrid bounds we computed.
    let normalize_cause = traits::ObligationCause::misc(impl_m_span, impl_m_hir_id);
    let param_env =
        ty::ParamEnv::new(tcx.intern_predicates(&hybrid_preds.predicates), Reveal::UserFacing);
    let param_env = traits::normalize_param_env_or_error(
        tcx,
        impl_m.def_id,
        param_env,
        normalize_cause.clone(),
    );

    tcx.infer_ctxt().enter(|infcx| {
        let inh = Inherited::new(infcx, impl_m.def_id.expect_local());
        let infcx = &inh.infcx;

        debug!("compare_impl_method: caller_bounds={:?}", param_env.caller_bounds());

        let mut selcx = traits::SelectionContext::new(&infcx);

        let impl_m_own_bounds = impl_m_predicates.instantiate_own(tcx, impl_to_placeholder_substs);
        for predicate in impl_m_own_bounds.predicates {
            let traits::Normalized { value: predicate, obligations } =
                traits::normalize(&mut selcx, param_env, normalize_cause.clone(), predicate);

            inh.register_predicates(obligations);
            inh.register_predicate(traits::Obligation::new(cause.clone(), param_env, predicate));
        }

        // We now need to check that the signature of the impl method is
        // compatible with that of the trait method. We do this by
        // checking that `impl_fty <: trait_fty`.
        //
        // FIXME. Unfortunately, this doesn't quite work right now because
        // associated type normalization is not integrated into subtype
        // checks. For the comparison to be valid, we need to
        // normalize the associated types in the impl/trait methods
        // first. However, because function types bind regions, just
        // calling `normalize_associated_types_in` would have no effect on
        // any associated types appearing in the fn arguments or return
        // type.

        // Compute placeholder form of impl and trait method tys.
        let tcx = infcx.tcx;

        let (impl_sig, _) = infcx.replace_bound_vars_with_fresh_vars(
            impl_m_span,
            infer::HigherRankedType,
            tcx.fn_sig(impl_m.def_id),
        );
        let impl_sig =
            inh.normalize_associated_types_in(impl_m_span, impl_m_hir_id, param_env, impl_sig);
        let impl_fty = tcx.mk_fn_ptr(ty::Binder::dummy(impl_sig));
        debug!("compare_impl_method: impl_fty={:?}", impl_fty);

        let trait_sig = tcx.liberate_late_bound_regions(impl_m.def_id, tcx.fn_sig(trait_m.def_id));
        let trait_sig = trait_sig.subst(tcx, trait_to_placeholder_substs);
        let trait_sig =
            inh.normalize_associated_types_in(impl_m_span, impl_m_hir_id, param_env, trait_sig);
        let trait_fty = tcx.mk_fn_ptr(ty::Binder::dummy(trait_sig));

        debug!("compare_impl_method: trait_fty={:?}", trait_fty);

        let sub_result = infcx.at(&cause, param_env).sup(trait_fty, impl_fty).map(
            |InferOk { obligations, .. }| {
                inh.register_predicates(obligations);
            },
        );

        if let Err(terr) = sub_result {
            debug!("sub_types failed: impl ty {:?}, trait ty {:?}", impl_fty, trait_fty);

            let (impl_err_span, trait_err_span) =
                extract_spans_for_error_reporting(&infcx, &terr, &cause, impl_m, trait_m);

            cause.make_mut().span = impl_err_span;

            let mut diag = struct_span_err!(
                tcx.sess,
                cause.span(tcx),
                E0053,
                "method `{}` has an incompatible type for trait",
                trait_m.ident
            );
            match &terr {
                TypeError::ArgumentMutability(0) | TypeError::ArgumentSorts(_, 0)
                    if trait_m.fn_has_self_parameter =>
                {
                    let ty = trait_sig.inputs()[0];
                    let sugg = match ExplicitSelf::determine(ty, |_| ty == impl_trait_ref.self_ty())
                    {
                        ExplicitSelf::ByValue => "self".to_owned(),
                        ExplicitSelf::ByReference(_, hir::Mutability::Not) => "&self".to_owned(),
                        ExplicitSelf::ByReference(_, hir::Mutability::Mut) => {
                            "&mut self".to_owned()
                        }
                        _ => format!("self: {}", ty),
                    };

                    // When the `impl` receiver is an arbitrary self type, like `self: Box<Self>`, the
                    // span points only at the type `Box<Self`>, but we want to cover the whole
                    // argument pattern and type.
                    let impl_m_hir_id =
                        tcx.hir().local_def_id_to_hir_id(impl_m.def_id.expect_local());
                    let span = match tcx.hir().expect_impl_item(impl_m_hir_id).kind {
                        ImplItemKind::Fn(ref sig, body) => tcx
                            .hir()
                            .body_param_names(body)
                            .zip(sig.decl.inputs.iter())
                            .map(|(param, ty)| param.span.to(ty.span))
                            .next()
                            .unwrap_or(impl_err_span),
                        _ => bug!("{:?} is not a method", impl_m),
                    };

                    diag.span_suggestion(
                        span,
                        "change the self-receiver type to match the trait",
                        sugg,
                        Applicability::MachineApplicable,
                    );
                }
                TypeError::ArgumentMutability(i) | TypeError::ArgumentSorts(_, i) => {
                    if trait_sig.inputs().len() == *i {
                        // Suggestion to change output type. We do not suggest in `async` functions
                        // to avoid complex logic or incorrect output.
                        let impl_m_hir_id =
                            tcx.hir().local_def_id_to_hir_id(impl_m.def_id.expect_local());
                        match tcx.hir().expect_impl_item(impl_m_hir_id).kind {
                            ImplItemKind::Fn(ref sig, _)
                                if sig.header.asyncness == hir::IsAsync::NotAsync =>
                            {
                                let msg = "change the output type to match the trait";
                                let ap = Applicability::MachineApplicable;
                                match sig.decl.output {
                                    hir::FnRetTy::DefaultReturn(sp) => {
                                        let sugg = format!("-> {} ", trait_sig.output());
                                        diag.span_suggestion_verbose(sp, msg, sugg, ap);
                                    }
                                    hir::FnRetTy::Return(hir_ty) => {
                                        let sugg = trait_sig.output().to_string();
                                        diag.span_suggestion(hir_ty.span, msg, sugg, ap);
                                    }
                                };
                            }
                            _ => {}
                        };
                    } else if let Some(trait_ty) = trait_sig.inputs().get(*i) {
                        diag.span_suggestion(
                            impl_err_span,
                            "change the parameter type to match the trait",
                            trait_ty.to_string(),
                            Applicability::MachineApplicable,
                        );
                    }
                }
                _ => {}
            }

            infcx.note_type_err(
                &mut diag,
                &cause,
                trait_err_span.map(|sp| (sp, "type in trait".to_owned())),
                Some(infer::ValuePairs::Types(ExpectedFound {
                    expected: trait_fty,
                    found: impl_fty,
                })),
                &terr,
                false,
            );
            diag.emit();
            return Err(ErrorReported);
        }

        // Check that all obligations are satisfied by the implementation's
        // version.
        if let Err(ref errors) = inh.fulfillment_cx.borrow_mut().select_all_or_error(&infcx) {
            infcx.report_fulfillment_errors(errors, None, false);
            return Err(ErrorReported);
        }

        // Finally, resolve all regions. This catches wily misuses of
        // lifetime parameters.
        let fcx = FnCtxt::new(&inh, param_env, impl_m_hir_id);
        fcx.regionck_item(impl_m_hir_id, impl_m_span, trait_sig.inputs_and_output);

        Ok(())
    })
}

fn check_region_bounds_on_impl_item<'tcx>(
    tcx: TyCtxt<'tcx>,
    span: Span,
    impl_m: &ty::AssocItem,
    trait_m: &ty::AssocItem,
    trait_generics: &ty::Generics,
    impl_generics: &ty::Generics,
) -> Result<(), ErrorReported> {
    let trait_params = trait_generics.own_counts().lifetimes;
    let impl_params = impl_generics.own_counts().lifetimes;

    debug!(
        "check_region_bounds_on_impl_item: \
            trait_generics={:?} \
            impl_generics={:?}",
        trait_generics, impl_generics
    );

    // Must have same number of early-bound lifetime parameters.
    // Unfortunately, if the user screws up the bounds, then this
    // will change classification between early and late.  E.g.,
    // if in trait we have `<'a,'b:'a>`, and in impl we just have
    // `<'a,'b>`, then we have 2 early-bound lifetime parameters
    // in trait but 0 in the impl. But if we report "expected 2
    // but found 0" it's confusing, because it looks like there
    // are zero. Since I don't quite know how to phrase things at
    // the moment, give a kind of vague error message.
    if trait_params != impl_params {
        let item_kind = assoc_item_kind_str(impl_m);
        let def_span = tcx.sess.source_map().guess_head_span(span);
        let span = tcx.hir().get_generics(impl_m.def_id).map_or(def_span, |g| g.span);
        let generics_span = tcx.hir().span_if_local(trait_m.def_id).map(|sp| {
            let def_sp = tcx.sess.source_map().guess_head_span(sp);
            tcx.hir().get_generics(trait_m.def_id).map_or(def_sp, |g| g.span)
        });

        tcx.sess.emit_err(LifetimesOrBoundsMismatchOnTrait {
            span,
            item_kind,
            ident: impl_m.ident,
            generics_span,
        });
        return Err(ErrorReported);
    }

    Ok(())
}

fn extract_spans_for_error_reporting<'a, 'tcx>(
    infcx: &infer::InferCtxt<'a, 'tcx>,
    terr: &TypeError<'_>,
    cause: &ObligationCause<'tcx>,
    impl_m: &ty::AssocItem,
    trait_m: &ty::AssocItem,
) -> (Span, Option<Span>) {
    let tcx = infcx.tcx;
    let impl_m_hir_id = tcx.hir().local_def_id_to_hir_id(impl_m.def_id.expect_local());
    let mut impl_args = match tcx.hir().expect_impl_item(impl_m_hir_id).kind {
        ImplItemKind::Fn(ref sig, _) => {
            sig.decl.inputs.iter().map(|t| t.span).chain(iter::once(sig.decl.output.span()))
        }
        _ => bug!("{:?} is not a method", impl_m),
    };
    let trait_args = trait_m.def_id.as_local().map(|def_id| {
        let trait_m_hir_id = tcx.hir().local_def_id_to_hir_id(def_id);
        match tcx.hir().expect_trait_item(trait_m_hir_id).kind {
            TraitItemKind::Fn(ref sig, _) => {
                sig.decl.inputs.iter().map(|t| t.span).chain(iter::once(sig.decl.output.span()))
            }
            _ => bug!("{:?} is not a TraitItemKind::Fn", trait_m),
        }
    });

    match *terr {
        TypeError::ArgumentMutability(i) => {
            (impl_args.nth(i).unwrap(), trait_args.and_then(|mut args| args.nth(i)))
        }
        TypeError::ArgumentSorts(ExpectedFound { .. }, i) => {
            (impl_args.nth(i).unwrap(), trait_args.and_then(|mut args| args.nth(i)))
        }
        _ => (cause.span(tcx), tcx.hir().span_if_local(trait_m.def_id)),
    }
}

fn compare_self_type<'tcx>(
    tcx: TyCtxt<'tcx>,
    impl_m: &ty::AssocItem,
    impl_m_span: Span,
    trait_m: &ty::AssocItem,
    impl_trait_ref: ty::TraitRef<'tcx>,
) -> Result<(), ErrorReported> {
    // Try to give more informative error messages about self typing
    // mismatches.  Note that any mismatch will also be detected
    // below, where we construct a canonical function type that
    // includes the self parameter as a normal parameter.  It's just
    // that the error messages you get out of this code are a bit more
    // inscrutable, particularly for cases where one method has no
    // self.

    let self_string = |method: &ty::AssocItem| {
        let untransformed_self_ty = match method.container {
            ty::ImplContainer(_) => impl_trait_ref.self_ty(),
            ty::TraitContainer(_) => tcx.types.self_param,
        };
        let self_arg_ty = tcx.fn_sig(method.def_id).input(0);
        let param_env = ty::ParamEnv::reveal_all();

        tcx.infer_ctxt().enter(|infcx| {
            let self_arg_ty = tcx.liberate_late_bound_regions(method.def_id, self_arg_ty);
            let can_eq_self = |ty| infcx.can_eq(param_env, untransformed_self_ty, ty).is_ok();
            match ExplicitSelf::determine(self_arg_ty, can_eq_self) {
                ExplicitSelf::ByValue => "self".to_owned(),
                ExplicitSelf::ByReference(_, hir::Mutability::Not) => "&self".to_owned(),
                ExplicitSelf::ByReference(_, hir::Mutability::Mut) => "&mut self".to_owned(),
                _ => format!("self: {}", self_arg_ty),
            }
        })
    };

    match (trait_m.fn_has_self_parameter, impl_m.fn_has_self_parameter) {
        (false, false) | (true, true) => {}

        (false, true) => {
            let self_descr = self_string(impl_m);
            let mut err = struct_span_err!(
                tcx.sess,
                impl_m_span,
                E0185,
                "method `{}` has a `{}` declaration in the impl, but not in the trait",
                trait_m.ident,
                self_descr
            );
            err.span_label(impl_m_span, format!("`{}` used in impl", self_descr));
            if let Some(span) = tcx.hir().span_if_local(trait_m.def_id) {
                err.span_label(span, format!("trait method declared without `{}`", self_descr));
            } else {
                err.note_trait_signature(trait_m.ident.to_string(), trait_m.signature(tcx));
            }
            err.emit();
            return Err(ErrorReported);
        }

        (true, false) => {
            let self_descr = self_string(trait_m);
            let mut err = struct_span_err!(
                tcx.sess,
                impl_m_span,
                E0186,
                "method `{}` has a `{}` declaration in the trait, but not in the impl",
                trait_m.ident,
                self_descr
            );
            err.span_label(impl_m_span, format!("expected `{}` in impl", self_descr));
            if let Some(span) = tcx.hir().span_if_local(trait_m.def_id) {
                err.span_label(span, format!("`{}` used in trait", self_descr));
            } else {
                err.note_trait_signature(trait_m.ident.to_string(), trait_m.signature(tcx));
            }
            err.emit();
            return Err(ErrorReported);
        }
    }

    Ok(())
}

fn compare_number_of_generics<'tcx>(
    tcx: TyCtxt<'tcx>,
    impl_: &ty::AssocItem,
    _impl_span: Span,
    trait_: &ty::AssocItem,
    trait_span: Option<Span>,
) -> Result<(), ErrorReported> {
    let trait_own_counts = tcx.generics_of(trait_.def_id).own_counts();
    let impl_own_counts = tcx.generics_of(impl_.def_id).own_counts();

    let matchings = [
        ("type", trait_own_counts.types, impl_own_counts.types),
        ("const", trait_own_counts.consts, impl_own_counts.consts),
    ];

    let item_kind = assoc_item_kind_str(impl_);

    let mut err_occurred = false;
    for (kind, trait_count, impl_count) in matchings {
        if impl_count != trait_count {
            err_occurred = true;

            let (trait_spans, impl_trait_spans) = if let Some(def_id) = trait_.def_id.as_local() {
                let trait_hir_id = tcx.hir().local_def_id_to_hir_id(def_id);
                let trait_item = tcx.hir().expect_trait_item(trait_hir_id);
                if trait_item.generics.params.is_empty() {
                    (Some(vec![trait_item.generics.span]), vec![])
                } else {
                    let arg_spans: Vec<Span> =
                        trait_item.generics.params.iter().map(|p| p.span).collect();
                    let impl_trait_spans: Vec<Span> = trait_item
                        .generics
                        .params
                        .iter()
                        .filter_map(|p| match p.kind {
                            GenericParamKind::Type {
                                synthetic: Some(hir::SyntheticTyParamKind::ImplTrait),
                                ..
                            } => Some(p.span),
                            _ => None,
                        })
                        .collect();
                    (Some(arg_spans), impl_trait_spans)
                }
            } else {
                (trait_span.map(|s| vec![s]), vec![])
            };

            let impl_hir_id = tcx.hir().local_def_id_to_hir_id(impl_.def_id.expect_local());
            let impl_item = tcx.hir().expect_impl_item(impl_hir_id);
            let impl_item_impl_trait_spans: Vec<Span> = impl_item
                .generics
                .params
                .iter()
                .filter_map(|p| match p.kind {
                    GenericParamKind::Type {
                        synthetic: Some(hir::SyntheticTyParamKind::ImplTrait),
                        ..
                    } => Some(p.span),
                    _ => None,
                })
                .collect();
            let spans = impl_item.generics.spans();
            let span = spans.primary_span();

            let mut err = tcx.sess.struct_span_err_with_code(
                spans,
                &format!(
                    "{} `{}` has {} {kind} parameter{} but its trait \
                     declaration has {} {kind} parameter{}",
                    item_kind,
                    trait_.ident,
                    impl_count,
                    pluralize!(impl_count),
                    trait_count,
                    pluralize!(trait_count),
                    kind = kind,
                ),
                DiagnosticId::Error("E0049".into()),
            );

            let mut suffix = None;

            if let Some(spans) = trait_spans {
                let mut spans = spans.iter();
                if let Some(span) = spans.next() {
                    err.span_label(
                        *span,
                        format!(
                            "expected {} {} parameter{}",
                            trait_count,
                            kind,
                            pluralize!(trait_count),
                        ),
                    );
                }
                for span in spans {
                    err.span_label(*span, "");
                }
            } else {
                suffix = Some(format!(", expected {}", trait_count));
            }

            if let Some(span) = span {
                err.span_label(
                    span,
                    format!(
                        "found {} {} parameter{}{}",
                        impl_count,
                        kind,
                        pluralize!(impl_count),
                        suffix.unwrap_or_else(String::new),
                    ),
                );
            }

            for span in impl_trait_spans.iter().chain(impl_item_impl_trait_spans.iter()) {
                err.span_label(*span, "`impl Trait` introduces an implicit type parameter");
            }

            err.emit();
        }
    }

    if err_occurred { Err(ErrorReported) } else { Ok(()) }
}

fn compare_number_of_method_arguments<'tcx>(
    tcx: TyCtxt<'tcx>,
    impl_m: &ty::AssocItem,
    impl_m_span: Span,
    trait_m: &ty::AssocItem,
    trait_item_span: Option<Span>,
) -> Result<(), ErrorReported> {
    let impl_m_fty = tcx.fn_sig(impl_m.def_id);
    let trait_m_fty = tcx.fn_sig(trait_m.def_id);
    let trait_number_args = trait_m_fty.inputs().skip_binder().len();
    let impl_number_args = impl_m_fty.inputs().skip_binder().len();
    if trait_number_args != impl_number_args {
        let trait_span = if let Some(def_id) = trait_m.def_id.as_local() {
            let trait_id = tcx.hir().local_def_id_to_hir_id(def_id);
            match tcx.hir().expect_trait_item(trait_id).kind {
                TraitItemKind::Fn(ref trait_m_sig, _) => {
                    let pos = if trait_number_args > 0 { trait_number_args - 1 } else { 0 };
                    if let Some(arg) = trait_m_sig.decl.inputs.get(pos) {
                        Some(if pos == 0 {
                            arg.span
                        } else {
                            Span::new(
                                trait_m_sig.decl.inputs[0].span.lo(),
                                arg.span.hi(),
                                arg.span.ctxt(),
                            )
                        })
                    } else {
                        trait_item_span
                    }
                }
                _ => bug!("{:?} is not a method", impl_m),
            }
        } else {
            trait_item_span
        };
        let impl_m_hir_id = tcx.hir().local_def_id_to_hir_id(impl_m.def_id.expect_local());
        let impl_span = match tcx.hir().expect_impl_item(impl_m_hir_id).kind {
            ImplItemKind::Fn(ref impl_m_sig, _) => {
                let pos = if impl_number_args > 0 { impl_number_args - 1 } else { 0 };
                if let Some(arg) = impl_m_sig.decl.inputs.get(pos) {
                    if pos == 0 {
                        arg.span
                    } else {
                        Span::new(
                            impl_m_sig.decl.inputs[0].span.lo(),
                            arg.span.hi(),
                            arg.span.ctxt(),
                        )
                    }
                } else {
                    impl_m_span
                }
            }
            _ => bug!("{:?} is not a method", impl_m),
        };
        let mut err = struct_span_err!(
            tcx.sess,
            impl_span,
            E0050,
            "method `{}` has {} but the declaration in trait `{}` has {}",
            trait_m.ident,
            potentially_plural_count(impl_number_args, "parameter"),
            tcx.def_path_str(trait_m.def_id),
            trait_number_args
        );
        if let Some(trait_span) = trait_span {
            err.span_label(
                trait_span,
                format!(
                    "trait requires {}",
                    potentially_plural_count(trait_number_args, "parameter")
                ),
            );
        } else {
            err.note_trait_signature(trait_m.ident.to_string(), trait_m.signature(tcx));
        }
        err.span_label(
            impl_span,
            format!(
                "expected {}, found {}",
                potentially_plural_count(trait_number_args, "parameter"),
                impl_number_args
            ),
        );
        err.emit();
        return Err(ErrorReported);
    }

    Ok(())
}

fn compare_synthetic_generics<'tcx>(
    tcx: TyCtxt<'tcx>,
    impl_m: &ty::AssocItem,
    trait_m: &ty::AssocItem,
) -> Result<(), ErrorReported> {
    // FIXME(chrisvittal) Clean up this function, list of FIXME items:
    //     1. Better messages for the span labels
    //     2. Explanation as to what is going on
    // If we get here, we already have the same number of generics, so the zip will
    // be okay.
    let mut error_found = false;
    let impl_m_generics = tcx.generics_of(impl_m.def_id);
    let trait_m_generics = tcx.generics_of(trait_m.def_id);
    let impl_m_type_params = impl_m_generics.params.iter().filter_map(|param| match param.kind {
        GenericParamDefKind::Type { synthetic, .. } => Some((param.def_id, synthetic)),
        GenericParamDefKind::Lifetime | GenericParamDefKind::Const { .. } => None,
    });
    let trait_m_type_params = trait_m_generics.params.iter().filter_map(|param| match param.kind {
        GenericParamDefKind::Type { synthetic, .. } => Some((param.def_id, synthetic)),
        GenericParamDefKind::Lifetime | GenericParamDefKind::Const { .. } => None,
    });
    for ((impl_def_id, impl_synthetic), (trait_def_id, trait_synthetic)) in
        iter::zip(impl_m_type_params, trait_m_type_params)
    {
        if impl_synthetic != trait_synthetic {
            let impl_hir_id = tcx.hir().local_def_id_to_hir_id(impl_def_id.expect_local());
            let impl_span = tcx.hir().span(impl_hir_id);
            let trait_span = tcx.def_span(trait_def_id);
            let mut err = struct_span_err!(
                tcx.sess,
                impl_span,
                E0643,
                "method `{}` has incompatible signature for trait",
                trait_m.ident
            );
            err.span_label(trait_span, "declaration in trait here");
            match (impl_synthetic, trait_synthetic) {
                // The case where the impl method uses `impl Trait` but the trait method uses
                // explicit generics
                (Some(hir::SyntheticTyParamKind::ImplTrait), None) => {
                    err.span_label(impl_span, "expected generic parameter, found `impl Trait`");
                    (|| {
                        // try taking the name from the trait impl
                        // FIXME: this is obviously suboptimal since the name can already be used
                        // as another generic argument
                        let new_name = tcx.sess.source_map().span_to_snippet(trait_span).ok()?;
                        let trait_m = trait_m.def_id.as_local()?;
                        let trait_m = tcx.hir().trait_item(hir::TraitItemId { def_id: trait_m });

                        let impl_m = impl_m.def_id.as_local()?;
                        let impl_m = tcx.hir().impl_item(hir::ImplItemId { def_id: impl_m });

                        // in case there are no generics, take the spot between the function name
                        // and the opening paren of the argument list
                        let new_generics_span =
                            tcx.sess.source_map().generate_fn_name_span(impl_span)?.shrink_to_hi();
                        // in case there are generics, just replace them
                        let generics_span =
                            impl_m.generics.span.substitute_dummy(new_generics_span);
                        // replace with the generics from the trait
                        let new_generics =
                            tcx.sess.source_map().span_to_snippet(trait_m.generics.span).ok()?;

                        err.multipart_suggestion(
                            "try changing the `impl Trait` argument to a generic parameter",
                            vec![
                                // replace `impl Trait` with `T`
                                (impl_span, new_name),
                                // replace impl method generics with trait method generics
                                // This isn't quite right, as users might have changed the names
                                // of the generics, but it works for the common case
                                (generics_span, new_generics),
                            ],
                            Applicability::MaybeIncorrect,
                        );
                        Some(())
                    })();
                }
                // The case where the trait method uses `impl Trait`, but the impl method uses
                // explicit generics.
                (None, Some(hir::SyntheticTyParamKind::ImplTrait)) => {
                    err.span_label(impl_span, "expected `impl Trait`, found generic parameter");
                    (|| {
                        let impl_m = impl_m.def_id.as_local()?;
                        let impl_m = tcx.hir().impl_item(hir::ImplItemId { def_id: impl_m });
                        let input_tys = match impl_m.kind {
                            hir::ImplItemKind::Fn(ref sig, _) => sig.decl.inputs,
                            _ => unreachable!(),
                        };
                        struct Visitor(Option<Span>, hir::def_id::DefId);
                        impl<'v> intravisit::Visitor<'v> for Visitor {
                            fn visit_ty(&mut self, ty: &'v hir::Ty<'v>) {
                                intravisit::walk_ty(self, ty);
                                if let hir::TyKind::Path(hir::QPath::Resolved(None, ref path)) =
                                    ty.kind
                                {
                                    if let Res::Def(DefKind::TyParam, def_id) = path.res {
                                        if def_id == self.1 {
                                            self.0 = Some(ty.span);
                                        }
                                    }
                                }
                            }
                            type Map = intravisit::ErasedMap<'v>;
                            fn nested_visit_map(
                                &mut self,
                            ) -> intravisit::NestedVisitorMap<Self::Map>
                            {
                                intravisit::NestedVisitorMap::None
                            }
                        }
                        let mut visitor = Visitor(None, impl_def_id);
                        for ty in input_tys {
                            intravisit::Visitor::visit_ty(&mut visitor, ty);
                        }
                        let span = visitor.0?;

                        let bounds =
                            impl_m.generics.params.iter().find_map(|param| match param.kind {
                                GenericParamKind::Lifetime { .. } => None,
                                GenericParamKind::Type { .. } | GenericParamKind::Const { .. } => {
                                    if param.hir_id == impl_hir_id {
                                        Some(&param.bounds)
                                    } else {
                                        None
                                    }
                                }
                            })?;
                        let bounds = bounds.first()?.span().to(bounds.last()?.span());
                        let bounds = tcx.sess.source_map().span_to_snippet(bounds).ok()?;

                        err.multipart_suggestion(
                            "try removing the generic parameter and using `impl Trait` instead",
                            vec![
                                // delete generic parameters
                                (impl_m.generics.span, String::new()),
                                // replace param usage with `impl Trait`
                                (span, format!("impl {}", bounds)),
                            ],
                            Applicability::MaybeIncorrect,
                        );
                        Some(())
                    })();
                }
                _ => unreachable!(),
            }
            err.emit();
            error_found = true;
        }
    }
    if error_found { Err(ErrorReported) } else { Ok(()) }
}

crate fn compare_const_impl<'tcx>(
    tcx: TyCtxt<'tcx>,
    impl_c: &ty::AssocItem,
    impl_c_span: Span,
    trait_c: &ty::AssocItem,
    impl_trait_ref: ty::TraitRef<'tcx>,
) {
    debug!("compare_const_impl(impl_trait_ref={:?})", impl_trait_ref);

    tcx.infer_ctxt().enter(|infcx| {
        let param_env = tcx.param_env(impl_c.def_id);
        let inh = Inherited::new(infcx, impl_c.def_id.expect_local());
        let infcx = &inh.infcx;

        // The below is for the most part highly similar to the procedure
        // for methods above. It is simpler in many respects, especially
        // because we shouldn't really have to deal with lifetimes or
        // predicates. In fact some of this should probably be put into
        // shared functions because of DRY violations...
        let trait_to_impl_substs = impl_trait_ref.substs;

        // Create a parameter environment that represents the implementation's
        // method.
        let impl_c_hir_id = tcx.hir().local_def_id_to_hir_id(impl_c.def_id.expect_local());

        // Compute placeholder form of impl and trait const tys.
        let impl_ty = tcx.type_of(impl_c.def_id);
        let trait_ty = tcx.type_of(trait_c.def_id).subst(tcx, trait_to_impl_substs);
        let mut cause = ObligationCause::new(
            impl_c_span,
            impl_c_hir_id,
            ObligationCauseCode::CompareImplConstObligation,
        );

        // There is no "body" here, so just pass dummy id.
        let impl_ty =
            inh.normalize_associated_types_in(impl_c_span, impl_c_hir_id, param_env, impl_ty);

        debug!("compare_const_impl: impl_ty={:?}", impl_ty);

        let trait_ty =
            inh.normalize_associated_types_in(impl_c_span, impl_c_hir_id, param_env, trait_ty);

        debug!("compare_const_impl: trait_ty={:?}", trait_ty);

        let err = infcx
            .at(&cause, param_env)
            .sup(trait_ty, impl_ty)
            .map(|ok| inh.register_infer_ok_obligations(ok));

        if let Err(terr) = err {
            debug!(
                "checking associated const for compatibility: impl ty {:?}, trait ty {:?}",
                impl_ty, trait_ty
            );

            // Locate the Span containing just the type of the offending impl
            match tcx.hir().expect_impl_item(impl_c_hir_id).kind {
                ImplItemKind::Const(ref ty, _) => cause.make_mut().span = ty.span,
                _ => bug!("{:?} is not a impl const", impl_c),
            }

            let mut diag = struct_span_err!(
                tcx.sess,
                cause.span,
                E0326,
                "implemented const `{}` has an incompatible type for trait",
                trait_c.ident
            );

            let trait_c_hir_id =
                trait_c.def_id.as_local().map(|def_id| tcx.hir().local_def_id_to_hir_id(def_id));
            let trait_c_span = trait_c_hir_id.map(|trait_c_hir_id| {
                // Add a label to the Span containing just the type of the const
                match tcx.hir().expect_trait_item(trait_c_hir_id).kind {
                    TraitItemKind::Const(ref ty, _) => ty.span,
                    _ => bug!("{:?} is not a trait const", trait_c),
                }
            });

            infcx.note_type_err(
                &mut diag,
                &cause,
                trait_c_span.map(|span| (span, "type in trait".to_owned())),
                Some(infer::ValuePairs::Types(ExpectedFound {
                    expected: trait_ty,
                    found: impl_ty,
                })),
                &terr,
                false,
            );
            diag.emit();
        }

        // Check that all obligations are satisfied by the implementation's
        // version.
        if let Err(ref errors) = inh.fulfillment_cx.borrow_mut().select_all_or_error(&infcx) {
            infcx.report_fulfillment_errors(errors, None, false);
            return;
        }

        let fcx = FnCtxt::new(&inh, param_env, impl_c_hir_id);
        fcx.regionck_item(impl_c_hir_id, impl_c_span, &[]);
    });
}

crate fn compare_ty_impl<'tcx>(
    tcx: TyCtxt<'tcx>,
    impl_ty: &ty::AssocItem,
    impl_ty_span: Span,
    trait_ty: &ty::AssocItem,
    impl_trait_ref: ty::TraitRef<'tcx>,
    trait_item_span: Option<Span>,
) {
    debug!("compare_impl_type(impl_trait_ref={:?})", impl_trait_ref);

    let _: Result<(), ErrorReported> = (|| {
        compare_number_of_generics(tcx, impl_ty, impl_ty_span, trait_ty, trait_item_span)?;

        compare_type_predicate_entailment(tcx, impl_ty, impl_ty_span, trait_ty, impl_trait_ref)?;

        check_type_bounds(tcx, trait_ty, impl_ty, impl_ty_span, impl_trait_ref)
    })();
}

/// The equivalent of [compare_predicate_entailment], but for associated types
/// instead of associated functions.
fn compare_type_predicate_entailment<'tcx>(
    tcx: TyCtxt<'tcx>,
    impl_ty: &ty::AssocItem,
    impl_ty_span: Span,
    trait_ty: &ty::AssocItem,
    impl_trait_ref: ty::TraitRef<'tcx>,
) -> Result<(), ErrorReported> {
    let impl_substs = InternalSubsts::identity_for_item(tcx, impl_ty.def_id);
    let trait_to_impl_substs =
        impl_substs.rebase_onto(tcx, impl_ty.container.id(), impl_trait_ref.substs);

    let impl_ty_generics = tcx.generics_of(impl_ty.def_id);
    let trait_ty_generics = tcx.generics_of(trait_ty.def_id);
    let impl_ty_predicates = tcx.predicates_of(impl_ty.def_id);
    let trait_ty_predicates = tcx.predicates_of(trait_ty.def_id);

    check_region_bounds_on_impl_item(
        tcx,
        impl_ty_span,
        impl_ty,
        trait_ty,
        &trait_ty_generics,
        &impl_ty_generics,
    )?;

    let impl_ty_own_bounds = impl_ty_predicates.instantiate_own(tcx, impl_substs);

    if impl_ty_own_bounds.is_empty() {
        // Nothing to check.
        return Ok(());
    }

    // This `HirId` should be used for the `body_id` field on each
    // `ObligationCause` (and the `FnCtxt`). This is what
    // `regionck_item` expects.
    let impl_ty_hir_id = tcx.hir().local_def_id_to_hir_id(impl_ty.def_id.expect_local());
    let cause = ObligationCause::new(
        impl_ty_span,
        impl_ty_hir_id,
        ObligationCauseCode::CompareImplTypeObligation {
            item_name: impl_ty.ident.name,
            impl_item_def_id: impl_ty.def_id,
            trait_item_def_id: trait_ty.def_id,
        },
    );

    debug!("compare_type_predicate_entailment: trait_to_impl_substs={:?}", trait_to_impl_substs);

    // The predicates declared by the impl definition, the trait and the
    // associated type in the trait are assumed.
    let impl_predicates = tcx.predicates_of(impl_ty_predicates.parent.unwrap());
    let mut hybrid_preds = impl_predicates.instantiate_identity(tcx);
    hybrid_preds
        .predicates
        .extend(trait_ty_predicates.instantiate_own(tcx, trait_to_impl_substs).predicates);

    debug!("compare_type_predicate_entailment: bounds={:?}", hybrid_preds);

    let normalize_cause = traits::ObligationCause::misc(impl_ty_span, impl_ty_hir_id);
    let param_env =
        ty::ParamEnv::new(tcx.intern_predicates(&hybrid_preds.predicates), Reveal::UserFacing);
    let param_env = traits::normalize_param_env_or_error(
        tcx,
        impl_ty.def_id,
        param_env,
        normalize_cause.clone(),
    );
    tcx.infer_ctxt().enter(|infcx| {
        let inh = Inherited::new(infcx, impl_ty.def_id.expect_local());
        let infcx = &inh.infcx;

        debug!("compare_type_predicate_entailment: caller_bounds={:?}", param_env.caller_bounds());

        let mut selcx = traits::SelectionContext::new(&infcx);

        for predicate in impl_ty_own_bounds.predicates {
            let traits::Normalized { value: predicate, obligations } =
                traits::normalize(&mut selcx, param_env, normalize_cause.clone(), predicate);

            inh.register_predicates(obligations);
            inh.register_predicate(traits::Obligation::new(cause.clone(), param_env, predicate));
        }

        // Check that all obligations are satisfied by the implementation's
        // version.
        if let Err(ref errors) = inh.fulfillment_cx.borrow_mut().select_all_or_error(&infcx) {
            infcx.report_fulfillment_errors(errors, None, false);
            return Err(ErrorReported);
        }

        // Finally, resolve all regions. This catches wily misuses of
        // lifetime parameters.
        let fcx = FnCtxt::new(&inh, param_env, impl_ty_hir_id);
        fcx.regionck_item(impl_ty_hir_id, impl_ty_span, &[]);

        Ok(())
    })
}

/// Validate that `ProjectionCandidate`s created for this associated type will
/// be valid.
///
/// Usually given
///
/// trait X { type Y: Copy } impl X for T { type Y = S; }
///
/// We are able to normalize `<T as X>::U` to `S`, and so when we check the
/// impl is well-formed we have to prove `S: Copy`.
///
/// For default associated types the normalization is not possible (the value
/// from the impl could be overridden). We also can't normalize generic
/// associated types (yet) because they contain bound parameters.
pub fn check_type_bounds<'tcx>(
    tcx: TyCtxt<'tcx>,
    trait_ty: &ty::AssocItem,
    impl_ty: &ty::AssocItem,
    impl_ty_span: Span,
    impl_trait_ref: ty::TraitRef<'tcx>,
) -> Result<(), ErrorReported> {
    // Given
    //
    // impl<A, B> Foo<u32> for (A, B) {
    //     type Bar<C> =...
    // }
    //
    // - `impl_substs` would be `[A, B, C]`
    // - `rebased_substs` would be `[(A, B), u32, C]`, combining the substs from
    //    the *trait* with the generic associated type parameters.
    let impl_ty_substs = InternalSubsts::identity_for_item(tcx, impl_ty.def_id);
    let rebased_substs =
        impl_ty_substs.rebase_onto(tcx, impl_ty.container.id(), impl_trait_ref.substs);
    let impl_ty_value = tcx.type_of(impl_ty.def_id);

    let param_env = tcx.param_env(impl_ty.def_id);

    // When checking something like
    //
    // trait X { type Y: PartialEq<<Self as X>::Y> }
    // impl X for T { default type Y = S; }
    //
    // We will have to prove the bound S: PartialEq<<T as X>::Y>. In this case
    // we want <T as X>::Y to normalize to S. This is valid because we are
    // checking the default value specifically here. Add this equality to the
    // ParamEnv for normalization specifically.
    let normalize_param_env = {
        let mut predicates = param_env.caller_bounds().iter().collect::<Vec<_>>();
        match impl_ty_value.kind() {
            ty::Projection(proj)
                if proj.item_def_id == trait_ty.def_id && proj.substs == rebased_substs =>
            {
                // Don't include this predicate if the projected type is
                // exactly the same as the projection. This can occur in
                // (somewhat dubious) code like this:
                //
                // impl<T> X for T where T: X { type Y = <T as X>::Y; }
            }
            _ => predicates.push(
                ty::Binder::dummy(ty::ProjectionPredicate {
                    projection_ty: ty::ProjectionTy {
                        item_def_id: trait_ty.def_id,
                        substs: rebased_substs,
                    },
                    ty: impl_ty_value,
                })
                .to_predicate(tcx),
            ),
        };
        ty::ParamEnv::new(tcx.intern_predicates(&predicates), Reveal::UserFacing)
    };

    tcx.infer_ctxt().enter(move |infcx| {
        let inh = Inherited::new(infcx, impl_ty.def_id.expect_local());
        let infcx = &inh.infcx;
        let mut selcx = traits::SelectionContext::new(&infcx);

        let impl_ty_hir_id = tcx.hir().local_def_id_to_hir_id(impl_ty.def_id.expect_local());
        let normalize_cause = traits::ObligationCause::misc(impl_ty_span, impl_ty_hir_id);
        let mk_cause = |span| {
            ObligationCause::new(
                impl_ty_span,
                impl_ty_hir_id,
                ObligationCauseCode::BindingObligation(trait_ty.def_id, span),
            )
        };

        let obligations = tcx
            .explicit_item_bounds(trait_ty.def_id)
            .iter()
            .map(|&(bound, span)| {
                let concrete_ty_bound = bound.subst(tcx, rebased_substs);
                debug!("check_type_bounds: concrete_ty_bound = {:?}", concrete_ty_bound);

                traits::Obligation::new(mk_cause(span), param_env, concrete_ty_bound)
            })
            .collect();
        debug!("check_type_bounds: item_bounds={:?}", obligations);

        for mut obligation in util::elaborate_obligations(tcx, obligations) {
            let traits::Normalized { value: normalized_predicate, obligations } = traits::normalize(
                &mut selcx,
                normalize_param_env,
                normalize_cause.clone(),
                obligation.predicate,
            );
            debug!("compare_projection_bounds: normalized predicate = {:?}", normalized_predicate);
            obligation.predicate = normalized_predicate;

            inh.register_predicates(obligations);
            inh.register_predicate(obligation);
        }

        // Check that all obligations are satisfied by the implementation's
        // version.
        if let Err(ref errors) = inh.fulfillment_cx.borrow_mut().select_all_or_error(&infcx) {
            infcx.report_fulfillment_errors(errors, None, false);
            return Err(ErrorReported);
        }

        // Finally, resolve all regions. This catches wily misuses of
        // lifetime parameters.
        let fcx = FnCtxt::new(&inh, param_env, impl_ty_hir_id);
        let implied_bounds = match impl_ty.container {
            ty::TraitContainer(_) => vec![],
            ty::ImplContainer(def_id) => fcx.impl_implied_bounds(def_id, impl_ty_span),
        };
        fcx.regionck_item(impl_ty_hir_id, impl_ty_span, &implied_bounds);

        Ok(())
    })
}

fn assoc_item_kind_str(impl_item: &ty::AssocItem) -> &'static str {
    match impl_item.kind {
        ty::AssocKind::Const => "const",
        ty::AssocKind::Fn => "method",
        ty::AssocKind::Type => "type",
    }
}
