pub use super::ffi::*;

use rustc_index::vec::IndexVec;
use rustc_middle::mir::coverage::{
    CodeRegion, CounterValueReference, ExpressionOperandId, InjectedExpressionId,
    InjectedExpressionIndex, MappedExpressionIndex, Op,
};
use rustc_middle::ty::Instance;
use rustc_middle::ty::TyCtxt;

#[derive(Clone, Debug)]
pub struct Expression {
    lhs: ExpressionOperandId,
    op: Op,
    rhs: ExpressionOperandId,
    region: Option<CodeRegion>,
}

/// Collects all of the coverage regions associated with (a) injected counters, (b) counter
/// expressions (additions or subtraction), and (c) unreachable regions (always counted as zero),
/// for a given Function. Counters and counter expressions have non-overlapping `id`s because they
/// can both be operands in an expression. This struct also stores the `function_source_hash`,
/// computed during instrumentation, and forwarded with counters.
///
/// Note, it may be important to understand LLVM's definitions of `unreachable` regions versus "gap
/// regions" (or "gap areas"). A gap region is a code region within a counted region (either counter
/// or expression), but the line or lines in the gap region are not executable (such as lines with
/// only whitespace or comments). According to LLVM Code Coverage Mapping documentation, "A count
/// for a gap area is only used as the line execution count if there are no other regions on a
/// line."
pub struct FunctionCoverage<'tcx> {
    instance: Instance<'tcx>,
    source_hash: u64,
    counters: IndexVec<CounterValueReference, Option<CodeRegion>>,
    expressions: IndexVec<InjectedExpressionIndex, Option<Expression>>,
    unreachable_regions: Vec<CodeRegion>,
}

impl<'tcx> FunctionCoverage<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> Self {
        let coverageinfo = tcx.coverageinfo(instance.def_id());
        debug!(
            "FunctionCoverage::new(instance={:?}) has coverageinfo={:?}",
            instance, coverageinfo
        );
        Self {
            instance,
            source_hash: 0, // will be set with the first `add_counter()`
            counters: IndexVec::from_elem_n(None, coverageinfo.num_counters as usize),
            expressions: IndexVec::from_elem_n(None, coverageinfo.num_expressions as usize),
            unreachable_regions: Vec::new(),
        }
    }

    /// Sets the function source hash value. If called multiple times for the same function, all
    /// calls should have the same hash value.
    pub fn set_function_source_hash(&mut self, source_hash: u64) {
        if self.source_hash == 0 {
            self.source_hash = source_hash;
        } else {
            debug_assert_eq!(source_hash, self.source_hash);
        }
    }

    /// Adds a code region to be counted by an injected counter intrinsic.
    pub fn add_counter(&mut self, id: CounterValueReference, region: CodeRegion) {
        self.counters[id].replace(region).expect_none("add_counter called with duplicate `id`");
    }

    /// Both counters and "counter expressions" (or simply, "expressions") can be operands in other
    /// expressions. Expression IDs start from `u32::MAX` and go down, so the range of expression
    /// IDs will not overlap with the range of counter IDs. Counters and expressions can be added in
    /// any order, and expressions can still be assigned contiguous (though descending) IDs, without
    /// knowing what the last counter ID will be.
    ///
    /// When storing the expression data in the `expressions` vector in the `FunctionCoverage`
    /// struct, its vector index is computed, from the given expression ID, by subtracting from
    /// `u32::MAX`.
    ///
    /// Since the expression operands (`lhs` and `rhs`) can reference either counters or
    /// expressions, an operand that references an expression also uses its original ID, descending
    /// from `u32::MAX`. Theses operands are translated only during code generation, after all
    /// counters and expressions have been added.
    pub fn add_counter_expression(
        &mut self,
        expression_id: InjectedExpressionId,
        lhs: ExpressionOperandId,
        op: Op,
        rhs: ExpressionOperandId,
        region: Option<CodeRegion>,
    ) {
        debug!(
            "add_counter_expression({:?}, lhs={:?}, op={:?}, rhs={:?} at {:?}",
            expression_id, lhs, op, rhs, region
        );
        let expression_index = self.expression_index(u32::from(expression_id));
        self.expressions[expression_index]
            .replace(Expression { lhs, op, rhs, region })
            .expect_none("add_counter_expression called with duplicate `id_descending_from_max`");
    }

    /// Add a region that will be marked as "unreachable", with a constant "zero counter".
    pub fn add_unreachable_region(&mut self, region: CodeRegion) {
        self.unreachable_regions.push(region)
    }

    /// Return the source hash, generated from the HIR node structure, and used to indicate whether
    /// or not the source code structure changed between different compilations.
    pub fn source_hash(&self) -> u64 {
        self.source_hash
    }

    /// Generate an array of CounterExpressions, and an iterator over all `Counter`s and their
    /// associated `Regions` (from which the LLVM-specific `CoverageMapGenerator` will create
    /// `CounterMappingRegion`s.
    pub fn get_expressions_and_counter_regions<'a>(
        &'a self,
    ) -> (Vec<CounterExpression>, impl Iterator<Item = (Counter, &'a CodeRegion)>) {
        assert!(
            self.source_hash != 0,
            "No counters provided the source_hash for function: {:?}",
            self.instance
        );

        let counter_regions = self.counter_regions();
        let (counter_expressions, expression_regions) = self.expressions_with_regions();
        let unreachable_regions = self.unreachable_regions();

        let counter_regions =
            counter_regions.chain(expression_regions.into_iter().chain(unreachable_regions));
        (counter_expressions, counter_regions)
    }

    fn counter_regions<'a>(&'a self) -> impl Iterator<Item = (Counter, &'a CodeRegion)> {
        self.counters.iter_enumerated().filter_map(|(index, entry)| {
            // Option::map() will return None to filter out missing counters. This may happen
            // if, for example, a MIR-instrumented counter is removed during an optimization.
            entry.as_ref().map(|region| {
                (Counter::counter_value_reference(index as CounterValueReference), region)
            })
        })
    }

    fn expressions_with_regions(
        &'a self,
    ) -> (Vec<CounterExpression>, impl Iterator<Item = (Counter, &'a CodeRegion)>) {
        let mut counter_expressions = Vec::with_capacity(self.expressions.len());
        let mut expression_regions = Vec::with_capacity(self.expressions.len());
        let mut new_indexes = IndexVec::from_elem_n(None, self.expressions.len());

        // This closure converts any `Expression` operand (`lhs` or `rhs` of the `Op::Add` or
        // `Op::Subtract` operation) into its native `llvm::coverage::Counter::CounterKind` type
        // and value. Operand ID value `0` maps to `CounterKind::Zero`; values in the known range
        // of injected LLVM counters map to `CounterKind::CounterValueReference` (and the value
        // matches the injected counter index); and any other value is converted into a
        // `CounterKind::Expression` with the expression's `new_index`.
        //
        // Expressions will be returned from this function in a sequential vector (array) of
        // `CounterExpression`, so the expression IDs must be mapped from their original,
        // potentially sparse set of indexes, originally in reverse order from `u32::MAX`.
        //
        // An `Expression` as an operand will have already been encountered as an `Expression` with
        // operands, so its new_index will already have been generated (as a 1-up index value).
        // (If an `Expression` as an operand does not have a corresponding new_index, it was
        // probably optimized out, after the expression was injected into the MIR, so it will
        // get a `CounterKind::Zero` instead.)
        //
        // In other words, an `Expression`s at any given index can include other expressions as
        // operands, but expression operands can only come from the subset of expressions having
        // `expression_index`s lower than the referencing `Expression`. Therefore, it is
        // reasonable to look up the new index of an expression operand while the `new_indexes`
        // vector is only complete up to the current `ExpressionIndex`.
        let id_to_counter = |new_indexes: &IndexVec<
            InjectedExpressionIndex,
            Option<MappedExpressionIndex>,
        >,
                             id: ExpressionOperandId| {
            if id == ExpressionOperandId::ZERO {
                Some(Counter::zero())
            } else if id.index() < self.counters.len() {
                // Note: Some codegen-injected Counters may be only referenced by `Expression`s,
                // and may not have their own `CodeRegion`s,
                let index = CounterValueReference::from(id.index());
                Some(Counter::counter_value_reference(index))
            } else {
                let index = self.expression_index(u32::from(id));
                self.expressions
                    .get(index)
                    .expect("expression id is out of range")
                    .as_ref()
                    // If an expression was optimized out, assume it would have produced a count
                    // of zero. This ensures that expressions dependent on optimized-out
                    // expressions are still valid.
                    .map_or(Some(Counter::zero()), |_| new_indexes[index].map(Counter::expression))
            }
        };

        for (original_index, expression) in
            self.expressions.iter_enumerated().filter_map(|(original_index, entry)| {
                // Option::map() will return None to filter out missing expressions. This may happen
                // if, for example, a MIR-instrumented expression is removed during an optimization.
                entry.as_ref().map(|expression| (original_index, expression))
            })
        {
            let optional_region = &expression.region;
            let Expression { lhs, op, rhs, .. } = *expression;

            if let Some(Some((lhs_counter, rhs_counter))) =
                id_to_counter(&new_indexes, lhs).map(|lhs_counter| {
                    id_to_counter(&new_indexes, rhs).map(|rhs_counter| (lhs_counter, rhs_counter))
                })
            {
                debug_assert!(
                    (lhs_counter.id as usize)
                        < usize::max(self.counters.len(), self.expressions.len())
                );
                debug_assert!(
                    (rhs_counter.id as usize)
                        < usize::max(self.counters.len(), self.expressions.len())
                );
                // Both operands exist. `Expression` operands exist in `self.expressions` and have
                // been assigned a `new_index`.
                let mapped_expression_index =
                    MappedExpressionIndex::from(counter_expressions.len());
                let expression = CounterExpression::new(
                    lhs_counter,
                    match op {
                        Op::Add => ExprKind::Add,
                        Op::Subtract => ExprKind::Subtract,
                    },
                    rhs_counter,
                );
                debug!(
                    "Adding expression {:?} = {:?}, region: {:?}",
                    mapped_expression_index, expression, optional_region
                );
                counter_expressions.push(expression);
                new_indexes[original_index] = Some(mapped_expression_index);
                if let Some(region) = optional_region {
                    expression_regions.push((Counter::expression(mapped_expression_index), region));
                }
            } else {
                debug!(
                    "Ignoring expression with one or more missing operands: \
                    original_index={:?}, lhs={:?}, op={:?}, rhs={:?}, region={:?}",
                    original_index, lhs, op, rhs, optional_region,
                )
            }
        }
        (counter_expressions, expression_regions.into_iter())
    }

    fn unreachable_regions<'a>(&'a self) -> impl Iterator<Item = (Counter, &'a CodeRegion)> {
        self.unreachable_regions.iter().map(|region| (Counter::zero(), region))
    }

    fn expression_index(&self, id_descending_from_max: u32) -> InjectedExpressionIndex {
        debug_assert!(id_descending_from_max >= self.counters.len() as u32);
        InjectedExpressionIndex::from(u32::MAX - id_descending_from_max)
    }
}
