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

use hir::def_id::DefId;

use std::rc::Rc;

use super::method::probe;

struct MethodInfo<'tcx> {
    ast: Option<ast::Attribute>,
    id: DefId,
    item: Rc<ImplOrTraitItem<'tcx>>,
}

impl<'tcx> MethodInfo<'tcx> {
    fn new(ast: Option<ast::Attribute>, id: DefId, item: Rc<ImplOrTraitItem<'tcx>>) -> MethodInfo {
        MethodInfo {
            ast: ast,
            id: id,
            item: item,
        }
    }
}

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

    // Checks that the type of `expr` can be coerced to `expected`.
    pub fn demand_coerce(&self, expr: &hir::Expr, checked_ty: Ty<'tcx>, expected: Ty<'tcx>) {
        let expected = self.resolve_type_vars_with_obligations(expected);
        if let Err(e) = self.try_coerce(expr, checked_ty, expected) {
            let origin = TypeOrigin::Misc(expr.span);
            let expr_ty = self.resolve_type_vars_with_obligations(checked_ty);
            let mode = probe::Mode::MethodCall;
            let suggestions = 
                if let Ok(methods) = self.probe_return(syntax_pos::DUMMY_SP, mode, expected,
                                                   checked_ty, ast::DUMMY_NODE_ID) {
                let suggestions: Vec<_> =
                    methods.iter()
                           .filter_map(|ref x| {
                            if let Some(id) = self.get_impl_id(&x.item) {
                                Some(MethodInfo::new(None, id, Rc::new(x.item.clone())))
                            } else {
                                None
                            }})
                           .collect();
                if suggestions.len() > 0 {
                    let safe_suggestions: Vec<_> =
                        suggestions.iter()
                                   .map(|ref x| MethodInfo::new(
                                                    self.find_attr(x.id, "safe_suggestion"),
                                                                   x.id,
                                                                   x.item.clone()))
                                   .filter(|ref x| x.ast.is_some())
                                   .collect();
                    Some(if safe_suggestions.len() > 0 {
                        self.get_best_match(&safe_suggestions)
                    } else {
                        format!("no safe suggestion found, here are functions which match your \
                                 needs but be careful:\n - {}",
                                self.get_best_match(&suggestions))
                    })
                } else {
                    None
                }
            } else {
                None
            };
            let mut err = self.report_mismatched_types(origin, expected, expr_ty, e);
            if let Some(suggestions) = suggestions {
                err.help(&suggestions);
            }
            err.emit();
        }
    }

    fn get_best_match(&self, methods: &[MethodInfo<'tcx>]) -> String {
        if methods.len() == 1 {
            return format!(" - {}", methods[0].item.name());
        }
        let no_argument_methods: Vec<&MethodInfo> =
            methods.iter()
                   .filter(|ref x| self.has_not_input_arg(&*x.item))
                   .collect();
        if no_argument_methods.len() > 0 {
            no_argument_methods.iter()
                               .map(|method| format!("{}", method.item.name()))
                               .collect::<Vec<String>>()
                               .join("\n - ")
        } else {
            methods.iter()
                   .map(|method| format!("{}", method.item.name()))
                   .collect::<Vec<String>>()
                   .join("\n - ")
        }
    }

    fn get_impl_id(&self, impl_: &ImplOrTraitItem<'tcx>) -> Option<DefId> {
        match *impl_ {
            ty::ImplOrTraitItem::MethodTraitItem(ref m) => Some((*m).def_id),
            _ => None,
        }
    }

    fn has_not_input_arg(&self, method: &ImplOrTraitItem<'tcx>) -> bool {
        match *method {
            ImplOrTraitItem::MethodTraitItem(ref x) => {
                x.fty.sig.skip_binder().inputs.len() == 1
            }
            _ => false,
        }
    }
}
