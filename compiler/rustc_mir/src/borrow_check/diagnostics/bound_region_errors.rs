use rustc_errors::DiagnosticBuilder;
use rustc_infer::infer::canonical::Canonical;
use rustc_infer::infer::error_reporting::nice_region_error::NiceRegionError;
use rustc_infer::infer::region_constraints::Constraint;
use rustc_infer::infer::{InferCtxt, RegionResolutionError, SubregionOrigin, TyCtxtInferExt as _};
use rustc_infer::traits::{Normalized, Obligation, ObligationCause, TraitEngine, TraitEngineExt};
use rustc_middle::ty::error::TypeError;
use rustc_middle::ty::{self, Ty, TyCtxt, TypeFoldable};
use rustc_span::Span;
use rustc_trait_selection::traits::query::type_op;
use rustc_trait_selection::traits::{SelectionContext, TraitEngineExt as _};

use std::fmt;
use std::rc::Rc;

use crate::borrow_check::region_infer::values::RegionElement;
use crate::borrow_check::MirBorrowckCtxt;

#[derive(Clone)]
crate struct UniverseInfo<'tcx>(UniverseInfoInner<'tcx>);

/// What operation a universe was created for.
#[derive(Clone)]
enum UniverseInfoInner<'tcx> {
    /// Relating two types which have binders.
    RelateTys { expected: Ty<'tcx>, found: Ty<'tcx> },
    /// Created from performing a `TypeOp`.
    TypeOp(Rc<dyn TypeOpInfo<'tcx> + 'tcx>),
    /// Any other reason.
    Other,
}

impl UniverseInfo<'tcx> {
    crate fn other() -> UniverseInfo<'tcx> {
        UniverseInfo(UniverseInfoInner::Other)
    }

    crate fn relate(expected: Ty<'tcx>, found: Ty<'tcx>) -> UniverseInfo<'tcx> {
        UniverseInfo(UniverseInfoInner::RelateTys { expected, found })
    }

    crate fn report_error(
        &self,
        mbcx: &mut MirBorrowckCtxt<'_, 'tcx>,
        placeholder: ty::PlaceholderRegion,
        error_element: RegionElement,
        span: Span,
    ) {
        match self.0 {
            UniverseInfoInner::RelateTys { expected, found } => {
                let body_id = mbcx.infcx.tcx.hir().local_def_id_to_hir_id(mbcx.mir_def_id());
                let err = mbcx.infcx.report_mismatched_types(
                    &ObligationCause::misc(span, body_id),
                    expected,
                    found,
                    TypeError::RegionsPlaceholderMismatch,
                );
                err.buffer(&mut mbcx.errors_buffer);
            }
            UniverseInfoInner::TypeOp(ref type_op_info) => {
                type_op_info.report_error(mbcx, placeholder, error_element, span);
            }
            UniverseInfoInner::Other => {
                // FIXME: This error message isn't great, but it doesn't show
                // up in the existing UI tests. Consider investigating this
                // some more.
                mbcx.infcx
                    .tcx
                    .sess
                    .struct_span_err(span, "higher-ranked subtype error")
                    .buffer(&mut mbcx.errors_buffer);
            }
        }
    }
}

crate trait ToUniverseInfo<'tcx> {
    fn to_universe_info(self, base_universe: ty::UniverseIndex) -> UniverseInfo<'tcx>;
}

impl<'tcx> ToUniverseInfo<'tcx>
    for Canonical<'tcx, ty::ParamEnvAnd<'tcx, type_op::prove_predicate::ProvePredicate<'tcx>>>
{
    fn to_universe_info(self, base_universe: ty::UniverseIndex) -> UniverseInfo<'tcx> {
        UniverseInfo(UniverseInfoInner::TypeOp(Rc::new(PredicateQuery {
            canonical_query: self,
            base_universe,
        })))
    }
}

impl<'tcx, T: Copy + fmt::Display + TypeFoldable<'tcx> + 'tcx> ToUniverseInfo<'tcx>
    for Canonical<'tcx, ty::ParamEnvAnd<'tcx, type_op::Normalize<T>>>
{
    fn to_universe_info(self, base_universe: ty::UniverseIndex) -> UniverseInfo<'tcx> {
        UniverseInfo(UniverseInfoInner::TypeOp(Rc::new(NormalizeQuery {
            canonical_query: self,
            base_universe,
        })))
    }
}

impl<'tcx> ToUniverseInfo<'tcx>
    for Canonical<'tcx, ty::ParamEnvAnd<'tcx, type_op::AscribeUserType<'tcx>>>
{
    fn to_universe_info(self, _base_universe: ty::UniverseIndex) -> UniverseInfo<'tcx> {
        // Ascribe user type isn't usually called on types that have different
        // bound regions.
        UniverseInfo::other()
    }
}

impl<'tcx, F, G> ToUniverseInfo<'tcx> for Canonical<'tcx, type_op::custom::CustomTypeOp<F, G>> {
    fn to_universe_info(self, _base_universe: ty::UniverseIndex) -> UniverseInfo<'tcx> {
        // We can't rerun custom type ops.
        UniverseInfo::other()
    }
}

#[allow(unused_lifetimes)]
trait TypeOpInfo<'tcx> {
    /// Returns an rrror to be reported if rerunning the type op fails to
    /// recover the error's cause.
    fn fallback_error(&self, tcx: TyCtxt<'tcx>, span: Span) -> DiagnosticBuilder<'tcx>;

    fn base_universe(&self) -> ty::UniverseIndex;

    fn nice_error(
        &self,
        tcx: TyCtxt<'tcx>,
        span: Span,
        placeholder_region: ty::Region<'tcx>,
        error_region: Option<ty::Region<'tcx>>,
    ) -> Option<DiagnosticBuilder<'tcx>>;

    fn report_error(
        &self,
        mbcx: &mut MirBorrowckCtxt<'_, 'tcx>,
        placeholder: ty::PlaceholderRegion,
        error_element: RegionElement,
        span: Span,
    ) {
        let tcx = mbcx.infcx.tcx;
        let base_universe = self.base_universe();

        let adjusted_universe = if let Some(adjusted) =
            placeholder.universe.as_u32().checked_sub(base_universe.as_u32())
        {
            adjusted
        } else {
            self.fallback_error(tcx, span).buffer(&mut mbcx.errors_buffer);
            return;
        };

        let placeholder_region = tcx.mk_region(ty::RePlaceholder(ty::Placeholder {
            name: placeholder.name,
            universe: adjusted_universe.into(),
        }));

        let error_region =
            if let RegionElement::PlaceholderRegion(error_placeholder) = error_element {
                let adjusted_universe =
                    error_placeholder.universe.as_u32().checked_sub(base_universe.as_u32());
                adjusted_universe.map(|adjusted| {
                    tcx.mk_region(ty::RePlaceholder(ty::Placeholder {
                        name: error_placeholder.name,
                        universe: adjusted.into(),
                    }))
                })
            } else {
                None
            };

        debug!(?placeholder_region);

        let nice_error = self.nice_error(tcx, span, placeholder_region, error_region);

        if let Some(nice_error) = nice_error {
            nice_error.buffer(&mut mbcx.errors_buffer);
        } else {
            self.fallback_error(tcx, span).buffer(&mut mbcx.errors_buffer);
        }
    }
}

struct PredicateQuery<'tcx> {
    canonical_query:
        Canonical<'tcx, ty::ParamEnvAnd<'tcx, type_op::prove_predicate::ProvePredicate<'tcx>>>,
    base_universe: ty::UniverseIndex,
}

impl TypeOpInfo<'tcx> for PredicateQuery<'tcx> {
    fn fallback_error(&self, tcx: TyCtxt<'tcx>, span: Span) -> DiagnosticBuilder<'tcx> {
        let mut err = tcx.sess.struct_span_err(span, "higher-ranked lifetime error");
        err.note(&format!("could not prove {}", self.canonical_query.value.value.predicate));
        err
    }

    fn base_universe(&self) -> ty::UniverseIndex {
        self.base_universe
    }

    fn nice_error(
        &self,
        tcx: TyCtxt<'tcx>,
        span: Span,
        placeholder_region: ty::Region<'tcx>,
        error_region: Option<ty::Region<'tcx>>,
    ) -> Option<DiagnosticBuilder<'tcx>> {
        tcx.infer_ctxt().enter_with_canonical(span, &self.canonical_query, |ref infcx, key, _| {
            let mut fulfill_cx = TraitEngine::new(tcx);

            let (param_env, prove_predicate) = key.into_parts();
            fulfill_cx.register_predicate_obligation(
                infcx,
                Obligation::new(
                    ObligationCause::dummy_with_span(span),
                    param_env,
                    prove_predicate.predicate,
                ),
            );

            try_extract_error_from_fulfill_cx(fulfill_cx, infcx, placeholder_region, error_region)
        })
    }
}

struct NormalizeQuery<'tcx, T> {
    canonical_query: Canonical<'tcx, ty::ParamEnvAnd<'tcx, type_op::Normalize<T>>>,
    base_universe: ty::UniverseIndex,
}

impl<T> TypeOpInfo<'tcx> for NormalizeQuery<'tcx, T>
where
    T: Copy + fmt::Display + TypeFoldable<'tcx> + 'tcx,
{
    fn fallback_error(&self, tcx: TyCtxt<'tcx>, span: Span) -> DiagnosticBuilder<'tcx> {
        let mut err = tcx.sess.struct_span_err(span, "higher-ranked lifetime error");
        err.note(&format!("could not normalize `{}`", self.canonical_query.value.value.value));
        err
    }

    fn base_universe(&self) -> ty::UniverseIndex {
        self.base_universe
    }

    fn nice_error(
        &self,
        tcx: TyCtxt<'tcx>,
        span: Span,
        placeholder_region: ty::Region<'tcx>,
        error_region: Option<ty::Region<'tcx>>,
    ) -> Option<DiagnosticBuilder<'tcx>> {
        tcx.infer_ctxt().enter_with_canonical(span, &self.canonical_query, |ref infcx, key, _| {
            let mut fulfill_cx = TraitEngine::new(tcx);

            let mut selcx = SelectionContext::new(infcx);
            let (param_env, value) = key.into_parts();

            let Normalized { value: _, obligations } = rustc_trait_selection::traits::normalize(
                &mut selcx,
                param_env,
                ObligationCause::dummy_with_span(span),
                value.value,
            );
            fulfill_cx.register_predicate_obligations(infcx, obligations);

            try_extract_error_from_fulfill_cx(fulfill_cx, infcx, placeholder_region, error_region)
        })
    }
}

fn try_extract_error_from_fulfill_cx<'tcx>(
    mut fulfill_cx: Box<dyn TraitEngine<'tcx> + 'tcx>,
    infcx: &InferCtxt<'_, 'tcx>,
    placeholder_region: ty::Region<'tcx>,
    error_region: Option<ty::Region<'tcx>>,
) -> Option<DiagnosticBuilder<'tcx>> {
    let tcx = infcx.tcx;

    // We generally shouldn't have here because the query was
    // already run, but there's no point using `delay_span_bug`
    // when we're going to emit an error here anyway.
    let _errors = fulfill_cx.select_all_or_error(infcx).err().unwrap_or_else(Vec::new);

    let region_obligations = infcx.take_registered_region_obligations();
    debug!(?region_obligations);

    let (sub_region, cause) = infcx.with_region_constraints(|region_constraints| {
        debug!(?region_constraints);
        region_constraints.constraints.iter().find_map(|(constraint, cause)| {
            match *constraint {
                Constraint::RegSubReg(sub, sup) if sup == placeholder_region && sup != sub => {
                    Some((sub, cause.clone()))
                }
                // FIXME: Should this check the universe of the var?
                Constraint::VarSubReg(vid, sup) if sup == placeholder_region => {
                    Some((tcx.mk_region(ty::ReVar(vid)), cause.clone()))
                }
                _ => None,
            }
        })
    })?;

    debug!(?sub_region, ?cause);
    let nice_error = match (error_region, sub_region) {
        (Some(error_region), &ty::ReVar(vid)) => NiceRegionError::new(
            infcx,
            RegionResolutionError::SubSupConflict(
                vid,
                infcx.region_var_origin(vid),
                cause.clone(),
                error_region,
                cause.clone(),
                placeholder_region,
            ),
        ),
        (Some(error_region), _) => NiceRegionError::new(
            infcx,
            RegionResolutionError::ConcreteFailure(cause.clone(), error_region, placeholder_region),
        ),
        // Note universe here is wrong...
        (None, &ty::ReVar(vid)) => NiceRegionError::new(
            infcx,
            RegionResolutionError::UpperBoundUniverseConflict(
                vid,
                infcx.region_var_origin(vid),
                infcx.universe_of_region(sub_region),
                cause.clone(),
                placeholder_region,
            ),
        ),
        (None, _) => NiceRegionError::new(
            infcx,
            RegionResolutionError::ConcreteFailure(cause.clone(), sub_region, placeholder_region),
        ),
    };
    nice_error.try_report_from_nll().or_else(|| {
        if let SubregionOrigin::Subtype(trace) = cause {
            Some(
                infcx.report_and_explain_type_error(*trace, &TypeError::RegionsPlaceholderMismatch),
            )
        } else {
            None
        }
    })
}
