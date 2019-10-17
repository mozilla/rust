/// Note: most tests relevant to this file can be found (at the time of writing)
/// in src/tests/ui/pattern/usefulness. Also look out for rfc2008 (feature
/// non_exhaustive) tests.
///
/// # Introduction
///
/// This file includes the logic for exhaustiveness and usefulness checking for
/// pattern-matching. Specifically, given a list of patterns for a type, we can
/// tell whether:
/// (a) the patterns cover every possible constructor for the type [exhaustiveness]
/// (b) each pattern is necessary [usefulness]
///
/// The algorithm implemented here is based on the one described in:
/// http://moscova.inria.fr/~maranget/papers/warn/index.html
/// However, various modifications have been made to it so we keep it only as reference
/// and will describe the extended algorithm here (without being so rigorous).
///
/// The core of the algorithm revolves about a "usefulness" check. In particular, we
/// are trying to compute a predicate `U(P, q)` where `P` is a list of patterns.
/// `U(P, q)` represents whether, given an existing list of patterns
/// `P_1 ..= P_m`, adding a new pattern `q` will be "useful" (that is, cover previously-
/// uncovered values of the type).
///
/// If we have this predicate, then we can easily compute both exhaustiveness of an
/// entire set of patterns and the individual usefulness of each one.
/// (a) the set of patterns is exhaustive iff `U(P, _)` is false (i.e., adding a wildcard
/// match doesn't increase the number of values we're matching)
/// (b) a pattern `P_i` is not useful (i.e. unreachable) if `U(P[0..=(i-1), P_i)` is
/// false (i.e., adding a pattern to those that have come before it doesn't match any value
/// that wasn't matched previously).
///
///
/// # Pattern-stacks and matrices
///
/// The basic datastructure that we will make use of in the algorithm is a list of patterns that
/// the paper calls "pattern-vector" and that we call "pattern-stack". The idea is that we
/// start with a single pattern of interest,
/// and repeatedly unpack the top constructor to reveal its arguments. We keep the yet-untreated
/// arguments in the tail of the stack.
///
/// For example, say we start with the pattern `Foo(Bar(1, 2), Some(true), false)`. The
/// pattern-stack might then evolve as follows:
///   [Foo(Bar(1, 2), Some(_), false)] // Initially we have a single pattern in the stack
///   [Bar(1, 2), Some(_), false] // After unpacking the `Foo` constructor
///   [1, 2, Some(_), false] // After unpacking the `Bar` constructor
///   [2, Some(_), false] // After unpacking the `1` constructor
///   // etc.
///
/// We call the operation of popping the constructor on top of the stack "specialization", and we
/// write it `S(c, p)`, where `p` is a pattern-stack and `c` a specific constructor (like `Some`
/// or `None`). This operation returns zero or more altered pattern-stacks, as follows.
/// We look at the pattern `p_1` on top of the stack, and we have four cases:
///      1. `p_1 = c(r_1, .., r_a)`, i.e. the top of the stack has constructor `c`. We push
///         onto the stack the arguments of this constructor, and return the result:
///              r_1, .., r_a, p_2, .., p_n
///      2. `p_1 = c'(r_1, .., r_a')` where `c ≠ c'`. We discard the current stack and return
///         nothing.
///      3. `p_1 = _`. We push onto the stack as many wildcards as the constructor `c`
///         has arguments (its arity), and return the resulting stack:
///              _, .., _, p_2, .., p_n
///      4. `p_1 = r_1 | r_2`. We expand the OR-pattern and then recurse on each resulting stack:
///              S(c, (r_1, p_2, .., p_n))
///              S(c, (r_2, p_2, .., p_n))
///
/// Note that when the required constructor does not match the constructor on top of the stack, we
/// return nothing. Thus specialization filters pattern-stacks by the constructor on top of them.
///
/// We call a list of pattern-stacks a "matrix", because in the run of the algorithm they will
/// keep a rectangular shape. `S` operation extends straightforwardly to matrices by
/// working row-by-row using flat_map.
///
///
/// # Abstract algorithm
///
/// The algorithm itself is a function `U`, that takes as arguments a matrix `M` and a new pattern
/// `p`, both with the same number `n` of columns.
/// The algorithm is inductive (on the number of columns: i.e., components of pattern-stacks).
/// The algorithm is realised in the `is_useful` function.
///
/// Base case. (`n = 0`, i.e., an empty tuple pattern)
///     - If `M` already contains an empty pattern (i.e., if the number of patterns `m > 0`),
///       then `U(M, p)` is false.
///     - Otherwise, `M` must be empty, so `U(M, p)` is true.
///
/// Inductive step. (`n > 0`)
///     We look at `p_1`, the head of the pattern-stack `p`.
///
///     We first generate the list of constructors that are covered by a pattern `pat`. We name
///     this operation `pat_constructors`.
///         - If `pat == c(r_1, .., r_a)`, i.e. we have a constructor pattern. Then we just
///         return `c`:
///             `pat_constructors(pat) = [c]`
///
///         - If `pat == _`, then we return the list of all possible constructors for the
///         relevant type:
///             `pat_constructors(pat) = all_constructors(pat.ty)`
///
///         - If `pat == r_1 | r_2`, then we return the constructors for either branch of the
///         OR-pattern:
///             `pat_constructors(pat) = pat_constructors(r_1) + pat_constructors(r_2)`
///
///     Then for each constructor `c` in `pat_constructors(p_1)`, we want to check whether a value
///     that starts with this constructor may show that `p` is useful, i.e. may match `p` but not
///     be matched by the matrix above.
///     For that, we only care about those rows of `M` whose first component covers the
///     constructor `c`; and for those rows that do, we want to unpack the arguments to `c` to check
///     further that `p` matches additional values.
///     This is where specialization comes in: this check amounts to computing `U(S(c, M), S(c,
///     p))`. More details can be found in the paper.
///
///     Thus we get: `U(M, p) := ∃(c ϵ pat_constructors(p_1)) U(S(c, M), S(c, p))`
///
///     Note that for c ϵ pat_constructors(p_1), `S(c, P)` always returns exactly one element, so
///     the formula above makes sense.
///
/// This algorithm however has a lot of practical issues. Most importantly, it may not terminate
/// for some types with infinitely many inhabitants, because when it encounters a wildcard it will
/// try all the values of the type. And it would be stupidly slow anyways for types with a lot of
/// constructors, like `u64` of `&[bool]`. We therefore present a modified version after the
/// example.
///
///
/// # Example run of the algorithm
///
/// Assume we have the following match. We want to know whether it is exhaustive, i.e. whether
/// an additional `_` pattern would be useful (would be reachable).
/// ```
///     match x {
///         Some(true) => {}
///         None => {}
///     }
/// ```
///
/// We start with the following `M` and `p`:
/// M = [ [Some(true)],
///       [None] ]
/// p =   [_]
/// `pat_constructors(p)` returns `[None, Some]`
///
/// We specialize on the `None` constructor first:
/// S(None, M) = [ [] ]
/// S(None, p) =   []
/// We hit the base case n = 0: since bool is inhabited, `U(S(None, M), S(None, p)) = false`.
///
/// We specialize on the `Some` constructor second:
/// S(Some, M) = [ [true] ]
/// S(Some, p) =   [_]
/// Let M' := S(Some, M) and p' := S(Some, p).
///
/// `pat_constructors(p')` returns `[true, false]`
/// S(true, M') = [ [] ]
/// S(true, p') =   []
/// So `U(S(true, M'), S(true, p')) = false`
///
/// S(false, M') = []
/// S(false, p') = []
/// So `U(S(false, M'), S(false, p')) = true`
///
/// Therefore `U(M, p) = true`, indeed by following the steps taken we can recover that
/// the pattern `Some(false)` was not covered by the initial match.
///
///
/// # Concrete algorithm
///
/// To make the algorithm tractable, we introduce the notion of meta-constructors. A
/// meta-constructor stands for a particular group of constructors. The typical example
/// is the wildcard `_`, which stands for all the constructors of a given type.
///
/// In practice, the meta-constructors we make use of in this file are the following:
///     - any normal constructor is also a meta-constructor with exactly one member;
///     - the wildcard `_`, that captures all constructors of a given type;
///     - the constant range `x..y` that captures a range of values for types that support
///     it, like integers;
///     - the variable-length slice `[x, y, .., z]`, that captures all slice constructors
///     from a given length onwards;
///     - the "missing constructors" meta-constructor, that captures a provided arbitrary group
///     of constructors.
///
/// We first redefine `pat_constructors` to potentially return a meta-constructor when relevant
/// for a pattern.
///
/// We then add a step to the algorithm: a function `split_meta_constructor(mc, M)` that returns
/// a list of meta-constructors, with the following properties:
///     - the set of base constructors covered by the output must be the same as covered by `mc`;
///     - for each meta-constructor `k` in the output, all the `c ϵ k` behave the same relative
///     to `M`. More precisely, we want that for any two `c1` and `c2` in `k`,
///     `U(S(c1, M), S(c1, p))` iff `U(S(c2, M), S(c2, p))`;
///     - if the first column of `M` is only wildcards, then the function returns at most
///     `[mc]` on its own;
///     - if the relevant type is uninhabited, the function returns nothing.
/// Any function that has those properties ensures correctness of the algorithm. We will of course
/// try to pick a function that also ensures good performance.
/// The idea is that we still need to try different constructors, but we try to keep them grouped
/// together when possible to avoid doing redundant work.
///
/// Here is roughly how splitting works for us:
///     - for wildcards, there are two cases:
///         - if all the possible constructors of the relevant type exist in the first column
///         of `M`, then we return the list of all those constructors, like we did before;
///         - if however some constructors are missing, then it turns out that considering
///         those missing constructors is enough. We return a "missing constructors" meta-
///         contructor that carries the missing constructors in question.
///     (Note the similarity with the algorithm from the paper. It is not a coincidence)
///     - for ranges, we split the range into a disjoint set of subranges, see the code for details;
///     - for slices, we split the slice into a number of fixed-length slices and one longer
///     variable-length slice, again see code;
///
/// Thus we get the new inductive step (i.e. when `n > 0`):
///     `U(M, p) :=
///         ∃(mc ϵ pat_constructors(p_1))
///         ∃(mc' ϵ split_meta-constructor(mc, M))
///         U(S(c, M), S(c, p)) for some c ϵ mc'`
/// Note: in the case of an uninhabited type, there won't be any `mc'` so this just returns false.
///
/// Note that the termination of the algorithm now depends on the behaviour of the splitting
/// phase. However, from the third property of the splitting function,
/// we can see that the depth of splitting of the algorithm is bounded by some
/// function of the depths of the patterns fed to it initially. So we're confident that
/// it terminates.
///
/// This algorithm is equivalent to the one presented in the paper if we only consider
/// wildcards. Thus this mostly extends the original algorithm to ranges and variable-length
/// slices, while removing the special-casing of the wildcard pattern. We also additionally
/// support uninhabited types.
use self::Constructor::*;
use self::Usefulness::*;
use self::WitnessPreference::*;

use rustc_data_structures::fx::FxHashSet;
use rustc_index::vec::Idx;

use super::{compare_const_vals, PatternFoldable, PatternFolder};
use super::{FieldPat, Pat, PatKind, PatRange};

use rustc::hir::def_id::DefId;
use rustc::hir::RangeEnd;
use rustc::ty::layout::{Integer, IntegerExt, Size, VariantIdx};
use rustc::ty::{self, Const, Ty, TyCtxt, TypeFoldable};

use rustc::mir::interpret::{truncate, AllocId, ConstValue, Pointer, Scalar};
use rustc::mir::Field;
use rustc::util::captures::Captures;
use rustc::util::common::ErrorReported;

use syntax::attr::{SignedInt, UnsignedInt};
use syntax_pos::{Span, DUMMY_SP};

use arena::TypedArena;

use smallvec::{smallvec, SmallVec};
use std::cmp::{self, max, min, Ordering};
use std::convert::TryInto;
use std::fmt;
use std::iter::{FromIterator, IntoIterator};
use std::ops::RangeInclusive;
use std::u128;

pub fn expand_pattern<'a, 'tcx>(cx: &MatchCheckCtxt<'a, 'tcx>, pat: Pat<'tcx>) -> &'a Pat<'tcx> {
    cx.pattern_arena.alloc(LiteralExpander { tcx: cx.tcx }.fold_pattern(&pat))
}

struct LiteralExpander<'tcx> {
    tcx: TyCtxt<'tcx>,
}

impl LiteralExpander<'tcx> {
    /// Derefs `val` and potentially unsizes the value if `crty` is an array and `rty` a slice.
    ///
    /// `crty` and `rty` can differ because you can use array constants in the presence of slice
    /// patterns. So the pattern may end up being a slice, but the constant is an array. We convert
    /// the array to a slice in that case.
    fn fold_const_value_deref(
        &mut self,
        val: ConstValue<'tcx>,
        // the pattern's pointee type
        rty: Ty<'tcx>,
        // the constant's pointee type
        crty: Ty<'tcx>,
    ) -> ConstValue<'tcx> {
        debug!("fold_const_value_deref {:?} {:?} {:?}", val, rty, crty);
        match (val, &crty.kind, &rty.kind) {
            // the easy case, deref a reference
            (ConstValue::Scalar(Scalar::Ptr(p)), x, y) if x == y => {
                let alloc = self.tcx.alloc_map.lock().unwrap_memory(p.alloc_id);
                ConstValue::ByRef { alloc, offset: p.offset }
            }
            // unsize array to slice if pattern is array but match value or other patterns are slice
            (ConstValue::Scalar(Scalar::Ptr(p)), ty::Array(t, n), ty::Slice(u)) => {
                assert_eq!(t, u);
                ConstValue::Slice {
                    data: self.tcx.alloc_map.lock().unwrap_memory(p.alloc_id),
                    start: p.offset.bytes().try_into().unwrap(),
                    end: n.eval_usize(self.tcx, ty::ParamEnv::empty()).try_into().unwrap(),
                }
            }
            // fat pointers stay the same
            (ConstValue::Slice { .. }, _, _)
            | (_, ty::Slice(_), ty::Slice(_))
            | (_, ty::Str, ty::Str) => val,
            // FIXME(oli-obk): this is reachable for `const FOO: &&&u32 = &&&42;` being used
            _ => bug!("cannot deref {:#?}, {} -> {}", val, crty, rty),
        }
    }
}

impl PatternFolder<'tcx> for LiteralExpander<'tcx> {
    fn fold_pattern(&mut self, pat: &Pat<'tcx>) -> Pat<'tcx> {
        debug!("fold_pattern {:?} {:?} {:?}", pat, pat.ty.kind, pat.kind);
        match (&pat.ty.kind, &*pat.kind) {
            (
                &ty::Ref(_, rty, _),
                &PatKind::Constant {
                    value: Const { val, ty: ty::TyS { kind: ty::Ref(_, crty, _), .. } },
                },
            ) => Pat {
                ty: pat.ty,
                span: pat.span,
                kind: box PatKind::Deref {
                    subpattern: Pat {
                        ty: rty,
                        span: pat.span,
                        kind: box PatKind::Constant {
                            value: self.tcx.mk_const(Const {
                                val: self.fold_const_value_deref(*val, rty, crty),
                                ty: rty,
                            }),
                        },
                    },
                },
            },
            (_, &PatKind::Binding { subpattern: Some(ref s), .. }) => s.fold_with(self),
            _ => pat.super_fold_with(self),
        }
    }
}

/// A row of a matrix. Rows of len 1 are very common, which is why `SmallVec[_; 2]`
/// works well.
#[derive(Debug, Clone)]
pub struct PatStack<'p, 'tcx> {
    patterns: SmallVec<[&'p Pat<'tcx>; 2]>,
}

impl<'p, 'tcx> PatStack<'p, 'tcx> {
    pub fn from_pattern(pat: &'p Pat<'tcx>) -> Self {
        PatStack::from_vec(smallvec![pat])
    }

    fn empty() -> Self {
        PatStack::from_vec(smallvec![])
    }

    fn from_vec(vec: SmallVec<[&'p Pat<'tcx>; 2]>) -> Self {
        PatStack { patterns: vec }
    }

    fn from_slice(s: &[&'p Pat<'tcx>]) -> Self {
        PatStack::from_vec(SmallVec::from_slice(s))
    }

    fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    fn len(&self) -> usize {
        self.patterns.len()
    }

    fn head<'a>(&'a self) -> &'p Pat<'tcx> {
        self.patterns[0]
    }

    fn head_ctors(&self, cx: &MatchCheckCtxt<'_, 'tcx>) -> SmallVec<[Constructor<'tcx>; 1]> {
        pat_constructors(cx.tcx, cx.param_env, self.head())
    }

    fn iter(&self) -> impl Iterator<Item = &Pat<'tcx>> {
        self.patterns.iter().map(|p| *p)
    }

    /// This computes `S(constructor, self)`. See top of the file for explanations.
    fn specialize<'a, 'q>(
        &self,
        cx: &MatchCheckCtxt<'a, 'tcx>,
        constructor: &Constructor<'tcx>,
        ctor_wild_subpatterns: &[&'q Pat<'tcx>],
    ) -> SmallVec<[PatStack<'q, 'tcx>; 1]>
    where
        'a: 'q,
        'p: 'q,
    {
        let new_heads = specialize_one_pattern(cx, self.head(), constructor, ctor_wild_subpatterns);
        let result = new_heads
            .into_iter()
            .map(|mut new_head| {
                new_head.patterns.extend_from_slice(&self.patterns[1..]);
                new_head
            })
            .collect();
        debug!("specialize({:#?}, {:#?}) = {:#?}", self, constructor, result);
        result
    }
}

impl<'p, 'tcx> Default for PatStack<'p, 'tcx> {
    fn default() -> Self {
        PatStack::empty()
    }
}

impl<'p, 'tcx> FromIterator<&'p Pat<'tcx>> for PatStack<'p, 'tcx> {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = &'p Pat<'tcx>>,
    {
        PatStack::from_vec(iter.into_iter().collect())
    }
}

/// A 2D matrix.
pub struct Matrix<'p, 'tcx>(Vec<PatStack<'p, 'tcx>>);

impl<'p, 'tcx> Matrix<'p, 'tcx> {
    pub fn empty() -> Self {
        Matrix(vec![])
    }

    pub fn push(&mut self, row: PatStack<'p, 'tcx>) {
        self.0.push(row)
    }

    /// Iterate over the first component of each row
    fn heads<'a>(&'a self) -> impl Iterator<Item = &'a Pat<'tcx>> + Captures<'p> {
        self.0.iter().map(|r| r.head())
    }

    fn head_ctors(&self, cx: &MatchCheckCtxt<'_, 'tcx>) -> Vec<Constructor<'tcx>> {
        self.0.iter().flat_map(|r| r.head_ctors(cx)).filter(|ctor| !ctor.is_wildcard()).collect()
    }

    /// This computes `S(constructor, self)`. See top of the file for explanations.
    fn specialize<'a, 'q>(
        &self,
        cx: &MatchCheckCtxt<'a, 'tcx>,
        constructor: &Constructor<'tcx>,
        ctor_wild_subpatterns: &[&'q Pat<'tcx>],
    ) -> Matrix<'q, 'tcx>
    where
        'a: 'q,
        'p: 'q,
    {
        Matrix(
            self.0
                .iter()
                .flat_map(|r| r.specialize(cx, constructor, ctor_wild_subpatterns))
                .collect(),
        )
    }
}

/// Pretty-printer for matrices of patterns, example:
/// +++++++++++++++++++++++++++++
/// + _     + []                +
/// +++++++++++++++++++++++++++++
/// + true  + [First]           +
/// +++++++++++++++++++++++++++++
/// + true  + [Second(true)]    +
/// +++++++++++++++++++++++++++++
/// + false + [_]               +
/// +++++++++++++++++++++++++++++
/// + _     + [_, _, tail @ ..] +
/// +++++++++++++++++++++++++++++
impl<'p, 'tcx> fmt::Debug for Matrix<'p, 'tcx> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "\n")?;

        let &Matrix(ref m) = self;
        let pretty_printed_matrix: Vec<Vec<String>> =
            m.iter().map(|row| row.iter().map(|pat| format!("{:?}", pat)).collect()).collect();

        let column_count = m.iter().map(|row| row.len()).max().unwrap_or(0);
        assert!(m.iter().all(|row| row.len() == column_count));
        let column_widths: Vec<usize> = (0..column_count)
            .map(|col| pretty_printed_matrix.iter().map(|row| row[col].len()).max().unwrap_or(0))
            .collect();

        let total_width = column_widths.iter().cloned().sum::<usize>() + column_count * 3 + 1;
        let br = "+".repeat(total_width);
        write!(f, "{}\n", br)?;
        for row in pretty_printed_matrix {
            write!(f, "+")?;
            for (column, pat_str) in row.into_iter().enumerate() {
                write!(f, " ")?;
                write!(f, "{:1$}", pat_str, column_widths[column])?;
                write!(f, " +")?;
            }
            write!(f, "\n")?;
            write!(f, "{}\n", br)?;
        }
        Ok(())
    }
}

impl<'p, 'tcx> FromIterator<PatStack<'p, 'tcx>> for Matrix<'p, 'tcx> {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = PatStack<'p, 'tcx>>,
    {
        Matrix(iter.into_iter().collect())
    }
}

pub struct MatchCheckCtxt<'a, 'tcx> {
    pub tcx: TyCtxt<'tcx>,
    /// The module in which the match occurs. This is necessary for
    /// checking inhabited-ness of types because whether a type is (visibly)
    /// inhabited can depend on whether it was defined in the current module or
    /// not. E.g., `struct Foo { _private: ! }` cannot be seen to be empty
    /// outside its module and should not be matchable with an empty match
    /// statement.
    pub module: DefId,
    param_env: ty::ParamEnv<'tcx>,
    pub pattern_arena: &'a TypedArena<Pat<'tcx>>,
}

impl<'a, 'tcx> MatchCheckCtxt<'a, 'tcx> {
    pub fn create_and_enter<F, R>(
        tcx: TyCtxt<'tcx>,
        param_env: ty::ParamEnv<'tcx>,
        module: DefId,
        f: F,
    ) -> R
    where
        F: for<'b> FnOnce(MatchCheckCtxt<'b, 'tcx>) -> R,
    {
        let pattern_arena = TypedArena::default();

        f(MatchCheckCtxt { tcx, param_env, module, pattern_arena: &pattern_arena })
    }

    fn is_uninhabited(&self, ty: Ty<'tcx>) -> bool {
        if self.tcx.features().exhaustive_patterns {
            self.tcx.is_ty_uninhabited_from(self.module, ty)
        } else {
            false
        }
    }

    fn is_non_exhaustive_variant<'p>(&self, pattern: &'p Pat<'tcx>) -> bool {
        match *pattern.kind {
            PatKind::Variant { adt_def, variant_index, .. } => {
                let ref variant = adt_def.variants[variant_index];
                variant.is_field_list_non_exhaustive()
            }
            _ => false,
        }
    }

    fn is_non_exhaustive_enum(&self, ty: Ty<'tcx>) -> bool {
        match ty.kind {
            ty::Adt(adt_def, ..) => adt_def.is_variant_list_non_exhaustive(),
            _ => false,
        }
    }

    fn is_local(&self, ty: Ty<'tcx>) -> bool {
        match ty.kind {
            ty::Adt(adt_def, ..) => adt_def.did.is_local(),
            _ => false,
        }
    }
}

/// Constructors, including base constructors and meta-constructors.
#[derive(Clone, Debug, PartialEq)]
enum Constructor<'tcx> {
    // Base constructors
    /// The constructor of all patterns that don't vary by constructor,
    /// e.g., struct patterns and fixed-length arrays.
    Single,
    /// Enum variants.
    Variant(DefId),
    /// Literal values.
    ConstantValue(&'tcx ty::Const<'tcx>),
    /// Array patterns of length n.
    FixedLenSlice(u64),

    // Meta-constructors
    /// Ranges of integer literal values (`2..=5` and `2..5`).
    IntRange(IntRange<'tcx>),
    /// Ranges of non-integer literal values (`2.0..=5.2`).
    ConstantRange(&'tcx ty::Const<'tcx>, &'tcx ty::Const<'tcx>, RangeEnd),
    /// Slice patterns. Captures any array constructor of length >= i+j.
    VarLenSlice(u64, u64),
    /// Wildcard meta-constructor. Captures all possible constructors for a given type.
    Wildcard,
    /// Special wildcard-like constructor that carries only a subset of all possible constructors.
    /// It is used only when splitting `Constructor::Wildcard` and some constructors were not
    /// present in the matrix.
    /// The contained list must be nonempty.
    MissingConstructors(MissingConstructors<'tcx>),
}

impl<'tcx> Constructor<'tcx> {
    fn is_slice(&self) -> bool {
        match self {
            FixedLenSlice(..) | VarLenSlice(..) => true,
            _ => false,
        }
    }

    fn is_wildcard(&self) -> bool {
        match self {
            Wildcard => true,
            MissingConstructors(_) => bug!(
                "not sure if MissingConstructors should be a wildcard; shouldn't happen anyways."
            ),
            _ => false,
        }
    }

    fn variant_index_for_adt<'a>(
        &self,
        cx: &MatchCheckCtxt<'a, 'tcx>,
        adt: &'tcx ty::AdtDef,
    ) -> VariantIdx {
        match self {
            Variant(id) => adt.variant_index_with_id(*id),
            Single => {
                assert!(!adt.is_enum());
                VariantIdx::new(0)
            }
            ConstantValue(c) => crate::const_eval::const_variant_index(cx.tcx, cx.param_env, c),
            _ => bug!("bad constructor {:?} for adt {:?}", self, adt),
        }
    }

    /// Split a constructor into equivalence classes of constructors that behave the same
    /// for the given matrix. See description of the algorithm for details.
    /// Note: We can rely on this returning an empty list if the type is (visibly) uninhabited.
    fn split_meta_constructor(
        self,
        cx: &MatchCheckCtxt<'_, 'tcx>,
        ty: Ty<'tcx>,
        head_ctors: &Vec<Constructor<'tcx>>,
    ) -> SmallVec<[Constructor<'tcx>; 1]> {
        debug!("split_meta_constructor {:?}", self);
        assert!(!head_ctors.iter().any(|c| c.is_wildcard()));

        match self {
            // Any base constructor can be used unchanged.
            Single | Variant(_) | ConstantValue(_) | FixedLenSlice(_) => smallvec![self],
            IntRange(ctor_range) if IntRange::should_treat_range_exhaustively(cx.tcx, ty) => {
                // Splitting up a range naïvely would mean creating a separate constructor for
                // every single value in the range, which is clearly impractical. We therefore want
                // to keep together subranges for which the specialisation will be identical across
                // all values in that range. These classes are grouped by the patterns that apply
                // to them (in the matrix `M`). We can split the range whenever the patterns that
                // apply to that range (specifically: the patterns that *intersect* with that
                // range) change. Our solution, therefore, is to split the range constructor into
                // subranges at every single point where the group of intersecting patterns changes
                // (using the method described below). The nice thing about this is that the number
                // of subranges is linear in the number of rows in the matrix (i.e., the number of
                // cases in the `match` statement), so we don't need to be worried about matching
                // over a gargantuan number of ranges.
                //
                // Essentially, given the first column of a matrix representing ranges, that looks
                // like the following:
                //
                // |------|  |----------| |-------|    ||
                //    |-------| |-------|            |----| ||
                //       |---------|
                //
                // We split the ranges up into equivalence classes so the ranges are no longer
                // overlapping:
                //
                // |--|--|||-||||--||---|||-------|  |-|||| ||
                //
                // The logic for determining how to split the ranges is fairly straightforward: we
                // calculate boundaries for each interval range, sort them, then create
                // constructors for each new interval between every pair of boundary points. (This
                // essentially amounts to performing the intuitive merging operation depicted
                // above.)

                /// Represents a border between 2 integers. Because the intervals spanning borders
                /// must be able to cover every integer, we need to be able to represent
                /// 2^128 + 1 such borders.
                #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
                enum Border {
                    JustBefore(u128),
                    AfterMax,
                }

                // A function for extracting the borders of an integer interval.
                fn range_borders(r: IntRange<'_>) -> impl Iterator<Item = Border> {
                    let (lo, hi) = r.range.into_inner();
                    let from = Border::JustBefore(lo);
                    let to = match hi.checked_add(1) {
                        Some(m) => Border::JustBefore(m),
                        None => Border::AfterMax,
                    };
                    vec![from, to].into_iter()
                }

                // `borders` is the set of borders between equivalence classes: each equivalence
                // class lies between 2 borders.
                let row_borders = head_ctors
                    .iter()
                    .flat_map(IntRange::from_ctor)
                    .flat_map(|range| ctor_range.intersection(cx.tcx, &range))
                    .flat_map(|range| range_borders(range));
                let ctor_borders = range_borders(ctor_range.clone());
                let mut borders: Vec<_> = row_borders.chain(ctor_borders).collect();
                borders.sort_unstable();

                // We're going to iterate through every adjacent pair of borders, making sure that
                // each represents an interval of nonnegative length, and convert each such
                // interval into a constructor.
                borders
                    .windows(2)
                    .filter_map(|window| match (window[0], window[1]) {
                        (Border::JustBefore(n), Border::JustBefore(m)) => {
                            if n < m {
                                Some(n..=(m - 1))
                            } else {
                                None
                            }
                        }
                        (Border::JustBefore(n), Border::AfterMax) => Some(n..=u128::MAX),
                        (Border::AfterMax, _) => None,
                    })
                    .map(|range| IntRange::new(ty, range))
                    .map(IntRange)
                    .collect()
            }
            // When not treated exhaustively, don't split ranges.
            ConstantRange(..) | IntRange(..) => smallvec![self],
            VarLenSlice(self_prefix, self_suffix) => {
                // A variable-length slice pattern is matched by an infinite collection of
                // fixed-length array patterns. However it turns out that for each finite set of
                // patterns `P`, all sufficiently large array lengths are equivalent.
                //
                // Each slice `s` with a "sufficiently-large" length `l ≥ L` that applies
                // to exactly the subset `Pₜ` of `P` can be transformed to a slice
                // `sₘ` for each sufficiently-large length `m` that applies to exactly
                // the same subset of `P`.
                //
                // Because of that, each witness for reachability-checking from one of the
                // sufficiently-large lengths can be transformed to an equally-valid witness from
                // any other length, so we all slice lengths from the "minimal sufficiently-large
                // length" until infinity will behave the same.
                //
                // Note that the fact that there is a *single* `sₘ` for each `m`,
                // not depending on the specific pattern in `P`, is important: if
                // you look at the pair of patterns
                //     `[true, ..]`
                //     `[.., false]`
                // Then any slice of length ≥1 that matches one of these two
                // patterns can be trivially turned to a slice of any
                // other length ≥1 that matches them and vice-versa -
                // but the slice from length 2 `[false, true]` that matches neither
                // of these patterns can't be turned to a slice from length 1 that
                // matches neither of these patterns, so we have to consider
                // slices from length 2 there.
                //
                // Now, to see that that length exists and find it, observe that slice
                // patterns are either "fixed-length" patterns (`[_, _, _]`) or
                // "variable-length" patterns (`[_, .., _]`).
                //
                // For fixed-length patterns, all slices with lengths *longer* than
                // the pattern's length have the same outcome (of not matching), so
                // as long as `L` is greater than the pattern's length we can pick
                // any `sₘ` from that length and get the same result.
                //
                // For variable-length patterns, the situation is more complicated,
                // because as seen above the precise value of `sₘ` matters.
                //
                // However, for each variable-length pattern `p` with a prefix of length
                // `plₚ` and suffix of length `slₚ`, only the first `plₚ` and the last
                // `slₚ` elements are examined.
                //
                // Therefore, as long as `L` is positive (to avoid concerns about empty
                // types), all elements after the maximum prefix length and before
                // the maximum suffix length are not examined by any variable-length
                // pattern, and therefore can be added/removed without affecting
                // them - creating equivalent patterns from any sufficiently-large
                // length.
                //
                // Of course, if fixed-length patterns exist, we must be sure
                // that our length is large enough to miss them all, so
                // we can pick `L = max(max(FIXED_LEN)+1, max(PREFIX_LEN) + max(SUFFIX_LEN))`
                //
                // For example, with the above pair of patterns, all elements
                // but the first and last can be added/removed, so any
                // witness of length ≥2 (say, `[false, false, true]`) can be
                // turned to a witness from any other length ≥2.
                //
                // For diagnostics, we keep the prefix and suffix lengths separate, so in the case
                // where `max(FIXED_LEN)+1` is the largest, we adapt `max(PREFIX_LEN)` accordingly,
                // so that `max(PREFIX_LEN) + max(SUFFIX_LEN) = L`.

                let mut max_prefix_len = self_prefix;
                let mut max_suffix_len = self_suffix;
                let mut max_fixed_len = 0;

                for ctor in head_ctors {
                    match *ctor {
                        ConstantValue(value) => {
                            // Extract the length of an array/slice from a constant
                            match (value.val, &value.ty.kind) {
                                (_, ty::Array(_, n)) => {
                                    max_fixed_len =
                                        cmp::max(max_fixed_len, n.eval_usize(cx.tcx, cx.param_env))
                                }
                                (ConstValue::Slice { start, end, .. }, ty::Slice(_)) => {
                                    max_fixed_len = cmp::max(max_fixed_len, (end - start) as u64)
                                }
                                _ => {}
                            }
                        }
                        FixedLenSlice(len) => {
                            max_fixed_len = cmp::max(max_fixed_len, len);
                        }
                        VarLenSlice(prefix, suffix) => {
                            max_prefix_len = cmp::max(max_prefix_len, prefix);
                            max_suffix_len = cmp::max(max_suffix_len, suffix);
                        }
                        _ => {}
                    }
                }

                if max_fixed_len + 1 >= max_prefix_len + max_suffix_len {
                    max_prefix_len = cmp::max(max_prefix_len, max_fixed_len + 1 - max_suffix_len);
                }

                (self_prefix + self_suffix..max_prefix_len + max_suffix_len)
                    .map(FixedLenSlice)
                    .chain(Some(VarLenSlice(max_prefix_len, max_suffix_len)))
                    .collect()
            }
            Wildcard => {
                let is_declared_nonexhaustive = !cx.is_local(ty) && cx.is_non_exhaustive_enum(ty);

                // `all_ctors` is the list of all the constructors for the given type.
                let all_ctors = all_constructors(cx, ty);

                let is_privately_empty = all_ctors.is_empty() && !cx.is_uninhabited(ty);

                // For privately empty and non-exhaustive enums, we work as if there were an "extra"
                // `_` constructor for the type, so we can never match over all constructors.
                // See the `match_privately_empty` test for details.
                //
                // FIXME: currently the only way I know of something can
                // be a privately-empty enum is when the exhaustive_patterns
                // feature flag is not present, so this is only
                // needed for that case.
                let is_non_exhaustive = is_privately_empty
                    || is_declared_nonexhaustive
                    || (ty.is_ptr_sized_integral()
                        && !cx.tcx.features().precise_pointer_size_matching);

                // `missing_ctors` is the set of constructors from the same type as the
                // first column of `matrix` that are matched only by wildcard patterns
                // from the first column.
                //
                // Therefore, if there is some pattern that is unmatched by `matrix`,
                // it will still be unmatched if the first constructor is replaced by
                // any of the constructors in `missing_ctors`
                let missing_ctors =
                    MissingConstructors::new(cx.tcx, cx.param_env, all_ctors, head_ctors.clone());
                debug!(
                    "missing_ctors.is_empty()={:#?} is_non_exhaustive={:#?}",
                    missing_ctors.is_empty(),
                    is_non_exhaustive,
                );

                // If there are some missing constructors, we only need to specialize relative
                // to them and we can ignore the other ones. Otherwise, we have to try all
                // existing constructors one-by-one.
                if is_non_exhaustive {
                    // We pretend the type has an additional `_` constructor, that counts as a
                    // missing constructor. So we return that constructor.
                    smallvec![Wildcard]
                } else if !missing_ctors.is_empty() {
                    if head_ctors.is_empty() {
                        // If head_ctors is empty, then all constructors of the type behave the same
                        // so we can keep the Wildcard meta-constructor.
                        smallvec![Wildcard]
                    } else {
                        // Otherwise, we have a set of missing constructors that is neither empty
                        // not equal to all_constructors. Since all missing constructors will
                        // behave the same (i.e. will be matched only by wildcards), we return a
                        // meta-constructor that contains all of them at once.
                        smallvec![MissingConstructors(missing_ctors)]
                    }
                } else {
                    // Here we know there are no missing constructors, so we have to try all
                    // existing constructors one-by-one.
                    let (all_ctors, _) = missing_ctors.into_inner();
                    // Recursively split newly generated list of constructors. This list must not
                    // contain any wildcards so we don't recurse infinitely.
                    all_ctors
                        .into_iter()
                        .flat_map(|ctor| ctor.split_meta_constructor(cx, ty, head_ctors))
                        .collect()
                }
            }
            MissingConstructors(_) => bug!("shouldn't try to split constructor {:?}", self),
        }
    }

    /// Returns a collection of constructors that spans the constructors covered by `self`,
    /// subtracted by the constructors covered by `head_ctors`: i.e., `self \ head_ctors` (in set
    /// notation).
    fn subtract_meta_constructor(
        self,
        _tcx: TyCtxt<'tcx>,
        _param_env: ty::ParamEnv<'tcx>,
        used_ctors: &Vec<Constructor<'tcx>>,
    ) -> SmallVec<[Constructor<'tcx>; 1]> {
        debug!("subtract_meta_constructor {:?}", self);
        // The input must not contain a wildcard
        assert!(!used_ctors.iter().any(|c| c.is_wildcard()));

        match self {
            // Those constructors can't intersect with a non-wildcard meta-constructor, so we're
            // fine just comparing for equality.
            Single | Variant(_) | ConstantRange(..) | ConstantValue(..) => {
                if used_ctors.iter().any(|c| c == &self) { smallvec![] } else { smallvec![self] }
            }
            FixedLenSlice(self_len) => {
                let overlaps = |c: &Constructor<'_>| match c {
                    FixedLenSlice(other_len) => *other_len == self_len,
                    VarLenSlice(prefix, suffix) => prefix + suffix <= self_len,
                    _ => false,
                };
                if used_ctors.iter().any(overlaps) { smallvec![] } else { smallvec![self] }
            }
            VarLenSlice(self_prefix, self_suffix) => {
                // Assume we have the following match:
                // ```
                // match slice {
                //     [0] => {}
                //     [_, _, _] => {}
                //     [1, 2, 3, 4, 5, 6, ..] => {}
                //     [_, _, _, _, _, _, _] => {}
                //     [0, ..] => {}
                // }
                // ```
                // We want to know which constructors are matched by the last pattern, but are not
                // matched by the first four ones. Since we only speak of constructors here, we
                // only care about the length of the slices and not the particular subpatterns.
                // For that, we first notice that because of the third pattern, all constructors of
                // lengths 6 or more are covered already. `max_len` will be `Some(6)`.
                // Then we'll look at fixed-length constructors to see which are missing. The
                // returned list of constructors will be those of lengths in 1..6 that are not
                // present in the match. Lengths 1, 3 and 7 are matched already, so we get
                // `[FixedLenSlice(2), FixedLenSlice(4), FixedLenSlice(5)]`.
                // If we had removed the third pattern, we would have instead returned
                // `[FixedLenSlice(2), FixedLenSlice(4), FixedLenSlice(5), FixedLenSlice(6),
                // VarLenSlice(8, 0)]`.

                // Initially we cover all slice lengths starting from self_len.
                let self_len = self_prefix + self_suffix;

                // If there is a VarLenSlice(n) in used_ctors, then we have to discard
                // all lengths >= n. So we pick the smallest such n.
                let max_len: Option<_> = used_ctors
                    .iter()
                    .filter_map(|c: &Constructor<'tcx>| match c {
                        VarLenSlice(prefix, suffix) => Some(prefix + suffix),
                        _ => None,
                    })
                    .min();

                // The remaining range of lengths is now either `self_len..`
                // or `self_len..max_len`. We then remove from that range all the
                // individual FixedLenSlice lengths in used_ctors.

                // If max_len <= self_len there are no lengths remaining.
                if let Some(max_len) = max_len {
                    if max_len <= self_len {
                        return smallvec![];
                    }
                }

                // Extract fixed-size lengths
                let used_fixed_lengths: FxHashSet<u64> = used_ctors
                    .iter()
                    .filter_map(|c: &Constructor<'tcx>| match c {
                        FixedLenSlice(len) => Some(*len),
                        _ => None,
                    })
                    .collect();

                if let Some(max_len) = max_len {
                    (self_len..max_len)
                        .filter(|len| !used_fixed_lengths.contains(len))
                        .map(FixedLenSlice)
                        .collect()
                } else {
                    // Choose a length for which we know that all larger lengths remain in the
                    // output.
                    let min_free_length = used_fixed_lengths
                        .iter()
                        .map(|len| len + 1)
                        .chain(Some(self_len))
                        .max()
                        .unwrap();

                    // We know min_free_length >= self_len >= self_suffix so this can't underflow.
                    let final_varlen = VarLenSlice(min_free_length - self_suffix, self_suffix);

                    (self_len..min_free_length)
                        .filter(|len| !used_fixed_lengths.contains(len))
                        .map(FixedLenSlice)
                        .chain(Some(final_varlen))
                        .collect()
                }
            }
            IntRange(range) => {
                let used_ranges = used_ctors.iter().flat_map(IntRange::from_ctor);
                let mut remaining_ranges: SmallVec<[IntRange<'tcx>; 1]> = smallvec![range];

                // For each used ctor, subtract from the current set of constructors.
                for used_range in used_ranges {
                    remaining_ranges = remaining_ranges
                        .into_iter()
                        .flat_map(|range| used_range.subtract_from(range))
                        .collect();

                    // If the constructors that have been considered so far already cover
                    // the entire range of `self`, no need to look at more constructors.
                    if remaining_ranges.is_empty() {
                        break;
                    }
                }

                remaining_ranges.into_iter().map(IntRange).collect()
            }
            Wildcard | MissingConstructors(_) => {
                bug!("shouldn't try to subtract constructor {:?}", self)
            }
        }
    }

    /// This returns one wildcard pattern for each argument to this constructor.
    fn wildcard_subpatterns<'a>(
        &self,
        cx: &MatchCheckCtxt<'a, 'tcx>,
        ty: Ty<'tcx>,
    ) -> impl Iterator<Item = Pat<'tcx>> + DoubleEndedIterator {
        debug!("wildcard_subpatterns({:#?}, {:?})", self, ty);
        let subpattern_types = match *self {
            Single | Variant(_) => match ty.kind {
                ty::Tuple(ref fs) => fs.into_iter().map(|t| t.expect_ty()).collect(),
                ty::Ref(_, rty, _) => vec![rty],
                ty::Adt(adt, substs) => {
                    if adt.is_box() {
                        // Use T as the sub pattern type of Box<T>.
                        vec![substs.type_at(0)]
                    } else {
                        adt.variants[self.variant_index_for_adt(cx, adt)]
                            .fields
                            .iter()
                            .map(|field| {
                                let is_visible = adt.is_enum()
                                    || field.vis.is_accessible_from(cx.module, cx.tcx);
                                if is_visible {
                                    let ty = field.ty(cx.tcx, substs);
                                    match ty.kind {
                                        // If the field type returned is an array of an unknown
                                        // size return an TyErr.
                                        ty::Array(_, len)
                                            if len
                                                .try_eval_usize(cx.tcx, cx.param_env)
                                                .is_none() =>
                                        {
                                            cx.tcx.types.err
                                        }
                                        _ => ty,
                                    }
                                } else {
                                    // Treat all non-visible fields as TyErr. They
                                    // can't appear in any other pattern from
                                    // this match (because they are private),
                                    // so their type does not matter - but
                                    // we don't want to know they are
                                    // uninhabited.
                                    cx.tcx.types.err
                                }
                            })
                            .collect()
                    }
                }
                ty::Slice(ty) | ty::Array(ty, _) => bug!("bad slice pattern {:?} {:?}", self, ty),
                _ => vec![],
            },
            FixedLenSlice(length) => match ty.kind {
                ty::Slice(ty) | ty::Array(ty, _) => (0..length).map(|_| ty).collect(),
                _ => bug!("bad slice pattern {:?} {:?}", self, ty),
            },
            VarLenSlice(prefix, suffix) => match ty.kind {
                ty::Slice(ty) | ty::Array(ty, _) => (0..prefix + suffix).map(|_| ty).collect(),
                _ => bug!("bad slice pattern {:?} {:?}", self, ty),
            },
            ConstantValue(_)
            | MissingConstructors(_)
            | ConstantRange(..)
            | IntRange(..)
            | Wildcard => vec![],
        };

        subpattern_types.into_iter().map(|ty| Pat { ty, span: DUMMY_SP, kind: box PatKind::Wild })
    }

    /// This computes the arity of a constructor. The arity of a constructor
    /// is the number of its arguments.
    ///
    /// For instance, a tuple pattern `(_, 42, Some([]))` has arity 3, a struct pattern's arity is
    /// the number of fields it contains, etc.
    fn arity<'a>(&self, cx: &MatchCheckCtxt<'a, 'tcx>, ty: Ty<'tcx>) -> u64 {
        debug!("Constructor::arity({:#?}, {:?})", self, ty);
        match *self {
            Single | Variant(_) => match ty.kind {
                ty::Tuple(ref fs) => fs.len() as u64,
                ty::Ref(..) => 1,
                ty::Adt(adt, _) => {
                    adt.variants[self.variant_index_for_adt(cx, adt)].fields.len() as u64
                }
                ty::Slice(..) | ty::Array(..) => bug!("bad slice pattern {:?} {:?}", self, ty),
                _ => 0,
            },
            FixedLenSlice(length) => length,
            VarLenSlice(prefix, suffix) => prefix + suffix,
            ConstantValue(_)
            | ConstantRange(..)
            | IntRange(..)
            | Wildcard
            | MissingConstructors(_) => 0,
        }
    }

    /// Apply a constructor to a list of patterns, yielding a new pattern. `pats`
    /// must have as many elements as this constructor's arity.
    ///
    /// Examples:
    /// `self`: `Constructor::Single`
    /// `ty`: `(u32, u32, u32)`
    /// `pats`: `[10, 20, _]`
    /// returns `(10, 20, _)`
    ///
    /// `self`: `Constructor::Variant(Option::Some)`
    /// `ty`: `Option<bool>`
    /// `pats`: `[false]`
    /// returns `Some(false)`
    fn apply<'a>(
        &self,
        cx: &MatchCheckCtxt<'a, 'tcx>,
        ty: Ty<'tcx>,
        pats: impl IntoIterator<Item = Pat<'tcx>>,
    ) -> SmallVec<[Pat<'tcx>; 1]> {
        let mut pats = pats.into_iter();
        let pat = match *self {
            Single | Variant(_) => match ty.kind {
                ty::Adt(..) | ty::Tuple(..) => {
                    let subpatterns = pats
                        .enumerate()
                        .map(|(i, p)| FieldPat { field: Field::new(i), pattern: p })
                        .collect();

                    match ty.kind {
                        ty::Adt(adt_def, substs) if adt_def.is_enum() => PatKind::Variant {
                            adt_def,
                            substs,
                            variant_index: self.variant_index_for_adt(cx, adt_def),
                            subpatterns,
                        },
                        _ => PatKind::Leaf { subpatterns },
                    }
                }
                ty::Ref(..) => PatKind::Deref { subpattern: pats.nth(0).unwrap() },
                _ => PatKind::Wild,
            },
            FixedLenSlice(_) => {
                PatKind::Slice { prefix: pats.collect(), slice: None, suffix: vec![] }
            }
            VarLenSlice(prefix_len, _suffix_len) => match ty.kind {
                ty::Slice(ty) | ty::Array(ty, _) => {
                    let prefix = pats.by_ref().take(prefix_len as usize).collect();
                    let suffix = pats.collect();
                    let wild = Pat { ty, span: DUMMY_SP, kind: Box::new(PatKind::Wild) };
                    PatKind::Slice { prefix, slice: Some(wild), suffix }
                }
                _ => bug!("bad slice pattern {:?} {:?}", self, ty),
            },
            ConstantValue(value) => PatKind::Constant { value },
            ConstantRange(lo, hi, end) => PatKind::Range(PatRange { lo, hi, end }),
            IntRange(ref range) => range.to_patkind(cx.tcx),
            Wildcard => PatKind::Wild,
            MissingConstructors(ref missing_ctors) => {
                // Construct for each missing constructor a "wildcard" version of this
                // constructor, that matches everything that can be built with
                // it. For example, if `ctor` is a `Constructor::Variant` for
                // `Option::Some`, we get the pattern `Some(_)`.
                return missing_ctors
                    .iter()
                    .flat_map(|ctor| ctor.apply_wildcards(cx, ty))
                    .collect();
            }
        };

        smallvec![Pat { ty, span: DUMMY_SP, kind: Box::new(pat) }]
    }

    /// Like `apply`, but where all the subpatterns are wildcards `_`.
    fn apply_wildcards<'a>(
        &self,
        cx: &MatchCheckCtxt<'a, 'tcx>,
        ty: Ty<'tcx>,
    ) -> SmallVec<[Pat<'tcx>; 1]> {
        let pats = self.wildcard_subpatterns(cx, ty).rev();
        self.apply(cx, ty, pats)
    }
}

#[derive(Clone, Debug)]
pub enum Usefulness<'tcx> {
    Useful,
    UsefulWithWitness(Vec<Witness<'tcx>>),
    NotUseful,
}

impl<'tcx> Usefulness<'tcx> {
    fn new_useful(preference: WitnessPreference) -> Self {
        match preference {
            ConstructWitness => UsefulWithWitness(vec![Witness(vec![])]),
            LeaveOutWitness => Useful,
        }
    }

    fn is_useful(&self) -> bool {
        match *self {
            NotUseful => false,
            _ => true,
        }
    }

    fn apply_constructor(
        self,
        cx: &MatchCheckCtxt<'_, 'tcx>,
        ctor: &Constructor<'tcx>,
        ty: Ty<'tcx>,
    ) -> Self {
        match self {
            UsefulWithWitness(witnesses) => UsefulWithWitness(
                witnesses
                    .into_iter()
                    .flat_map(|witness| witness.apply_constructor(cx, &ctor, ty))
                    .collect(),
            ),
            x => x,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum WitnessPreference {
    ConstructWitness,
    LeaveOutWitness,
}

/// A witness of non-exhaustiveness for error reporting, represented
/// as a list of patterns (in reverse order of construction) with
/// wildcards inside to represent elements that can take any inhabitant
/// of the type as a value.
///
/// A witness against a list of patterns should have the same types
/// and length as the pattern matched against. Because Rust `match`
/// is always against a single pattern, at the end the witness will
/// have length 1, but in the middle of the algorithm, it can contain
/// multiple patterns.
///
/// For example, if we are constructing a witness for the match against
/// ```
/// struct Pair(Option<(u32, u32)>, bool);
///
/// match (p: Pair) {
///    Pair(None, _) => {}
///    Pair(_, false) => {}
/// }
/// ```
///
/// We'll perform the following steps:
/// 1. Start with an empty witness
///     `Witness(vec![])`
/// 2. Push a witness `Some(_)` against the `None`
///     `Witness(vec![Some(_)])`
/// 3. Push a witness `true` against the `false`
///     `Witness(vec![Some(_), true])`
/// 4. Apply the `Pair` constructor to the witnesses
///     `Witness(vec![Pair(Some(_), true)])`
///
/// The final `Pair(Some(_), true)` is then the resulting witness.
#[derive(Clone, Debug)]
pub struct Witness<'tcx>(Vec<Pat<'tcx>>);

impl<'tcx> Witness<'tcx> {
    pub fn single_pattern(self) -> Pat<'tcx> {
        assert_eq!(self.0.len(), 1);
        self.0.into_iter().next().unwrap()
    }

    /// Constructs a partial witness for a pattern given a list of
    /// patterns expanded by the specialization step.
    ///
    /// When a pattern P is discovered to be useful, this function is used bottom-up
    /// to reconstruct a complete witness, e.g., a pattern P' that covers a subset
    /// of values, V, where each value in that set is not covered by any previously
    /// used patterns and is covered by the pattern P'. Examples:
    ///
    /// ty: tuple of 3 elements
    /// pats: [10, 20, _]           => (10, 20, _)
    ///
    /// ty: struct X { a: (bool, &'static str), b: usize}
    /// pats: [(false, "foo"), 42]  => X { a: (false, "foo"), b: 42 }
    fn apply_constructor<'a>(
        mut self,
        cx: &MatchCheckCtxt<'a, 'tcx>,
        ctor: &Constructor<'tcx>,
        ty: Ty<'tcx>,
    ) -> SmallVec<[Self; 1]> {
        let arity = ctor.arity(cx, ty);
        let applied_pats = {
            let len = self.0.len() as u64;
            let pats = self.0.drain((len - arity) as usize..).rev();
            ctor.apply(cx, ty, pats)
        };

        applied_pats
            .into_iter()
            .map(|pat| {
                let mut w = self.clone();
                w.0.push(pat);
                w
            })
            .collect()
    }
}

/// This determines the set of all possible constructors of a pattern matching
/// values of type `ty`. We possibly return meta-constructors like integer ranges
/// that capture several base constructors at once.
///
/// We make sure to omit constructors that are statically impossible. E.g., for
/// `Option<!>`, we do not include `Some(_)` in the returned list of constructors.
fn all_constructors<'a, 'tcx>(
    cx: &MatchCheckCtxt<'a, 'tcx>,
    ty: Ty<'tcx>,
) -> Vec<Constructor<'tcx>> {
    debug!("all_constructors({:?})", ty);
    let ctors = match ty.kind {
        ty::Bool => {
            [true, false].iter().map(|&b| ConstantValue(ty::Const::from_bool(cx.tcx, b))).collect()
        }
        ty::Array(ref sub_ty, len) if len.try_eval_usize(cx.tcx, cx.param_env).is_some() => {
            let len = len.eval_usize(cx.tcx, cx.param_env);
            if len != 0 && cx.is_uninhabited(sub_ty) { vec![] } else { vec![FixedLenSlice(len)] }
        }
        // Treat arrays of a constant but unknown length like slices.
        ty::Array(ref sub_ty, _) | ty::Slice(ref sub_ty) => {
            if cx.is_uninhabited(sub_ty) {
                vec![FixedLenSlice(0)]
            } else {
                vec![VarLenSlice(0, 0)]
            }
        }
        ty::Adt(def, substs) if def.is_enum() => def
            .variants
            .iter()
            .filter(|v| {
                !cx.tcx.features().exhaustive_patterns
                    || !v
                        .uninhabited_from(cx.tcx, substs, def.adt_kind())
                        .contains(cx.tcx, cx.module)
            })
            .map(|v| Variant(v.def_id))
            .collect(),
        ty::Char => {
            let to_const = |x| x;
            vec![
                // The valid Unicode Scalar Value ranges.
                IntRange(
                    IntRange::from_range(
                        cx.tcx,
                        cx.tcx.types.char,
                        to_const('\u{0000}' as u128),
                        to_const('\u{D7FF}' as u128),
                        RangeEnd::Included,
                    )
                    .unwrap(),
                ),
                IntRange(
                    IntRange::from_range(
                        cx.tcx,
                        cx.tcx.types.char,
                        to_const('\u{E000}' as u128),
                        to_const('\u{10FFFF}' as u128),
                        RangeEnd::Included,
                    )
                    .unwrap(),
                ),
            ]
        }
        ty::Int(ity) => {
            let to_const = |x| x;
            let bits = Integer::from_attr(&cx.tcx, SignedInt(ity)).size().bits() as u128;
            let min = 1u128 << (bits - 1);
            let max = min - 1;
            vec![IntRange(
                IntRange::from_range(cx.tcx, ty, to_const(min), to_const(max), RangeEnd::Included)
                    .unwrap(),
            )]
        }
        ty::Uint(uty) => {
            let to_const = |x| x;
            let size = Integer::from_attr(&cx.tcx, UnsignedInt(uty)).size();
            let max = truncate(u128::max_value(), size);
            vec![IntRange(
                IntRange::from_range(cx.tcx, ty, to_const(0), to_const(max), RangeEnd::Included)
                    .unwrap(),
            )]
        }
        _ => {
            if cx.is_uninhabited(ty) {
                vec![]
            } else {
                vec![Single]
            }
        }
    };
    ctors
}

/// An inclusive interval, used for precise integer exhaustiveness checking.
/// `IntRange`s always store a contiguous range. This means that values are
/// encoded such that `0` encodes the minimum value for the integer,
/// regardless of the signedness.
/// For example, the pattern `-128..=127i8` is encoded as `0..=255`.
/// This makes comparisons and arithmetic on interval endpoints much more
/// straightforward. See `signed_bias` for details.
///
/// `IntRange` is never used to encode an empty range or a "range" that wraps
/// around the (offset) space: i.e., `range.lo <= range.hi`.
#[derive(Debug, Clone, PartialEq)]
struct IntRange<'tcx> {
    pub range: RangeInclusive<u128>,
    pub ty: Ty<'tcx>,
}

impl<'tcx> IntRange<'tcx> {
    fn new(ty: Ty<'tcx>, range: RangeInclusive<u128>) -> Self {
        IntRange { ty, range }
    }

    #[inline]
    fn is_integral(ty: Ty<'_>) -> bool {
        match ty.kind {
            ty::Char | ty::Int(_) | ty::Uint(_) => true,
            _ => false,
        }
    }

    fn should_treat_range_exhaustively(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
        // Don't treat `usize`/`isize` exhaustively unless the `precise_pointer_size_matching`
        // feature is enabled.
        IntRange::is_integral(ty)
            && (!ty.is_ptr_sized_integral() || tcx.features().precise_pointer_size_matching)
    }

    #[inline]
    fn integral_size_and_signed_bias(tcx: TyCtxt<'tcx>, ty: Ty<'_>) -> Option<(Size, u128)> {
        match ty.kind {
            ty::Char => Some((Size::from_bytes(4), 0)),
            ty::Int(ity) => {
                let size = Integer::from_attr(&tcx, SignedInt(ity)).size();
                Some((size, 1u128 << (size.bits() as u128 - 1)))
            }
            ty::Uint(uty) => Some((Integer::from_attr(&tcx, UnsignedInt(uty)).size(), 0)),
            _ => None,
        }
    }

    #[inline]
    fn from_const(
        tcx: TyCtxt<'tcx>,
        param_env: ty::ParamEnv<'tcx>,
        value: &Const<'tcx>,
    ) -> Option<IntRange<'tcx>> {
        if let Some((target_size, bias)) = Self::integral_size_and_signed_bias(tcx, value.ty) {
            let ty = value.ty;
            let val = if let ConstValue::Scalar(Scalar::Raw { data, size }) = value.val {
                // For this specific pattern we can skip a lot of effort and go
                // straight to the result, after doing a bit of checking. (We
                // could remove this branch and just use the next branch, which
                // is more general but much slower.)
                Scalar::<()>::check_raw(data, size, target_size);
                data
            } else if let Some(val) = value.try_eval_bits(tcx, param_env, ty) {
                // This is a more general form of the previous branch.
                val
            } else {
                return None;
            };
            let val = val ^ bias;
            Some(IntRange { range: val..=val, ty })
        } else {
            None
        }
    }

    #[inline]
    fn from_const_range(
        tcx: TyCtxt<'tcx>,
        param_env: ty::ParamEnv<'tcx>,
        lo: &Const<'tcx>,
        hi: &Const<'tcx>,
        end: &RangeEnd,
    ) -> Option<IntRange<'tcx>> {
        let ty = lo.ty;
        let lo = lo.eval_bits(tcx, param_env, lo.ty);
        let hi = hi.eval_bits(tcx, param_env, hi.ty);
        Self::from_range(tcx, ty, lo, hi, *end)
    }

    #[inline]
    fn from_range(
        tcx: TyCtxt<'tcx>,
        ty: Ty<'tcx>,
        lo: u128,
        hi: u128,
        end: RangeEnd,
    ) -> Option<IntRange<'tcx>> {
        if Self::is_integral(ty) {
            // Perform a shift if the underlying types are signed,
            // which makes the interval arithmetic simpler.
            let bias = IntRange::signed_bias(tcx, ty);
            let (lo, hi) = (lo ^ bias, hi ^ bias);
            // Make sure the interval is well-formed.
            if lo > hi || lo == hi && end == RangeEnd::Excluded {
                None
            } else {
                let offset = (end == RangeEnd::Excluded) as u128;
                Some(IntRange { range: lo..=(hi - offset), ty })
            }
        } else {
            None
        }
    }

    fn from_ctor(ctor: &Constructor<'tcx>) -> Option<IntRange<'tcx>> {
        match ctor {
            IntRange(range) => Some(range.clone()),
            _ => None,
        }
    }

    // The return value of `signed_bias` should be XORed with an endpoint to encode/decode it.
    fn signed_bias(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> u128 {
        match ty.kind {
            ty::Int(ity) => {
                let bits = Integer::from_attr(&tcx, SignedInt(ity)).size().bits() as u128;
                1u128 << (bits - 1)
            }
            _ => 0,
        }
    }

    /// Converts an `IntRange` to a `PatKind::Constant` or inclusive `PatKind::Range`.
    fn to_patkind(&self, tcx: TyCtxt<'tcx>) -> PatKind<'tcx> {
        let bias = IntRange::signed_bias(tcx, self.ty);
        let (lo, hi) = self.range.clone().into_inner();
        if lo == hi {
            let ty = ty::ParamEnv::empty().and(self.ty);
            PatKind::Constant { value: ty::Const::from_bits(tcx, lo ^ bias, ty) }
        } else {
            let param_env = ty::ParamEnv::empty().and(self.ty);
            let to_const = |x| ty::Const::from_bits(tcx, x, param_env);
            PatKind::Range(PatRange {
                lo: to_const(lo ^ bias),
                hi: to_const(hi ^ bias),
                end: RangeEnd::Included,
            })
        }
    }

    /// Returns a collection of ranges that spans the values covered by `ctor`, subtracted
    /// by the values covered by `self`: i.e., `ctor \ self` (in set notation).
    fn subtract_from(&self, other: Self) -> SmallVec<[Self; 2]> {
        let range = other.range;

        let ty = self.ty;
        let (lo, hi) = (*self.range.start(), *self.range.end());
        let (range_lo, range_hi) = range.into_inner();
        let mut remaining_ranges = smallvec![];
        if lo > range_hi || range_lo > hi {
            // The pattern doesn't intersect with the range at all,
            // so the range remains untouched.
            remaining_ranges.push(Self::new(ty, range_lo..=range_hi));
        } else {
            if lo > range_lo {
                // The pattern intersects an upper section of the
                // range, so a lower section will remain.
                remaining_ranges.push(Self::new(ty, range_lo..=(lo - 1)));
            }
            if hi < range_hi {
                // The pattern intersects a lower section of the
                // range, so an upper section will remain.
                remaining_ranges.push(Self::new(ty, (hi + 1)..=range_hi));
            }
        }
        remaining_ranges
    }

    fn intersection(&self, tcx: TyCtxt<'tcx>, other: &Self) -> Option<Self> {
        let ty = self.ty;
        let (lo, hi) = (*self.range.start(), *self.range.end());
        let (other_lo, other_hi) = (*other.range.start(), *other.range.end());
        if Self::should_treat_range_exhaustively(tcx, ty) {
            if lo <= other_hi && other_lo <= hi {
                Some(IntRange { range: max(lo, other_lo)..=min(hi, other_hi), ty })
            } else {
                None
            }
        } else {
            // If the range sould not be treated exhaustively, fallback to checking for inclusion.
            if other_lo <= lo && hi <= other_hi { Some(self.clone()) } else { None }
        }
    }
}

// A struct to compute a set of constructors equivalent to `all_ctors \ used_ctors`.
#[derive(Clone)]
struct MissingConstructors<'tcx> {
    param_env: ty::ParamEnv<'tcx>,
    tcx: TyCtxt<'tcx>,
    all_ctors: Vec<Constructor<'tcx>>,
    used_ctors: Vec<Constructor<'tcx>>,
}

impl<'tcx> MissingConstructors<'tcx> {
    fn new(
        tcx: TyCtxt<'tcx>,
        param_env: ty::ParamEnv<'tcx>,
        all_ctors: Vec<Constructor<'tcx>>,
        used_ctors: Vec<Constructor<'tcx>>,
    ) -> Self {
        MissingConstructors { tcx, param_env, all_ctors, used_ctors }
    }

    fn into_inner(self) -> (Vec<Constructor<'tcx>>, Vec<Constructor<'tcx>>) {
        (self.all_ctors, self.used_ctors)
    }

    fn is_empty(&self) -> bool {
        self.iter().next().is_none()
    }

    /// Iterate over all_ctors \ used_ctors
    fn iter<'a>(&'a self) -> impl Iterator<Item = Constructor<'tcx>> + Captures<'a> {
        self.all_ctors.iter().flat_map(move |req_ctor| {
            req_ctor.clone().subtract_meta_constructor(self.tcx, self.param_env, &self.used_ctors)
        })
    }
}

impl<'tcx> fmt::Debug for MissingConstructors<'tcx> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ctors: Vec<_> = self.iter().collect();
        f.debug_tuple("MissingConstructors").field(&ctors).finish()
    }
}

/// This is needed for the `PartialEq` impl of `Constructor`.
/// Comparing a `Constructor::MissingConstructor` with something else
/// should however never happen, so this implementaiton panics.
impl<'tcx> PartialEq<Self> for MissingConstructors<'tcx> {
    fn eq(&self, _other: &Self) -> bool {
        bug!("tried to compare MissingConstructors for equality")
    }
}

/// Main entrypoint of the algorithm described at th top of the file.
/// Note that to correctly handle empty types:
///   (0) We don't exit early if the pattern matrix has zero rows. We just
///       continue to recurse over columns.
///   (1) all_constructors will only return constructors that are statically
///       possible. E.g., it will only return `Ok` for `Result<T, !>`.
///
/// This finds whether a pattern-stack `v` is 'useful' in relation to a set of such pattern-stacks
/// (aka 'matrix') `m` - this is defined as there being a set of inputs that will match `v` but not
/// any of the rows in `m`.
///
/// All the patterns at each column of the `matrix ++ v` matrix must
/// have the same type, except that wildcard (PatKind::Wild) patterns
/// with type `TyErr` are also allowed, even if the "type of the column"
/// is not `TyErr`. That is used to represent private fields, as using their
/// real type might leak that they are inhabited.
///
/// This is used both for reachability checking (if a pattern isn't useful in
/// relation to preceding patterns, it is not reachable) and exhaustiveness
/// checking (if a wildcard pattern is useful in relation to a matrix, the
/// matrix isn't exhaustive).
pub fn is_useful<'p, 'a, 'tcx>(
    cx: &MatchCheckCtxt<'a, 'tcx>,
    matrix: &Matrix<'p, 'tcx>,
    v: &PatStack<'_, 'tcx>,
    witness_preference: WitnessPreference,
) -> Usefulness<'tcx> {
    let &Matrix(ref rows) = matrix;
    debug!("is_useful({:#?}, {:#?})", matrix, v);

    // The base case. We are pattern-matching on () and the return value is
    // based on whether our matrix has a row or not.
    // NOTE: This could potentially be optimized by checking rows.is_empty()
    // first and then, if v is non-empty, the return value is based on whether
    // the type of the tuple we're checking is inhabited or not.
    if v.is_empty() {
        return if rows.is_empty() {
            Usefulness::new_useful(witness_preference)
        } else {
            NotUseful
        };
    };

    assert!(rows.iter().all(|r| r.len() == v.len()));

    // TyErr is used to represent the type of wildcard patterns matching
    // against inaccessible (private) fields of structs, so that we won't
    // be able to observe whether the types of the struct's fields are
    // inhabited.
    //
    // If the field is truly inaccessible, then all the patterns
    // matching against it must be wildcard patterns, so its type
    // does not matter.
    //
    // However, if we are matching against non-wildcard patterns, we
    // need to know the real type of the field so we can specialize
    // against it. This primarily occurs through constants - they
    // can include contents for fields that are inaccessible at the
    // location of the match. In that case, the field's type is
    // inhabited - by the constant - so we can just use it.
    //
    // FIXME: this might lead to "unstable" behavior with macro hygiene
    // introducing uninhabited patterns for inaccessible fields. We
    // need to figure out how to model that.
    let ty = matrix.heads().map(|p| p.ty).find(|ty| !ty.references_error()).unwrap_or(v.head().ty);

    debug!("is_useful_expand_first_col: ty={:#?}, expanding {:#?}", ty, v.head());

    let v_constructors = v.head_ctors(cx);

    if cx.is_non_exhaustive_variant(v.head())
        && !cx.is_local(ty)
        && !v_constructors.iter().any(|ctor| ctor.is_wildcard())
    {
        debug!("is_useful - shortcut because declared non-exhaustive");
        // FIXME(#65157)
        return Useful;
    }

    let matrix_head_ctors = matrix.head_ctors(cx);
    debug!("matrix_head_ctors = {:#?}", matrix_head_ctors);

    v_constructors
        .into_iter()
        .flat_map(|ctor| ctor.split_meta_constructor(cx, ty, &matrix_head_ctors))
        .map(|c| is_useful_specialized(cx, matrix, v, c, ty, witness_preference))
        .find(|result| result.is_useful())
        .unwrap_or(NotUseful)
}

/// A shorthand for the `U(S(c, M), S(c, q))` operation. I.e., `is_useful` applied
/// to the specialised version of both the pattern matrix `M` and the new pattern `q`.
fn is_useful_specialized<'p, 'a, 'tcx>(
    cx: &MatchCheckCtxt<'a, 'tcx>,
    matrix: &Matrix<'p, 'tcx>,
    v: &PatStack<'_, 'tcx>,
    ctor: Constructor<'tcx>,
    ty: Ty<'tcx>,
    witness_preference: WitnessPreference,
) -> Usefulness<'tcx> {
    debug!("is_useful_specialized({:#?}, {:#?}, {:?})", v, ctor, ty);

    let ctor_wild_subpatterns_owned: Vec<_> = ctor.wildcard_subpatterns(cx, ty).collect();
    let ctor_wild_subpatterns: Vec<_> = ctor_wild_subpatterns_owned.iter().collect();
    let matrix = matrix.specialize(cx, &ctor, &ctor_wild_subpatterns);
    let ret = v
        .specialize(cx, &ctor, &ctor_wild_subpatterns)
        .into_iter()
        .map(|v| is_useful(cx, &matrix, &v, witness_preference))
        .map(|u| u.apply_constructor(cx, &ctor, ty))
        .find(|result| result.is_useful())
        .unwrap_or(NotUseful);
    ret
}

/// Determines the constructors that are covered by the given pattern.
/// Except for or-patterns, this returns only one constructor (possibly a meta-constructor).
fn pat_constructors<'tcx>(
    tcx: TyCtxt<'tcx>,
    param_env: ty::ParamEnv<'tcx>,
    pat: &Pat<'tcx>,
) -> SmallVec<[Constructor<'tcx>; 1]> {
    match *pat.kind {
        PatKind::AscribeUserType { ref subpattern, .. } => {
            pat_constructors(tcx, param_env, subpattern)
        }
        PatKind::Binding { .. } | PatKind::Wild => smallvec![Wildcard],
        PatKind::Leaf { .. } | PatKind::Deref { .. } => smallvec![Single],
        PatKind::Variant { adt_def, variant_index, .. } => {
            smallvec![Variant(adt_def.variants[variant_index].def_id)]
        }
        PatKind::Constant { value } => {
            if let Some(range) = IntRange::from_const(tcx, param_env, value) {
                smallvec![IntRange(range)]
            } else {
                smallvec![ConstantValue(value)]
            }
        }
        PatKind::Range(PatRange { lo, hi, end }) => {
            if let Some(range) = IntRange::from_const_range(tcx, param_env, &lo, &hi, &end) {
                smallvec![IntRange(range)]
            } else {
                smallvec![ConstantRange(lo, hi, end)]
            }
        }
        PatKind::Array { .. } => match pat.ty.kind {
            ty::Array(_, length) => smallvec![FixedLenSlice(length.eval_usize(tcx, param_env))],
            _ => span_bug!(pat.span, "bad ty {:?} for array pattern", pat.ty),
        },
        PatKind::Slice { ref prefix, ref slice, ref suffix } => {
            let prefix = prefix.len() as u64;
            let suffix = suffix.len() as u64;
            if slice.is_some() {
                smallvec![VarLenSlice(prefix, suffix)]
            } else {
                smallvec![FixedLenSlice(prefix + suffix)]
            }
        }
        PatKind::Or { .. } => {
            bug!("support for or-patterns has not been fully implemented yet.");
        }
    }
}

// Checks whether a constant is equal to a user-written slice pattern. Only supports byte slices,
// meaning all other types will compare unequal and thus equal patterns often do not cause the
// second pattern to lint about unreachable match arms.
fn slice_pat_covered_by_const<'tcx>(
    tcx: TyCtxt<'tcx>,
    _span: Span,
    const_val: &'tcx ty::Const<'tcx>,
    prefix: &[Pat<'tcx>],
    slice: &Option<Pat<'tcx>>,
    suffix: &[Pat<'tcx>],
    param_env: ty::ParamEnv<'tcx>,
) -> Result<bool, ErrorReported> {
    let data: &[u8] = match (const_val.val, &const_val.ty.kind) {
        (ConstValue::ByRef { offset, alloc, .. }, ty::Array(t, n)) => {
            assert_eq!(*t, tcx.types.u8);
            let n = n.eval_usize(tcx, param_env);
            let ptr = Pointer::new(AllocId(0), offset);
            alloc.get_bytes(&tcx, ptr, Size::from_bytes(n)).unwrap()
        }
        (ConstValue::Slice { data, start, end }, ty::Slice(t)) => {
            assert_eq!(*t, tcx.types.u8);
            let ptr = Pointer::new(AllocId(0), Size::from_bytes(start as u64));
            data.get_bytes(&tcx, ptr, Size::from_bytes((end - start) as u64)).unwrap()
        }
        // FIXME(oli-obk): create a way to extract fat pointers from ByRef
        (_, ty::Slice(_)) => return Ok(false),
        _ => bug!(
            "slice_pat_covered_by_const: {:#?}, {:#?}, {:#?}, {:#?}",
            const_val,
            prefix,
            slice,
            suffix,
        ),
    };

    let pat_len = prefix.len() + suffix.len();
    if data.len() < pat_len || (slice.is_none() && data.len() > pat_len) {
        return Ok(false);
    }

    for (ch, pat) in data[..prefix.len()]
        .iter()
        .zip(prefix)
        .chain(data[data.len() - suffix.len()..].iter().zip(suffix))
    {
        match pat.kind {
            box PatKind::Constant { value } => {
                let b = value.eval_bits(tcx, param_env, pat.ty);
                assert_eq!(b as u8 as u128, b);
                if b as u8 != *ch {
                    return Ok(false);
                }
            }
            _ => {}
        }
    }

    Ok(true)
}

/// Checks whether there exists any shared value in either `ctor` or `pat` by intersecting them.
// This has a single call site that can be hot
#[inline(always)]
fn constructor_intersects_pattern<'p, 'tcx>(
    cx: &MatchCheckCtxt<'_, 'tcx>,
    ctor: &Constructor<'tcx>,
    pat: &'p Pat<'tcx>,
) -> Option<PatStack<'p, 'tcx>> {
    trace!("constructor_intersects_pattern {:#?}, {:#?}", ctor, pat);
    match ctor {
        Single => Some(PatStack::default()),
        IntRange(ctor) => {
            let pat = match *pat.kind {
                PatKind::Constant { value } => IntRange::from_const(cx.tcx, cx.param_env, value)?,
                PatKind::Range(PatRange { lo, hi, end }) => {
                    IntRange::from_const_range(cx.tcx, cx.param_env, lo, hi, &end)?
                }
                _ => bug!("`constructor_intersects_pattern` called with {:?}", pat),
            };

            ctor.intersection(cx.tcx, &pat)?;

            // Constructor splitting should ensure that all intersections we encounter are actually
            // inclusions.
            let (pat_lo, pat_hi) = pat.range.into_inner();
            let (ctor_lo, ctor_hi) = ctor.range.clone().into_inner();
            assert!(pat_lo <= ctor_lo && ctor_hi <= pat_hi);

            Some(PatStack::default())
        }
        ConstantValue(..) | ConstantRange(..) => {
            // Fallback for non-ranges and ranges that involve floating-point numbers, which are
            // not conveniently handled by `IntRange`. For these cases, the constructor may not be
            // a range so intersection actually devolves into being covered by the pattern.
            let (pat_from, pat_to, pat_end) = match *pat.kind {
                PatKind::Constant { value } => (value, value, RangeEnd::Included),
                PatKind::Range(PatRange { lo, hi, end }) => (lo, hi, end),
                _ => bug!("`constructor_intersects_pattern` called with {:?}", pat),
            };
            let (ctor_from, ctor_to, ctor_end) = match *ctor {
                ConstantValue(value) => (value, value, RangeEnd::Included),
                ConstantRange(from, to, range_end) => (from, to, range_end),
                _ => bug!(),
            };
            let order_to = compare_const_vals(cx.tcx, ctor_to, pat_to, cx.param_env, pat_from.ty)?;
            let order_from =
                compare_const_vals(cx.tcx, ctor_from, pat_from, cx.param_env, pat_from.ty)?;
            let included = (order_from != Ordering::Less)
                && ((order_to == Ordering::Less)
                    || (pat_end == ctor_end && order_to == Ordering::Equal));
            if included { Some(PatStack::default()) } else { None }
        }
        _ => bug!("`constructor_intersects_pattern` called with {:?}", ctor),
    }
}

fn patterns_for_variant<'p, 'tcx>(
    subpatterns: &'p [FieldPat<'tcx>],
    ctor_wild_subpatterns: &[&'p Pat<'tcx>],
) -> PatStack<'p, 'tcx> {
    let mut result = SmallVec::from_slice(ctor_wild_subpatterns);

    for subpat in subpatterns {
        result[subpat.field.index()] = &subpat.pattern;
    }

    debug!(
        "patterns_for_variant({:#?}, {:#?}) = {:#?}",
        subpatterns, ctor_wild_subpatterns, result
    );
    PatStack::from_vec(result)
}

/// This is the main specialization step. It expands the pattern into `arity` patterns based on the
/// constructor. For most patterns, the step is trivial, for instance tuple patterns are flattened
/// and box patterns expand into their inner pattern. Returns vec![] if the pattern does not have
/// the given constructor. See the top of the file for details.
///
/// Structure patterns with a partial wild pattern (Foo { a: 42, .. }) have their missing
/// fields filled with wild patterns.
fn specialize_one_pattern<'p, 'a: 'p, 'q: 'p, 'tcx>(
    cx: &MatchCheckCtxt<'a, 'tcx>,
    mut pat: &'q Pat<'tcx>,
    constructor: &Constructor<'tcx>,
    ctor_wild_subpatterns: &[&'p Pat<'tcx>],
) -> SmallVec<[PatStack<'p, 'tcx>; 1]> {
    while let PatKind::AscribeUserType { ref subpattern, .. } = *pat.kind {
        pat = subpattern;
    }

    if let Wildcard | MissingConstructors(_) = constructor {
        // If `constructor` is `Wildcard`: either there were only wildcards in the first component
        // of the matrix, or we are in a special non_exhaustive case where we pretend the type has
        // an extra `_` constructor to prevent exhaustive matching. In both cases, all non-wildcard
        // constructors should be discarded.
        // If `constructor` is `MissingConstructors(_)`: by the invariant of MissingConstructors,
        // we know that all non-wildcard constructors should be discarded.
        return match *pat.kind {
            PatKind::Binding { .. } | PatKind::Wild => smallvec![PatStack::empty()],
            _ => smallvec![],
        };
    }

    match *pat.kind {
        PatKind::AscribeUserType { .. } => unreachable!(), // Handled above

        PatKind::Binding { .. } | PatKind::Wild => {
            smallvec![PatStack::from_slice(ctor_wild_subpatterns)]
        }

        PatKind::Variant { adt_def, variant_index, ref subpatterns, .. } => {
            let ref variant = adt_def.variants[variant_index];
            if Variant(variant.def_id) == *constructor {
                smallvec![patterns_for_variant(subpatterns, ctor_wild_subpatterns)]
            } else {
                smallvec![]
            }
        }

        PatKind::Leaf { ref subpatterns } => {
            smallvec![patterns_for_variant(subpatterns, ctor_wild_subpatterns)]
        }

        PatKind::Deref { ref subpattern } => smallvec![PatStack::from_pattern(subpattern)],

        PatKind::Constant { value } if constructor.is_slice() => {
            // We extract an `Option` for the pointer because slices of zero
            // elements don't necessarily point to memory, they are usually
            // just integers. The only time they should be pointing to memory
            // is when they are subslices of nonzero slices.
            let (alloc, offset, n, ty) = match value.ty.kind {
                ty::Array(t, n) => match value.val {
                    ConstValue::ByRef { offset, alloc, .. } => {
                        (alloc, offset, n.eval_usize(cx.tcx, cx.param_env), t)
                    }
                    _ => span_bug!(pat.span, "array pattern is {:?}", value,),
                },
                ty::Slice(t) => {
                    match value.val {
                        ConstValue::Slice { data, start, end } => {
                            (data, Size::from_bytes(start as u64), (end - start) as u64, t)
                        }
                        ConstValue::ByRef { .. } => {
                            // FIXME(oli-obk): implement `deref` for `ConstValue`
                            return smallvec![];
                        }
                        _ => span_bug!(
                            pat.span,
                            "slice pattern constant must be scalar pair but is {:?}",
                            value,
                        ),
                    }
                }
                _ => span_bug!(
                    pat.span,
                    "unexpected const-val {:?} with ctor {:?}",
                    value,
                    constructor,
                ),
            };
            if ctor_wild_subpatterns.len() as u64 == n {
                // convert a constant slice/array pattern to a list of patterns.
                let layout = if let Ok(layout) = cx.tcx.layout_of(cx.param_env.and(ty)) {
                    layout
                } else {
                    return smallvec![];
                };
                let ptr = Pointer::new(AllocId(0), offset);
                let stack: Option<PatStack<'_, '_>> = (0..n)
                    .map(|i| {
                        let ptr = ptr.offset(layout.size * i, &cx.tcx).ok()?;
                        let scalar = alloc.read_scalar(&cx.tcx, ptr, layout.size).ok()?;
                        let scalar = scalar.not_undef().ok()?;
                        let value = ty::Const::from_scalar(cx.tcx, scalar, ty);
                        let pattern =
                            Pat { ty, span: pat.span, kind: box PatKind::Constant { value } };
                        Some(&*cx.pattern_arena.alloc(pattern))
                    })
                    .collect();
                stack.into_iter().collect()
            } else {
                smallvec![]
            }
        }

        PatKind::Constant { .. } | PatKind::Range { .. } => {
            // If the constructor is a:
            // - Single value: add a row if the pattern contains the constructor.
            // - Range: add a row if the constructor intersects the pattern.
            if let Some(ps) = constructor_intersects_pattern(cx, constructor, pat) {
                smallvec![ps]
            } else {
                smallvec![]
            }
        }

        PatKind::Array { ref prefix, ref slice, ref suffix }
        | PatKind::Slice { ref prefix, ref slice, ref suffix } => match *constructor {
            FixedLenSlice(..) | VarLenSlice(..) => {
                let pat_len = prefix.len() + suffix.len();
                if let Some(slice_count) = ctor_wild_subpatterns.len().checked_sub(pat_len) {
                    if slice_count == 0 || slice.is_some() {
                        smallvec![
                            prefix
                                .iter()
                                .chain(
                                    ctor_wild_subpatterns
                                        .iter()
                                        .map(|p| *p)
                                        .skip(prefix.len())
                                        .take(slice_count)
                                        .chain(suffix.iter()),
                                )
                                .collect(),
                        ]
                    } else {
                        smallvec![]
                    }
                } else {
                    smallvec![]
                }
            }
            ConstantValue(cv) => {
                match slice_pat_covered_by_const(
                    cx.tcx,
                    pat.span,
                    cv,
                    prefix,
                    slice,
                    suffix,
                    cx.param_env,
                ) {
                    Ok(true) => smallvec![PatStack::default()],
                    Ok(false) | Err(ErrorReported) => smallvec![],
                }
            }
            _ => span_bug!(pat.span, "unexpected ctor {:?} for slice pat", constructor),
        },

        PatKind::Or { .. } => {
            bug!("support for or-patterns has not been fully implemented yet.");
        }
    }
}
