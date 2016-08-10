// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use hir::def::Def;
use rustc::infer::{self, InferOk, TypeOrigin};
use hir::pat_util::EnumerateAndAdjustIterator;
use rustc::ty::subst::Substs;
use rustc::ty::{self, Ty, TypeFoldable, LvaluePreference, VariantKind};
use check::{FnCtxt, Expectation};
use lint;
use util::nodemap::FnvHashMap;

use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::cmp;
use syntax::ast;
use syntax::codemap::Spanned;
use syntax::ptr::P;
use syntax_pos::Span;

use rustc::hir::{self, PatKind};
use rustc::hir::print as pprust;

impl<'a, 'gcx, 'tcx> FnCtxt<'a, 'gcx, 'tcx> {
    pub fn check_pat(&self, pat: &'gcx hir::Pat, expected: Ty<'tcx>) {
        let tcx = self.tcx;

        debug!("check_pat(pat={:?},expected={:?})", pat, expected);

        match pat.node {
            PatKind::Wild => {
                self.write_ty(pat.id, expected);
            }
            PatKind::Lit(ref lt) => {
                self.check_expr(&lt);
                let expr_ty = self.expr_ty(&lt);

                // Byte string patterns behave the same way as array patterns
                // They can denote both statically and dynamically sized byte arrays
                let mut pat_ty = expr_ty;
                if let hir::ExprLit(ref lt) = lt.node {
                    if let ast::LitKind::ByteStr(_) = lt.node {
                        let expected_ty = self.structurally_resolved_type(pat.span, expected);
                        if let ty::TyRef(_, mt) = expected_ty.sty {
                            if let ty::TySlice(_) = mt.ty.sty {
                                pat_ty = tcx.mk_imm_ref(tcx.mk_region(ty::ReStatic),
                                                         tcx.mk_slice(tcx.types.u8))
                            }
                        }
                    }
                }

                self.write_ty(pat.id, pat_ty);

                // somewhat surprising: in this case, the subtyping
                // relation goes the opposite way as the other
                // cases. Actually what we really want is not a subtyping
                // relation at all but rather that there exists a LUB (so
                // that they can be compared). However, in practice,
                // constants are always scalars or strings.  For scalars
                // subtyping is irrelevant, and for strings `expr_ty` is
                // type is `&'static str`, so if we say that
                //
                //     &'static str <: expected
                //
                // that's equivalent to there existing a LUB.
                self.demand_suptype(pat.span, expected, pat_ty);
            }
            PatKind::Range(ref begin, ref end) => {
                self.check_expr(begin);
                self.check_expr(end);

                let lhs_ty = self.expr_ty(begin);
                let rhs_ty = self.expr_ty(end);

                // Check that both end-points are of numeric or char type.
                let numeric_or_char = |ty: Ty| ty.is_numeric() || ty.is_char();
                let lhs_compat = numeric_or_char(lhs_ty);
                let rhs_compat = numeric_or_char(rhs_ty);

                if !lhs_compat || !rhs_compat {
                    let span = if !lhs_compat && !rhs_compat {
                        pat.span
                    } else if !lhs_compat {
                        begin.span
                    } else {
                        end.span
                    };

                    struct_span_err!(tcx.sess, span, E0029,
                        "only char and numeric types are allowed in range patterns")
                        .span_label(span, &format!("ranges require char or numeric types"))
                        .note(&format!("start type: {}", self.ty_to_string(lhs_ty)))
                        .note(&format!("end type: {}", self.ty_to_string(rhs_ty)))
                        .emit();
                    return;
                }

                // Now that we know the types can be unified we find the unified type and use
                // it to type the entire expression.
                let common_type = self.resolve_type_vars_if_possible(&lhs_ty);

                self.write_ty(pat.id, common_type);

                // subtyping doesn't matter here, as the value is some kind of scalar
                self.demand_eqtype(pat.span, expected, lhs_ty);
                self.demand_eqtype(pat.span, expected, rhs_ty);
            }
            PatKind::Binding(bm, _, ref sub) => {
                let typ = self.local_ty(pat.span, pat.id);
                match bm {
                    hir::BindByRef(mutbl) => {
                        // if the binding is like
                        //    ref x | ref const x | ref mut x
                        // then `x` is assigned a value of type `&M T` where M is the mutability
                        // and T is the expected type.
                        let region_var = self.next_region_var(infer::PatternRegion(pat.span));
                        let mt = ty::TypeAndMut { ty: expected, mutbl: mutbl };
                        let region_ty = tcx.mk_ref(region_var, mt);

                        // `x` is assigned a value of type `&M T`, hence `&M T <: typeof(x)` is
                        // required. However, we use equality, which is stronger. See (*) for
                        // an explanation.
                        self.demand_eqtype(pat.span, region_ty, typ);
                    }
                    // otherwise the type of x is the expected type T
                    hir::BindByValue(_) => {
                        // As above, `T <: typeof(x)` is required but we
                        // use equality, see (*) below.
                        self.demand_eqtype(pat.span, expected, typ);
                    }
                }

                self.write_ty(pat.id, typ);

                // if there are multiple arms, make sure they all agree on
                // what the type of the binding `x` ought to be
                match tcx.expect_def(pat.id) {
                    Def::Err => {}
                    Def::Local(_, var_id) => {
                        if var_id != pat.id {
                            let vt = self.local_ty(pat.span, var_id);
                            self.demand_eqtype(pat.span, vt, typ);
                        }
                    }
                    d => bug!("bad def for pattern binding `{:?}`", d)
                }

                if let Some(ref p) = *sub {
                    self.check_pat(&p, expected);
                }
            }
            PatKind::TupleStruct(ref path, ref subpats, ddpos) => {
                self.check_pat_tuple_struct(pat, path, &subpats, ddpos, expected);
            }
            PatKind::Path(ref opt_qself, ref path) => {
                let opt_qself_ty = opt_qself.as_ref().map(|qself| self.to_ty(&qself.ty));
                self.check_pat_path(pat, opt_qself_ty, path, expected);
            }
            PatKind::Struct(ref path, ref fields, etc) => {
                self.check_pat_struct(pat, path, fields, etc, expected);
            }
            PatKind::Tuple(ref elements, ddpos) => {
                let mut expected_len = elements.len();
                if ddpos.is_some() {
                    // Require known type only when `..` is present
                    if let ty::TyTuple(ref tys) =
                            self.structurally_resolved_type(pat.span, expected).sty {
                        expected_len = tys.len();
                    }
                }
                let max_len = cmp::max(expected_len, elements.len());

                let element_tys: Vec<_> = (0 .. max_len).map(|_| self.next_ty_var()).collect();
                let pat_ty = tcx.mk_tup(element_tys.clone());
                self.write_ty(pat.id, pat_ty);
                self.demand_eqtype(pat.span, expected, pat_ty);
                for (i, elem) in elements.iter().enumerate_and_adjust(max_len, ddpos) {
                    self.check_pat(elem, &element_tys[i]);
                }
            }
            PatKind::Box(ref inner) => {
                let inner_ty = self.next_ty_var();
                let uniq_ty = tcx.mk_box(inner_ty);

                if self.check_dereferencable(pat.span, expected, &inner) {
                    // Here, `demand::subtype` is good enough, but I don't
                    // think any errors can be introduced by using
                    // `demand::eqtype`.
                    self.demand_eqtype(pat.span, expected, uniq_ty);
                    self.write_ty(pat.id, uniq_ty);
                    self.check_pat(&inner, inner_ty);
                } else {
                    self.write_error(pat.id);
                    self.check_pat(&inner, tcx.types.err);
                }
            }
            PatKind::Ref(ref inner, mutbl) => {
                let expected = self.shallow_resolve(expected);
                if self.check_dereferencable(pat.span, expected, &inner) {
                    // `demand::subtype` would be good enough, but using
                    // `eqtype` turns out to be equally general. See (*)
                    // below for details.

                    // Take region, inner-type from expected type if we
                    // can, to avoid creating needless variables.  This
                    // also helps with the bad interactions of the given
                    // hack detailed in (*) below.
                    let (rptr_ty, inner_ty) = match expected.sty {
                        ty::TyRef(_, mt) if mt.mutbl == mutbl => {
                            (expected, mt.ty)
                        }
                        _ => {
                            let inner_ty = self.next_ty_var();
                            let mt = ty::TypeAndMut { ty: inner_ty, mutbl: mutbl };
                            let region = self.next_region_var(infer::PatternRegion(pat.span));
                            let rptr_ty = tcx.mk_ref(region, mt);
                            self.demand_eqtype(pat.span, expected, rptr_ty);
                            (rptr_ty, inner_ty)
                        }
                    };

                    self.write_ty(pat.id, rptr_ty);
                    self.check_pat(&inner, inner_ty);
                } else {
                    self.write_error(pat.id);
                    self.check_pat(&inner, tcx.types.err);
                }
            }
            PatKind::Vec(ref before, ref slice, ref after) => {
                let expected_ty = self.structurally_resolved_type(pat.span, expected);
                let (inner_ty, slice_ty) = match expected_ty.sty {
                    ty::TyArray(inner_ty, size) => {
                        let min_len = before.len() + after.len();
                        if slice.is_none() {
                            if min_len != size {
                                span_err!(tcx.sess, pat.span, E0527,
                                          "pattern requires {} elements but array has {}",
                                          min_len, size);
                            }
                            (inner_ty, tcx.types.err)
                        } else if let Some(rest) = size.checked_sub(min_len) {
                            (inner_ty, tcx.mk_array(inner_ty, rest))
                        } else {
                            struct_span_err!(tcx.sess, pat.span, E0528,
                                    "pattern requires at least {} elements but array has {}",
                                    min_len, size)
                                .span_label(pat.span,
                                    &format!("pattern cannot match array of {} elements", size))
                                .emit();
                            (inner_ty, tcx.types.err)
                        }
                    }
                    ty::TySlice(inner_ty) => (inner_ty, expected_ty),
                    _ => {
                        if !expected_ty.references_error() {
                            let mut err = struct_span_err!(
                                tcx.sess, pat.span, E0529,
                                "expected an array or slice, found `{}`",
                                expected_ty);
                            if let ty::TyRef(_, ty::TypeAndMut { mutbl: _, ty }) = expected_ty.sty {
                                match ty.sty {
                                    ty::TyArray(..) | ty::TySlice(..) => {
                                        err.help("the semantics of slice patterns changed \
                                                  recently; see issue #23121");
                                    }
                                    _ => {}
                                }
                            }

                            err.span_label( pat.span,
                                &format!("pattern cannot match with input type `{}`", expected_ty)
                            ).emit();
                        }
                        (tcx.types.err, tcx.types.err)
                    }
                };

                self.write_ty(pat.id, expected_ty);

                for elt in before {
                    self.check_pat(&elt, inner_ty);
                }
                if let Some(ref slice) = *slice {
                    self.check_pat(&slice, slice_ty);
                }
                for elt in after {
                    self.check_pat(&elt, inner_ty);
                }
            }
        }

        // (*) In most of the cases above (literals and constants being
        // the exception), we relate types using strict equality, evewn
        // though subtyping would be sufficient. There are a few reasons
        // for this, some of which are fairly subtle and which cost me
        // (nmatsakis) an hour or two debugging to remember, so I thought
        // I'd write them down this time.
        //
        // 1. There is no loss of expressiveness here, though it does
        // cause some inconvenience. What we are saying is that the type
        // of `x` becomes *exactly* what is expected. This can cause unnecessary
        // errors in some cases, such as this one:
        // it will cause errors in a case like this:
        //
        // ```
        // fn foo<'x>(x: &'x int) {
        //    let a = 1;
        //    let mut z = x;
        //    z = &a;
        // }
        // ```
        //
        // The reason we might get an error is that `z` might be
        // assigned a type like `&'x int`, and then we would have
        // a problem when we try to assign `&a` to `z`, because
        // the lifetime of `&a` (i.e., the enclosing block) is
        // shorter than `'x`.
        //
        // HOWEVER, this code works fine. The reason is that the
        // expected type here is whatever type the user wrote, not
        // the initializer's type. In this case the user wrote
        // nothing, so we are going to create a type variable `Z`.
        // Then we will assign the type of the initializer (`&'x
        // int`) as a subtype of `Z`: `&'x int <: Z`. And hence we
        // will instantiate `Z` as a type `&'0 int` where `'0` is
        // a fresh region variable, with the constraint that `'x :
        // '0`.  So basically we're all set.
        //
        // Note that there are two tests to check that this remains true
        // (`regions-reassign-{match,let}-bound-pointer.rs`).
        //
        // 2. Things go horribly wrong if we use subtype. The reason for
        // THIS is a fairly subtle case involving bound regions. See the
        // `givens` field in `region_inference`, as well as the test
        // `regions-relate-bound-regions-on-closures-to-inference-variables.rs`,
        // for details. Short version is that we must sometimes detect
        // relationships between specific region variables and regions
        // bound in a closure signature, and that detection gets thrown
        // off when we substitute fresh region variables here to enable
        // subtyping.
    }

    pub fn check_dereferencable(&self, span: Span, expected: Ty<'tcx>, inner: &hir::Pat) -> bool {
        if let PatKind::Binding(..) = inner.node {
            if let Some(mt) = self.shallow_resolve(expected).builtin_deref(true, ty::NoPreference) {
                if let ty::TyTrait(..) = mt.ty.sty {
                    // This is "x = SomeTrait" being reduced from
                    // "let &x = &SomeTrait" or "let box x = Box<SomeTrait>", an error.
                    let type_str = self.ty_to_string(expected);
                    struct_span_err!(self.tcx.sess, span, E0033,
                              "type `{}` cannot be dereferenced", type_str)
                        .span_label(span, &format!("type `{}` cannot be dereferenced", type_str))
                        .emit();
                    return false
                }
            }
        }
        true
    }
}

impl<'a, 'gcx, 'tcx> FnCtxt<'a, 'gcx, 'tcx> {
    pub fn check_match(&self,
                       expr: &'gcx hir::Expr,
                       discrim: &'gcx hir::Expr,
                       arms: &'gcx [hir::Arm],
                       expected: Expectation<'tcx>,
                       match_src: hir::MatchSource) {
        let tcx = self.tcx;

        // Not entirely obvious: if matches may create ref bindings, we
        // want to use the *precise* type of the discriminant, *not* some
        // supertype, as the "discriminant type" (issue #23116).
        let contains_ref_bindings = arms.iter()
                                        .filter_map(|a| tcx.arm_contains_ref_binding(a))
                                        .max_by_key(|m| match *m {
                                            hir::MutMutable => 1,
                                            hir::MutImmutable => 0,
                                        });
        let discrim_ty;
        if let Some(m) = contains_ref_bindings {
            self.check_expr_with_lvalue_pref(discrim, LvaluePreference::from_mutbl(m));
            discrim_ty = self.expr_ty(discrim);
        } else {
            // ...but otherwise we want to use any supertype of the
            // discriminant. This is sort of a workaround, see note (*) in
            // `check_pat` for some details.
            discrim_ty = self.next_ty_var();
            self.check_expr_has_type(discrim, discrim_ty);
        };

        // Typecheck the patterns first, so that we get types for all the
        // bindings.
        for arm in arms {
            for p in &arm.pats {
                self.check_pat(&p, discrim_ty);
            }
        }

        // Now typecheck the blocks.
        //
        // The result of the match is the common supertype of all the
        // arms. Start out the value as bottom, since it's the, well,
        // bottom the type lattice, and we'll be moving up the lattice as
        // we process each arm. (Note that any match with 0 arms is matching
        // on any empty type and is therefore unreachable; should the flow
        // of execution reach it, we will panic, so bottom is an appropriate
        // type in that case)
        let expected = expected.adjust_for_branches(self);
        let mut result_ty = self.next_diverging_ty_var();
        let coerce_first = match expected {
            // We don't coerce to `()` so that if the match expression is a
            // statement it's branches can have any consistent type. That allows
            // us to give better error messages (pointing to a usually better
            // arm for inconsistent arms or to the whole match when a `()` type
            // is required).
            Expectation::ExpectHasType(ety) if ety != self.tcx.mk_nil() => {
                ety
            }
            _ => result_ty
        };
        for (i, arm) in arms.iter().enumerate() {
            if let Some(ref e) = arm.guard {
                self.check_expr_has_type(e, tcx.types.bool);
            }
            self.check_expr_with_expectation(&arm.body, expected);
            let arm_ty = self.expr_ty(&arm.body);

            if result_ty.references_error() || arm_ty.references_error() {
                result_ty = tcx.types.err;
                continue;
            }

            // Handle the fallback arm of a desugared if-let like a missing else.
            let is_if_let_fallback = match match_src {
                hir::MatchSource::IfLetDesugar { contains_else_clause: false } => {
                    i == arms.len() - 1 && arm_ty.is_nil()
                }
                _ => false
            };

            let origin = if is_if_let_fallback {
                TypeOrigin::IfExpressionWithNoElse(expr.span)
            } else {
                TypeOrigin::MatchExpressionArm(expr.span, arm.body.span, match_src)
            };

            let result = if is_if_let_fallback {
                self.eq_types(true, origin, arm_ty, result_ty)
                    .map(|InferOk { obligations, .. }| {
                        // FIXME(#32730) propagate obligations
                        assert!(obligations.is_empty());
                        arm_ty
                    })
            } else if i == 0 {
                // Special-case the first arm, as it has no "previous expressions".
                self.try_coerce(&arm.body, coerce_first)
            } else {
                let prev_arms = || arms[..i].iter().map(|arm| &*arm.body);
                self.try_find_coercion_lub(origin, prev_arms, result_ty, &arm.body)
            };

            result_ty = match result {
                Ok(ty) => ty,
                Err(e) => {
                    let (expected, found) = if is_if_let_fallback {
                        (arm_ty, result_ty)
                    } else {
                        (result_ty, arm_ty)
                    };
                    self.report_mismatched_types(origin, expected, found, e);
                    self.tcx.types.err
                }
            };
        }

        self.write_ty(expr.id, result_ty);
    }
}

impl<'a, 'gcx, 'tcx> FnCtxt<'a, 'gcx, 'tcx> {
    fn check_pat_struct(&self,
                        pat: &'gcx hir::Pat,
                        path: &hir::Path,
                        fields: &'gcx [Spanned<hir::FieldPat>],
                        etc: bool,
                        expected: Ty<'tcx>)
    {
        // Resolve the path and check the definition for errors.
        let (variant, pat_ty) = if let Some(variant_ty) = self.check_struct_path(path, pat.id,
                                                                                 pat.span) {
            variant_ty
        } else {
            self.write_error(pat.id);
            for field in fields {
                self.check_pat(&field.node.pat, self.tcx.types.err);
            }
            return;
        };

        // Type check the path.
        self.demand_eqtype(pat.span, expected, pat_ty);

        // Type check subpatterns.
        let substs = match pat_ty.sty {
            ty::TyStruct(_, substs) | ty::TyUnion(_, substs) | ty::TyEnum(_, substs) => substs,
            _ => span_bug!(pat.span, "struct variant is not an ADT")
        };
        self.check_struct_pat_fields(pat.span, fields, variant, substs, etc);
    }

    fn check_pat_path(&self,
                      pat: &hir::Pat,
                      opt_self_ty: Option<Ty<'tcx>>,
                      path: &hir::Path,
                      expected: Ty<'tcx>)
    {
        let tcx = self.tcx;
        let report_unexpected_def = || {
            span_err!(tcx.sess, pat.span, E0533,
                      "`{}` does not name a unit variant, unit struct or a constant",
                      pprust::path_to_string(path));
            self.write_error(pat.id);
        };

        // Resolve the path and check the definition for errors.
        let (def, opt_ty, segments) = self.resolve_ty_and_def_ufcs(opt_self_ty, path,
                                                                   pat.id, pat.span);
        match def {
            Def::Err => {
                self.set_tainted_by_errors();
                self.write_error(pat.id);
                return;
            }
            Def::Method(..) => {
                report_unexpected_def();
                return;
            }
            Def::Variant(..) | Def::Struct(..) => {
                let variant = tcx.expect_variant_def(def);
                if variant.kind != VariantKind::Unit {
                    report_unexpected_def();
                    return;
                }
            }
            Def::Const(..) | Def::AssociatedConst(..) => {} // OK
            _ => bug!("unexpected pattern definition {:?}", def)
        }

        // Type check the path.
        let pat_ty = self.instantiate_value_path(segments, opt_ty, def, pat.span, pat.id);
        self.demand_suptype(pat.span, expected, pat_ty);
    }

    fn check_pat_tuple_struct(&self,
                              pat: &hir::Pat,
                              path: &hir::Path,
                              subpats: &'gcx [P<hir::Pat>],
                              ddpos: Option<usize>,
                              expected: Ty<'tcx>)
    {
        let tcx = self.tcx;
        let on_error = || {
            self.write_error(pat.id);
            for pat in subpats {
                self.check_pat(&pat, tcx.types.err);
            }
        };
        let report_unexpected_def = |is_lint| {
            let msg = format!("`{}` does not name a tuple variant or a tuple struct",
                              pprust::path_to_string(path));
            if is_lint {
                tcx.sess.add_lint(lint::builtin::MATCH_OF_UNIT_VARIANT_VIA_PAREN_DOTDOT,
                                  pat.id, pat.span, msg);
            } else {
                struct_span_err!(tcx.sess, pat.span, E0164, "{}", msg)
                    .span_label(pat.span, &format!("not a tuple variant or struct")).emit();
                on_error();
            }
        };

        // Resolve the path and check the definition for errors.
        let (def, opt_ty, segments) = self.resolve_ty_and_def_ufcs(None, path, pat.id, pat.span);
        let variant = match def {
            Def::Err => {
                self.set_tainted_by_errors();
                on_error();
                return;
            }
            Def::Const(..) | Def::AssociatedConst(..) | Def::Method(..) => {
                report_unexpected_def(false);
                return;
            }
            Def::Variant(..) | Def::Struct(..) => {
                tcx.expect_variant_def(def)
            }
            _ => bug!("unexpected pattern definition {:?}", def)
        };
        if variant.kind == VariantKind::Unit && subpats.is_empty() && ddpos.is_some() {
            // Matching unit structs with tuple variant patterns (`UnitVariant(..)`)
            // is allowed for backward compatibility.
            report_unexpected_def(true);
        } else if variant.kind != VariantKind::Tuple {
            report_unexpected_def(false);
            return;
        }

        // Type check the path.
        let pat_ty = self.instantiate_value_path(segments, opt_ty, def, pat.span, pat.id);

        let pat_ty = if pat_ty.is_fn() {
            // Replace constructor type with constructed type for tuple struct patterns.
            tcx.no_late_bound_regions(&pat_ty.fn_ret()).unwrap()
        } else {
            // Leave the type as is for unit structs (backward compatibility).
            pat_ty
        };
        self.write_ty(pat.id, pat_ty);
        self.demand_eqtype(pat.span, expected, pat_ty);

        // Type check subpatterns.
        if subpats.len() == variant.fields.len() ||
                subpats.len() < variant.fields.len() && ddpos.is_some() {
            let substs = match pat_ty.sty {
                ty::TyStruct(_, substs) | ty::TyEnum(_, substs) => substs,
                ref ty => bug!("unexpected pattern type {:?}", ty),
            };
            for (i, subpat) in subpats.iter().enumerate_and_adjust(variant.fields.len(), ddpos) {
                let field_ty = self.field_ty(subpat.span, &variant.fields[i], substs);
                self.check_pat(&subpat, field_ty);
            }
        } else {
            let subpats_ending = if subpats.len() == 1 {
                ""
            } else {
                "s"
            };
            let fields_ending = if variant.fields.len() == 1 {
                ""
            } else {
                "s"
            };
            struct_span_err!(tcx.sess, pat.span, E0023,
                             "this pattern has {} field{}, but the corresponding {} has {} field{}",
                             subpats.len(), subpats_ending, def.kind_name(),
                             variant.fields.len(),  fields_ending)
                .span_label(pat.span, &format!("expected {} field{}, found {}",
                                               variant.fields.len(), fields_ending, subpats.len()))
                .emit();
            on_error();
        }
    }

    /// `path` is the AST path item naming the type of this struct.
    /// `fields` is the field patterns of the struct pattern.
    /// `struct_fields` describes the type of each field of the struct.
    /// `struct_id` is the ID of the struct.
    /// `etc` is true if the pattern said '...' and false otherwise.
    pub fn check_struct_pat_fields(&self,
                                   span: Span,
                                   fields: &'gcx [Spanned<hir::FieldPat>],
                                   variant: ty::VariantDef<'tcx>,
                                   substs: &Substs<'tcx>,
                                   etc: bool) {
        let tcx = self.tcx;

        // Index the struct fields' types.
        let field_map = variant.fields
            .iter()
            .map(|field| (field.name, field))
            .collect::<FnvHashMap<_, _>>();

        // Keep track of which fields have already appeared in the pattern.
        let mut used_fields = FnvHashMap();

        // Typecheck each field.
        for &Spanned { node: ref field, span } in fields {
            let field_ty = match used_fields.entry(field.name) {
                Occupied(occupied) => {
                    let mut err = struct_span_err!(tcx.sess, span, E0025,
                                                   "field `{}` bound multiple times \
                                                    in the pattern",
                                                   field.name);
                    span_note!(&mut err, *occupied.get(),
                               "field `{}` previously bound here",
                               field.name);
                    err.emit();
                    tcx.types.err
                }
                Vacant(vacant) => {
                    vacant.insert(span);
                    field_map.get(&field.name)
                        .map(|f| self.field_ty(span, f, substs))
                        .unwrap_or_else(|| {
                            struct_span_err!(tcx.sess, span, E0026,
                                             "struct `{}` does not have a field named `{}`",
                                             tcx.item_path_str(variant.did),
                                             field.name)
                                .span_label(span,
                                            &format!("struct `{}` does not have field `{}`",
                                                     tcx.item_path_str(variant.did),
                                                     field.name))
                                .emit();

                            tcx.types.err
                        })
                }
            };

            self.check_pat(&field.pat, field_ty);
        }

        // Report an error if not all the fields were specified.
        if !etc {
            for field in variant.fields
                .iter()
                .filter(|field| !used_fields.contains_key(&field.name)) {
                struct_span_err!(tcx.sess, span, E0027,
                                "pattern does not mention field `{}`",
                                field.name)
                                .span_label(span, &format!("missing field `{}`", field.name))
                                .emit();
            }
        }
    }
}
