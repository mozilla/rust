// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.


use check::FnCtxt;
use rustc::ty::Ty;
use rustc::infer::{InferOk, TypeOrigin};

use syntax::ast;
use syntax_pos::{self, Span};
use rustc::hir;
use rustc::ty::{self, ImplOrTraitItem};

use super::method::probe;

impl<'a, 'gcx, 'tcx> FnCtxt<'a, 'gcx, 'tcx> {
    // Requires that the two types unify, and prints an error message if
    // they don't.
    pub fn demand_suptype(&self, sp: Span, expected: Ty<'tcx>, actual: Ty<'tcx>) {
        let origin = TypeOrigin::Misc(sp);
        match self.sub_types(false, origin, actual, expected) {
            Ok(InferOk { obligations, .. }) => {
                // FIXME(#32730) propagate obligations
                assert!(obligations.is_empty());
            },
            Err(e) => {
                self.report_mismatched_types(origin, expected, actual, e).emit();
            }
        }
    }

    pub fn demand_eqtype(&self, sp: Span, expected: Ty<'tcx>, actual: Ty<'tcx>) {
        self.demand_eqtype_with_origin(TypeOrigin::Misc(sp), expected, actual);
    }

    pub fn demand_eqtype_with_origin(&self,
                                     origin: TypeOrigin,
                                     expected: Ty<'tcx>,
                                     actual: Ty<'tcx>)
    {
        match self.eq_types(false, origin, actual, expected) {
            Ok(InferOk { obligations, .. }) => {
                // FIXME(#32730) propagate obligations
                assert!(obligations.is_empty());
            },
            Err(e) => {
                self.report_mismatched_types(origin, expected, actual, e).emit();
            }
        }
    }

    /// This function is used to determine potential "simple" improvements or users' errors and
    /// provide them useful help. For example:
    ///
    /// ```
    /// fn some_fn(s: &str) {}
    ///
    /// let x = "hey!".to_owned();
    /// some_fn(x); // error
    /// ```
    ///
    /// No need to find every potential function which could make a coercion to transform a
    /// `String` into a `&str` since a `&` would do the trick!
    ///
    /// In addition of this check, it also checks between references mutability state. If the
    /// expected is mutable but the provided isn't, maybe we could just say "Hey, try with
    /// `&mut`!".
    fn check_ref(&self,
                 expr: &hir::Expr,
                 checked_ty: Ty<'tcx>,
                 expected: Ty<'tcx>)
                 -> Option<String> {
        match (&expected.sty, &checked_ty.sty) {
            (&ty::TyRef(_, expected_mutability),
             &ty::TyRef(_, checked_mutability)) => {
                // check if there is a mutability difference
                if checked_mutability.mutbl == hir::Mutability::MutImmutable &&
                   checked_mutability.mutbl != expected_mutability.mutbl &&
                   self.can_sub_types(&checked_mutability.ty,
                                      expected_mutability.ty).is_ok() {
                    if let Ok(src) = self.tcx.sess.codemap().span_to_snippet(expr.span) {
                        return Some(format!("try with `&mut {}`", &src.replace("&", "")));
                    }
                }
                None
            }
            (&ty::TyRef(_, mutability), _) => {
                // Check if it can work when put into a ref. For example:
                //
                // ```
                // fn bar(x: &mut i32) {}
                //
                // let x = 0u32;
                // bar(&x); // error, expected &mut
                // ```
                let ref_ty = match mutability.mutbl {
                    hir::Mutability::MutMutable => self.tcx.mk_mut_ref(
                                                       self.tcx.mk_region(ty::ReStatic),
                                                       checked_ty),
                    hir::Mutability::MutImmutable => self.tcx.mk_imm_ref(
                                                       self.tcx.mk_region(ty::ReStatic),
                                                       checked_ty),
                };
                if self.try_coerce(expr, ref_ty, expected).is_ok() {
                    if let Ok(src) = self.tcx.sess.codemap().span_to_snippet(expr.span) {
                        return Some(format!("try with `{}{}`",
                                            match mutability.mutbl {
                                                hir::Mutability::MutMutable => "&mut ",
                                                hir::Mutability::MutImmutable => "&",
                                            },
                                            &src));
                    }
                }
                None
            }
            _ => None,
        }
    }

    // Checks that the type of `expr` can be coerced to `expected`.
    pub fn demand_coerce(&self, expr: &hir::Expr, checked_ty: Ty<'tcx>, expected: Ty<'tcx>) {
        let expected = self.resolve_type_vars_with_obligations(expected);
        if let Err(e) = self.try_coerce(expr, checked_ty, expected) {
            let origin = TypeOrigin::Misc(expr.span);
            let expr_ty = self.resolve_type_vars_with_obligations(checked_ty);
            let mode = probe::Mode::MethodCall;
            let suggestions = if let Some(s) = self.check_ref(expr, checked_ty, expected) {
                Some(s)
            } else {
                let suggestions = self.probe_for_return_type(syntax_pos::DUMMY_SP,
                                                             mode,
                                                             expected,
                                                             checked_ty,
                                                             ast::DUMMY_NODE_ID);
                if suggestions.len() > 0 {
                    Some(format!("here are some functions which \
                                  might fulfill your needs:\n - {}",
                                 self.get_best_match(&suggestions)))
                } else {
                    None
                }
            };
            let mut err = self.report_mismatched_types(origin, expected, expr_ty, e);
            if let Some(suggestions) = suggestions {
                err.help(&suggestions);
            }
            err.emit();
        }
    }

    fn format_method_suggestion(&self, method: &ImplOrTraitItem<'tcx>) -> String {
        format!(".{}({})",
                method.name(),
                if self.has_not_input_arg(method) {
                    ""
                } else {
                    "..."
                })
    }

    fn display_suggested_methods(&self, methods: &[ImplOrTraitItem<'tcx>]) -> String {
        methods.iter()
               .take(5)
               .map(|method| self.format_method_suggestion(&*method))
               .collect::<Vec<String>>()
               .join("\n - ")
    }

    fn get_best_match(&self, methods: &[ImplOrTraitItem<'tcx>]) -> String {
        let no_argument_methods: Vec<_> =
            methods.iter()
                   .filter(|ref x| self.has_not_input_arg(&*x))
                   .map(|x| x.clone())
                   .collect();
        if no_argument_methods.len() > 0 {
            self.display_suggested_methods(&no_argument_methods)
        } else {
            self.display_suggested_methods(&methods)
        }
    }

    // This function checks if the method isn't static and takes other arguments than `self`.
    fn has_not_input_arg(&self, method: &ImplOrTraitItem<'tcx>) -> bool {
        match *method {
            ImplOrTraitItem::MethodTraitItem(ref x) => {
                x.fty.sig.skip_binder().inputs.len() == 1
            }
            _ => false,
        }
    }
}
