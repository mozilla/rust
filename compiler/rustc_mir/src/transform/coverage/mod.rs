pub mod query;

mod counters;
mod debug;
mod graph;
mod spans;

use counters::CoverageCounters;
use graph::{BasicCoverageBlock, BasicCoverageBlockData, CoverageGraph};
use spans::{CoverageSpan, CoverageSpans};

use crate::transform::MirPass;
use crate::util::pretty;

use rustc_data_structures::fingerprint::Fingerprint;
use rustc_data_structures::graph::WithNumNodes;
use rustc_data_structures::stable_hasher::{HashStable, StableHasher};
use rustc_data_structures::sync::Lrc;
use rustc_index::vec::IndexVec;
use rustc_middle::hir;
use rustc_middle::hir::map::blocks::FnLikeNode;
use rustc_middle::ich::StableHashingContext;
use rustc_middle::mir::coverage::*;
use rustc_middle::mir::{
    self, BasicBlock, BasicBlockData, Coverage, SourceInfo, Statement, StatementKind, Terminator,
    TerminatorKind,
};
use rustc_middle::ty::TyCtxt;
use rustc_span::def_id::DefId;
use rustc_span::{CharPos, Pos, SourceFile, Span, Symbol};

/// A simple error message wrapper for `coverage::Error`s.
#[derive(Debug)]
pub(crate) struct Error {
    message: String,
}

impl Error {
    pub fn from_string<T>(message: String) -> Result<T, Error> {
        Err(Self { message })
    }
}

/// Inserts `StatementKind::Coverage` statements that either instrument the binary with injected
/// counters, via intrinsic `llvm.instrprof.increment`, and/or inject metadata used during codegen
/// to construct the coverage map.
pub struct InstrumentCoverage;

impl<'tcx> MirPass<'tcx> for InstrumentCoverage {
    fn run_pass(&self, tcx: TyCtxt<'tcx>, mir_body: &mut mir::Body<'tcx>) {
        let mir_source = mir_body.source;

        // If the InstrumentCoverage pass is called on promoted MIRs, skip them.
        // See: https://github.com/rust-lang/rust/pull/73011#discussion_r438317601
        if mir_source.promoted.is_some() {
            trace!(
                "InstrumentCoverage skipped for {:?} (already promoted for Miri evaluation)",
                mir_source.def_id()
            );
            return;
        }

        let hir_id = tcx.hir().local_def_id_to_hir_id(mir_source.def_id().expect_local());
        let is_fn_like = FnLikeNode::from_node(tcx.hir().get(hir_id)).is_some();

        // Only instrument functions, methods, and closures (not constants since they are evaluated
        // at compile time by Miri).
        // FIXME(#73156): Handle source code coverage in const eval, but note, if and when const
        // expressions get coverage spans, we will probably have to "carve out" space for const
        // expressions from coverage spans in enclosing MIR's, like we do for closures. (That might
        // be tricky if const expressions have no corresponding statements in the enclosing MIR.
        // Closures are carved out by their initial `Assign` statement.)
        if !is_fn_like {
            trace!("InstrumentCoverage skipped for {:?} (not an FnLikeNode)", mir_source.def_id());
            return;
        }
        // FIXME(richkadel): By comparison, the MIR pass `ConstProp` includes associated constants,
        // with functions, methods, and closures. I assume Miri is used for associated constants as
        // well. If not, we may need to include them here too.

        trace!("InstrumentCoverage starting for {:?}", mir_source.def_id());
        Instrumentor::new(&self.name(), tcx, mir_body).inject_counters();
        trace!("InstrumentCoverage starting for {:?}", mir_source.def_id());
    }
}

struct Instrumentor<'a, 'tcx> {
    pass_name: &'a str,
    tcx: TyCtxt<'tcx>,
    mir_body: &'a mut mir::Body<'tcx>,
    body_span: Span,
    basic_coverage_blocks: CoverageGraph,
    coverage_counters: CoverageCounters,
}

impl<'a, 'tcx> Instrumentor<'a, 'tcx> {
    fn new(pass_name: &'a str, tcx: TyCtxt<'tcx>, mir_body: &'a mut mir::Body<'tcx>) -> Self {
        let hir_body = hir_body(tcx, mir_body.source.def_id());
        let body_span = hir_body.value.span;
        let function_source_hash = hash_mir_source(tcx, hir_body);
        let basic_coverage_blocks = CoverageGraph::from_mir(mir_body);
        Self {
            pass_name,
            tcx,
            mir_body,
            body_span,
            basic_coverage_blocks,
            coverage_counters: CoverageCounters::new(function_source_hash),
        }
    }

    fn inject_counters(&'a mut self) {
        let tcx = self.tcx;
        let source_map = tcx.sess.source_map();
        let mir_source = self.mir_body.source;
        let def_id = mir_source.def_id();
        let body_span = self.body_span;

        debug!("instrumenting {:?}, span: {}", def_id, source_map.span_to_string(body_span));

        let mut graphviz_data = debug::GraphvizData::new();
        let mut debug_used_expressions = debug::UsedExpressions::new();

        let dump_graphviz = tcx.sess.opts.debugging_opts.dump_mir_graphviz;
        if dump_graphviz {
            graphviz_data.enable();
            self.coverage_counters.enable_debug();
        }

        if dump_graphviz || level_enabled!(tracing::Level::DEBUG) {
            debug_used_expressions.enable();
        }

        ////////////////////////////////////////////////////
        // Compute `CoverageSpan`s from the `CoverageGraph`.
        let coverage_spans = CoverageSpans::generate_coverage_spans(
            &self.mir_body,
            body_span,
            &self.basic_coverage_blocks,
        );

        if pretty::dump_enabled(tcx, self.pass_name, def_id) {
            debug::dump_coverage_spanview(
                tcx,
                self.mir_body,
                &self.basic_coverage_blocks,
                self.pass_name,
                &coverage_spans,
            );
        }

        ////////////////////////////////////////////////////
        // Create an optimized mix of `Counter`s and `Expression`s for the `CoverageGraph`. Ensure
        // every `CoverageSpan` has a `Counter` or `Expression` assigned to its `BasicCoverageBlock`
        // and all `Expression` dependencies (operands) are also generated, for any other
        // `BasicCoverageBlock`s not already associated with a `CoverageSpan`.
        //
        // Intermediate expressions (used to compute other `Expression` values), which have no
        // direct associate to any `BasicCoverageBlock`, are returned in the method `Result`.
        let intermediate_expressions_or_error = self
            .coverage_counters
            .make_bcb_counters(&mut self.basic_coverage_blocks, &coverage_spans);

        let (result, intermediate_expressions) = match intermediate_expressions_or_error {
            Ok(intermediate_expressions) => {
                // If debugging, add any intermediate expressions (which are not associated with any
                // BCB) to the `debug_used_expressions` map.
                if debug_used_expressions.is_enabled() {
                    for intermediate_expression in &intermediate_expressions {
                        debug_used_expressions.add_expression_operands(intermediate_expression);
                    }
                }

                ////////////////////////////////////////////////////
                // Remove the counter or edge counter from of each `CoverageSpan`s associated
                // `BasicCoverageBlock`, and inject a `Coverage` statement into the MIR.
                self.inject_coverage_span_counters(
                    coverage_spans,
                    &mut graphviz_data,
                    &mut debug_used_expressions,
                );

                ////////////////////////////////////////////////////
                // For any remaining `BasicCoverageBlock` counters (that were not associated with
                // any `CoverageSpan`), inject `Coverage` statements (_without_ code region `Span`s)
                // to ensure `BasicCoverageBlock` counters that other `Expression`s may depend on
                // are in fact counted, even though they don't directly contribute to counting
                // their own independent code region's coverage.
                self.inject_indirect_counters(&mut graphviz_data, &mut debug_used_expressions);

                // Intermediate expressions will be injected as the final step, after generating
                // debug output, if any.
                ////////////////////////////////////////////////////

                (Ok(()), intermediate_expressions)
            }
            Err(e) => (Err(e), Vec::new()),
        };

        if graphviz_data.is_enabled() {
            // Even if there was an error, a partial CoverageGraph can still generate a useful
            // graphviz output.
            debug::dump_coverage_graphviz(
                tcx,
                self.mir_body,
                self.pass_name,
                &self.basic_coverage_blocks,
                &self.coverage_counters.debug_counters,
                &graphviz_data,
                &intermediate_expressions,
                &debug_used_expressions,
            );
        }

        if let Err(e) = result {
            bug!("Error processing: {:?}: {:?}", self.mir_body.source.def_id(), e)
        };

        // Depending on current `debug_options()`, `alert_on_unused_expressions()` could panic, so
        // this check is performed as late as possible, to allow other debug output (logs and dump
        // files), which might be helpful in analyzing unused expressions, to still be generated.
        debug_used_expressions.alert_on_unused_expressions(&self.coverage_counters.debug_counters);

        ////////////////////////////////////////////////////
        // Finally, inject the intermediate expressions collected along the way.
        for intermediate_expression in intermediate_expressions {
            inject_intermediate_expression(self.mir_body, intermediate_expression);
        }
    }

    /// Inject a counter for each `CoverageSpan`. There can be multiple `CoverageSpan`s for a given
    /// BCB, but only one actual counter needs to be incremented per BCB. `bb_counters` maps each
    /// `bcb` to its `Counter`, when injected. Subsequent `CoverageSpan`s for a BCB that already has
    /// a `Counter` will inject an `Expression` instead, and compute its value by adding `ZERO` to
    /// the BCB `Counter` value.
    ///
    /// If debugging, add every BCB `Expression` associated with a `CoverageSpan`s to the
    /// `used_expression_operands` map.
    fn inject_coverage_span_counters(
        &mut self,
        coverage_spans: Vec<CoverageSpan>,
        graphviz_data: &mut debug::GraphvizData,
        debug_used_expressions: &mut debug::UsedExpressions,
    ) {
        let tcx = self.tcx;
        let source_map = tcx.sess.source_map();
        let body_span = self.body_span;
        let source_file = source_map.lookup_source_file(body_span.lo());
        let file_name = Symbol::intern(&source_file.name.to_string());

        let mut bcb_counters = IndexVec::from_elem_n(None, self.basic_coverage_blocks.num_nodes());
        for covspan in coverage_spans {
            let bcb = covspan.bcb;
            let span = covspan.span;
            let counter_kind = if let Some(&counter_operand) = bcb_counters[bcb].as_ref() {
                self.coverage_counters.make_identity_counter(counter_operand)
            } else if let Some(counter_kind) = self.bcb_data_mut(bcb).take_counter() {
                bcb_counters[bcb] = Some(counter_kind.as_operand_id());
                debug_used_expressions.add_expression_operands(&counter_kind);
                counter_kind
            } else {
                bug!("Every BasicCoverageBlock should have a Counter or Expression");
            };
            graphviz_data.add_bcb_coverage_span_with_counter(bcb, &covspan, &counter_kind);
            let some_code_region = if self.is_code_region_redundant(bcb, span, body_span) {
                None
            } else {
                Some(make_code_region(file_name, &source_file, span, body_span))
            };
            inject_statement(self.mir_body, counter_kind, self.bcb_last_bb(bcb), some_code_region);
        }
    }

    /// Returns true if the type of `BasicCoverageBlock` (specifically, it's `BasicBlock`s
    /// `TerminatorKind`) with the given `Span` (relative to the `body_span`) is known to produce
    /// a redundant coverage count.
    ///
    /// There is at least one case for this, and if it's not handled, the last line in a function
    /// will be double-counted.
    ///
    /// If this method returns `true`, the counter (which other `Expressions` may depend on) is
    /// still injected, but without an associated code region.
    fn is_code_region_redundant(
        &self,
        bcb: BasicCoverageBlock,
        span: Span,
        body_span: Span,
    ) -> bool {
        if span.hi() == body_span.hi() {
            // All functions execute a `Return`-terminated `BasicBlock`, regardless of how the
            // function returns; but only some functions also _can_ return after a `Goto` block
            // that ends on the closing brace of the function (with the `Return`). When this
            // happens, the last character is counted 2 (or possibly more) times, when we know
            // the function returned only once (of course). By giving all `Goto` terminators at
            // the end of a function a `non-reportable` code region, they are still counted
            // if appropriate, but they don't increment the line counter, as long as their is
            // also a `Return` on that last line.
            if let TerminatorKind::Goto { .. } = self.bcb_terminator(bcb).kind {
                return true;
            }
        }
        false
    }

    /// `inject_coverage_span_counters()` looped through the `CoverageSpan`s and injected the
    /// counter from the `CoverageSpan`s `BasicCoverageBlock`, removing it from the BCB in the
    /// process (via `take_counter()`).
    ///
    /// Any other counter associated with a `BasicCoverageBlock`, or its incoming edge, but not
    /// associated with a `CoverageSpan`, should only exist if the counter is a `Expression`
    /// dependency (one of the expression operands). Collect them, and inject the additional
    /// counters into the MIR, without a reportable coverage span.
    fn inject_indirect_counters(
        &mut self,
        graphviz_data: &mut debug::GraphvizData,
        debug_used_expressions: &mut debug::UsedExpressions,
    ) {
        let mut bcb_counters_without_direct_coverage_spans = Vec::new();
        for (target_bcb, target_bcb_data) in self.basic_coverage_blocks.iter_enumerated_mut() {
            if let Some(counter_kind) = target_bcb_data.take_counter() {
                bcb_counters_without_direct_coverage_spans.push((None, target_bcb, counter_kind));
            }
            if let Some(edge_counters) = target_bcb_data.take_edge_counters() {
                for (from_bcb, counter_kind) in edge_counters {
                    bcb_counters_without_direct_coverage_spans.push((
                        Some(from_bcb),
                        target_bcb,
                        counter_kind,
                    ));
                }
            }
        }

        // If debug is enabled, validate that every BCB or edge counter not directly associated
        // with a coverage span is at least indirectly associated (it is a dependency of a BCB
        // counter that _is_ associated with a coverage span).
        debug_used_expressions.validate(&bcb_counters_without_direct_coverage_spans);

        for (edge_from_bcb, target_bcb, counter_kind) in bcb_counters_without_direct_coverage_spans
        {
            debug_used_expressions.add_unused_expression_if_not_found(
                &counter_kind,
                edge_from_bcb,
                target_bcb,
            );

            match counter_kind {
                CoverageKind::Counter { .. } => {
                    let inject_to_bb = if let Some(from_bcb) = edge_from_bcb {
                        // The MIR edge starts `from_bb` (the outgoing / last BasicBlock in
                        // `from_bcb`) and ends at `to_bb` (the incoming / first BasicBlock in the
                        // `target_bcb`; also called the `leader_bb`).
                        let from_bb = self.bcb_last_bb(from_bcb);
                        let to_bb = self.bcb_leader_bb(target_bcb);

                        let new_bb = inject_edge_counter_basic_block(self.mir_body, from_bb, to_bb);
                        graphviz_data.set_edge_counter(from_bcb, new_bb, &counter_kind);
                        debug!(
                            "Edge {:?} (last {:?}) -> {:?} (leader {:?}) requires a new MIR \
                            BasicBlock {:?}, for unclaimed edge counter {}",
                            edge_from_bcb,
                            from_bb,
                            target_bcb,
                            to_bb,
                            new_bb,
                            self.format_counter(&counter_kind),
                        );
                        new_bb
                    } else {
                        let target_bb = self.bcb_last_bb(target_bcb);
                        graphviz_data.add_bcb_dependency_counter(target_bcb, &counter_kind);
                        debug!(
                            "{:?} ({:?}) gets a new Coverage statement for unclaimed counter {}",
                            target_bcb,
                            target_bb,
                            self.format_counter(&counter_kind),
                        );
                        target_bb
                    };

                    inject_statement(self.mir_body, counter_kind, inject_to_bb, None);
                }
                CoverageKind::Expression { .. } => {
                    inject_intermediate_expression(self.mir_body, counter_kind)
                }
                _ => bug!("CoverageKind should be a counter"),
            }
        }
    }

    #[inline]
    fn bcb_leader_bb(&self, bcb: BasicCoverageBlock) -> BasicBlock {
        self.bcb_data(bcb).leader_bb()
    }

    #[inline]
    fn bcb_last_bb(&self, bcb: BasicCoverageBlock) -> BasicBlock {
        self.bcb_data(bcb).last_bb()
    }

    #[inline]
    fn bcb_terminator(&self, bcb: BasicCoverageBlock) -> &Terminator<'tcx> {
        self.bcb_data(bcb).terminator(self.mir_body)
    }

    #[inline]
    fn bcb_data(&self, bcb: BasicCoverageBlock) -> &BasicCoverageBlockData {
        &self.basic_coverage_blocks[bcb]
    }

    #[inline]
    fn bcb_data_mut(&mut self, bcb: BasicCoverageBlock) -> &mut BasicCoverageBlockData {
        &mut self.basic_coverage_blocks[bcb]
    }

    #[inline]
    fn format_counter(&self, counter_kind: &CoverageKind) -> String {
        self.coverage_counters.debug_counters.format_counter(counter_kind)
    }
}

fn inject_edge_counter_basic_block(
    mir_body: &mut mir::Body<'tcx>,
    from_bb: BasicBlock,
    to_bb: BasicBlock,
) -> BasicBlock {
    let span = mir_body[from_bb].terminator().source_info.span.shrink_to_hi();
    let new_bb = mir_body.basic_blocks_mut().push(BasicBlockData {
        statements: vec![], // counter will be injected here
        terminator: Some(Terminator {
            source_info: SourceInfo::outermost(span),
            kind: TerminatorKind::Goto { target: to_bb },
        }),
        is_cleanup: false,
    });
    let edge_ref = mir_body[from_bb]
        .terminator_mut()
        .successors_mut()
        .find(|successor| **successor == to_bb)
        .expect("from_bb should have a successor for to_bb");
    *edge_ref = new_bb;
    new_bb
}

fn inject_statement(
    mir_body: &mut mir::Body<'tcx>,
    counter_kind: CoverageKind,
    bb: BasicBlock,
    some_code_region: Option<CodeRegion>,
) {
    debug!(
        "  injecting statement {:?} for {:?} at code region: {:?}",
        counter_kind, bb, some_code_region
    );
    let data = &mut mir_body[bb];
    let source_info = data.terminator().source_info;
    let statement = Statement {
        source_info,
        kind: StatementKind::Coverage(box Coverage {
            kind: counter_kind,
            code_region: some_code_region,
        }),
    };
    data.statements.push(statement);
}

// Non-code expressions are injected into the coverage map, without generating executable code.
fn inject_intermediate_expression(mir_body: &mut mir::Body<'tcx>, expression: CoverageKind) {
    debug_assert!(if let CoverageKind::Expression { .. } = expression { true } else { false });
    debug!("  injecting non-code expression {:?}", expression);
    let inject_in_bb = mir::START_BLOCK;
    let data = &mut mir_body[inject_in_bb];
    let source_info = data.terminator().source_info;
    let statement = Statement {
        source_info,
        kind: StatementKind::Coverage(box Coverage { kind: expression, code_region: None }),
    };
    data.statements.push(statement);
}

/// Convert the Span into its file name, start line and column, and end line and column
fn make_code_region(
    file_name: Symbol,
    source_file: &Lrc<SourceFile>,
    span: Span,
    body_span: Span,
) -> CodeRegion {
    let (start_line, mut start_col) = source_file.lookup_file_pos(span.lo());
    let (end_line, end_col) = if span.hi() == span.lo() {
        let (end_line, mut end_col) = (start_line, start_col);
        // Extend an empty span by one character so the region will be counted.
        let CharPos(char_pos) = start_col;
        if span.hi() == body_span.hi() {
            start_col = CharPos(char_pos - 1);
        } else {
            end_col = CharPos(char_pos + 1);
        }
        (end_line, end_col)
    } else {
        source_file.lookup_file_pos(span.hi())
    };
    CodeRegion {
        file_name,
        start_line: start_line as u32,
        start_col: start_col.to_u32() + 1,
        end_line: end_line as u32,
        end_col: end_col.to_u32() + 1,
    }
}

fn hir_body<'tcx>(tcx: TyCtxt<'tcx>, def_id: DefId) -> &'tcx rustc_hir::Body<'tcx> {
    let hir_node = tcx.hir().get_if_local(def_id).expect("expected DefId is local");
    let fn_body_id = hir::map::associated_body(hir_node).expect("HIR node is a function with body");
    tcx.hir().body(fn_body_id)
}

fn hash_mir_source<'tcx>(tcx: TyCtxt<'tcx>, hir_body: &'tcx rustc_hir::Body<'tcx>) -> u64 {
    let mut hcx = tcx.create_no_span_stable_hashing_context();
    hash(&mut hcx, &hir_body.value).to_smaller_hash()
}

fn hash(
    hcx: &mut StableHashingContext<'tcx>,
    node: &impl HashStable<StableHashingContext<'tcx>>,
) -> Fingerprint {
    let mut stable_hasher = StableHasher::new();
    node.hash_stable(hcx, &mut stable_hasher);
    stable_hasher.finish()
}
