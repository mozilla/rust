// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/*

# check.rs

Within the check phase of type check, we check each item one at a time
(bodies of function expressions are checked as part of the containing
function).  Inference is used to supply types wherever they are
unknown.

By far the most complex case is checking the body of a function. This
can be broken down into several distinct phases:

- gather: creates type variables to represent the type of each local
  variable and pattern binding.

- main: the main pass does the lion's share of the work: it
  determines the types of all expressions, resolves
  methods, checks for most invalid conditions, and so forth.  In
  some cases, where a type is unknown, it may create a type or region
  variable and use that as the type of an expression.

  In the process of checking, various constraints will be placed on
  these type variables through the subtyping relationships requested
  through the `demand` module.  The `typeck::infer` module is in charge
  of resolving those constraints.

- regionck: after main is complete, the regionck pass goes over all
  types looking for regions and making sure that they did not escape
  into places they are not in scope.  This may also influence the
  final assignments of the various region variables if there is some
  flexibility.

- vtable: find and records the impls to use for each trait bound that
  appears on a type parameter.

- writeback: writes the final types within a function body, replacing
  type variables with their final inferred types.  These final types
  are written into the `tcx.node_types` table, which should *never* contain
  any reference to a type variable.

## Intermediate types

While type checking a function, the intermediate types for the
expressions, blocks, and so forth contained within the function are
stored in `fcx.node_types` and `fcx.item_substs`.  These types
may contain unresolved type variables.  After type checking is
complete, the functions in the writeback module are used to take the
types from this table, resolve them, and then write them into their
permanent home in the type context `ccx.tcx`.

This means that during inferencing you should use `fcx.write_ty()`
and `fcx.expr_ty()` / `fcx.node_ty()` to write/obtain the types of
nodes within the function.

The types of top-level items, which never contain unbound type
variables, are stored directly into the `tcx` tables.

n.b.: A type variable is not the same thing as a type parameter.  A
type variable is rather an "instance" of a type parameter: that is,
given a generic function `fn foo<T>(t: T)`: while checking the
function `foo`, the type `ty_param(0)` refers to the type `T`, which
is treated in abstract.  When `foo()` is called, however, `T` will be
substituted for a fresh type variable `N`.  This variable will
eventually be resolved to some concrete type (which might itself be
type parameter).

*/


use middle::const_eval;
use middle::def;
use middle::lang_items::{ExchangeHeapLangItem, GcLangItem};
use middle::lang_items::{ManagedHeapLangItem};
use middle::lint::UnreachableCode;
use middle::pat_util::pat_id_map;
use middle::pat_util;
use middle::subst;
use middle::subst::{Subst, Substs};
use middle::ty::{FnSig, VariantInfo};
use middle::ty::{ty_param_bounds_and_ty, ty_param_substs_and_ty};
use middle::ty::{param_ty, Disr, ExprTyProvider};
use middle::ty;
use middle::ty_fold::TypeFolder;
use middle::typeck::astconv::AstConv;
use middle::typeck::astconv::{ast_region_to_region, ast_ty_to_ty};
use middle::typeck::astconv;
use middle::typeck::check::_match::pat_ctxt;
use middle::typeck::check::method::{AutoderefReceiver};
use middle::typeck::check::method::{AutoderefReceiverFlag};
use middle::typeck::check::method::{CheckTraitsAndInherentMethods};
use middle::typeck::check::method::{DontAutoderefReceiver};
use middle::typeck::check::method::{IgnoreStaticMethods, ReportStaticMethods};
use middle::typeck::check::regionmanip::replace_late_bound_regions_in_fn_sig;
use middle::typeck::check::regionmanip::relate_free_regions;
use middle::typeck::check::vtable::VtableContext;
use middle::typeck::CrateCtxt;
use middle::typeck::infer::{resolve_type, force_tvar};
use middle::typeck::infer;
use middle::typeck::rscope::RegionScope;
use middle::typeck::{lookup_def_ccx};
use middle::typeck::no_params;
use middle::typeck::{require_same_types, vtable_map};
use middle::typeck::{MethodCall, MethodMap};
use middle::lang_items::TypeIdLangItem;
use util::common::{block_query, indenter, loop_query};
use util::ppaux;
use util::ppaux::{UserString, Repr};
use util::nodemap::{FnvHashMap, NodeMap};

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::mem::replace;
use std::rc::Rc;
use std::vec::Vec;
use syntax::abi;
use syntax::ast::{Provided, Required};
use syntax::ast;
use syntax::ast_map;
use syntax::ast_util::local_def;
use syntax::ast_util;
use syntax::attr;
use syntax::codemap::Span;
use syntax::codemap;
use syntax::owned_slice::OwnedSlice;
use syntax::parse::token;
use syntax::print::pprust;
use syntax::visit;
use syntax::visit::Visitor;
use syntax;

pub mod _match;
pub mod vtable;
pub mod writeback;
pub mod regionmanip;
pub mod regionck;
pub mod demand;
pub mod method;

/// Fields that are part of a `FnCtxt` which are inherited by
/// closures defined within the function.  For example:
///
///     fn foo() {
///         bar(proc() { ... })
///     }
///
/// Here, the function `foo()` and the closure passed to
/// `bar()` will each have their own `FnCtxt`, but they will
/// share the inherited fields.
pub struct Inherited<'a> {
    infcx: infer::InferCtxt<'a>,
    locals: RefCell<NodeMap<ty::t>>,
    param_env: ty::ParameterEnvironment,

    // Temporary tables:
    node_types: RefCell<NodeMap<ty::t>>,
    item_substs: RefCell<NodeMap<ty::ItemSubsts>>,
    adjustments: RefCell<NodeMap<ty::AutoAdjustment>>,
    method_map: MethodMap,
    vtable_map: vtable_map,
    upvar_borrow_map: RefCell<ty::UpvarBorrowMap>,
}

#[deriving(Clone)]
pub struct FnStyleState {
    pub def: ast::NodeId,
    pub fn_style: ast::FnStyle,
    from_fn: bool
}

impl FnStyleState {
    pub fn function(fn_style: ast::FnStyle, def: ast::NodeId) -> FnStyleState {
        FnStyleState { def: def, fn_style: fn_style, from_fn: true }
    }

    pub fn recurse(&mut self, blk: &ast::Block) -> FnStyleState {
        match self.fn_style {
            // If this unsafe, then if the outer function was already marked as
            // unsafe we shouldn't attribute the unsafe'ness to the block. This
            // way the block can be warned about instead of ignoring this
            // extraneous block (functions are never warned about).
            ast::UnsafeFn if self.from_fn => *self,

            fn_style => {
                let (fn_style, def) = match blk.rules {
                    ast::UnsafeBlock(..) => (ast::UnsafeFn, blk.id),
                    ast::DefaultBlock => (fn_style, self.def),
                };
                FnStyleState{ def: def,
                             fn_style: fn_style,
                             from_fn: false }
            }
        }
    }
}

/// Whether `check_binop` is part of an assignment or not.
/// Used to know wether we allow user overloads and to print
/// better messages on error.
#[deriving(PartialEq)]
enum IsBinopAssignment{
    SimpleBinop,
    BinopAssignment,
}

#[deriving(Clone)]
pub struct FnCtxt<'a> {
    // This flag is set to true if, during the writeback phase, we encounter
    // a type error in this function.
    writeback_errors: Cell<bool>,

    // Number of errors that had been reported when we started
    // checking this function. On exit, if we find that *more* errors
    // have been reported, we will skip regionck and other work that
    // expects the types within the function to be consistent.
    err_count_on_creation: uint,

    ret_ty: ty::t,
    ps: RefCell<FnStyleState>,

    // Sometimes we generate region pointers where the precise region
    // to use is not known. For example, an expression like `&x.f`
    // where `x` is of type `@T`: in this case, we will be rooting
    // `x` onto the stack frame, and we could choose to root it until
    // the end of (almost) any enclosing block or expression.  We
    // want to pick the narrowest block that encompasses all uses.
    //
    // What we do in such cases is to generate a region variable with
    // `region_lb` as a lower bound.  The regionck pass then adds
    // other constraints based on how the variable is used and region
    // inference selects the ultimate value.  Finally, borrowck is
    // charged with guaranteeing that the value whose address was taken
    // can actually be made to live as long as it needs to live.
    region_lb: Cell<ast::NodeId>,

    inh: &'a Inherited<'a>,

    ccx: &'a CrateCtxt<'a>,
}

impl<'a> Inherited<'a> {
    fn new(tcx: &'a ty::ctxt,
           param_env: ty::ParameterEnvironment)
           -> Inherited<'a> {
        Inherited {
            infcx: infer::new_infer_ctxt(tcx),
            locals: RefCell::new(NodeMap::new()),
            param_env: param_env,
            node_types: RefCell::new(NodeMap::new()),
            item_substs: RefCell::new(NodeMap::new()),
            adjustments: RefCell::new(NodeMap::new()),
            method_map: RefCell::new(FnvHashMap::new()),
            vtable_map: RefCell::new(FnvHashMap::new()),
            upvar_borrow_map: RefCell::new(HashMap::new()),
        }
    }
}

// Used by check_const and check_enum_variants
fn blank_fn_ctxt<'a>(ccx: &'a CrateCtxt<'a>,
                     inh: &'a Inherited<'a>,
                     rty: ty::t,
                     region_bnd: ast::NodeId)
                     -> FnCtxt<'a> {
    FnCtxt {
        writeback_errors: Cell::new(false),
        err_count_on_creation: ccx.tcx.sess.err_count(),
        ret_ty: rty,
        ps: RefCell::new(FnStyleState::function(ast::NormalFn, 0)),
        region_lb: Cell::new(region_bnd),
        inh: inh,
        ccx: ccx
    }
}

fn blank_inherited_fields<'a>(ccx: &'a CrateCtxt<'a>) -> Inherited<'a> {
    // It's kind of a kludge to manufacture a fake function context
    // and statement context, but we might as well do write the code only once
    let param_env = ty::ParameterEnvironment {
        free_substs: subst::Substs::empty(),
        self_param_bound: None,
        type_param_bounds: Vec::new()
    };
    Inherited::new(ccx.tcx, param_env)
}

impl<'a> ExprTyProvider for FnCtxt<'a> {
    fn expr_ty(&self, ex: &ast::Expr) -> ty::t {
        self.expr_ty(ex)
    }

    fn ty_ctxt<'a>(&'a self) -> &'a ty::ctxt {
        self.ccx.tcx
    }
}

struct CheckItemTypesVisitor<'a> { ccx: &'a CrateCtxt<'a> }

impl<'a> Visitor<()> for CheckItemTypesVisitor<'a> {
    fn visit_item(&mut self, i: &ast::Item, _: ()) {
        check_item(self.ccx, i);
        visit::walk_item(self, i, ());
    }
}

struct CheckItemSizedTypesVisitor<'a> { ccx: &'a CrateCtxt<'a> }

impl<'a> Visitor<()> for CheckItemSizedTypesVisitor<'a> {
    fn visit_item(&mut self, i: &ast::Item, _: ()) {
        check_item_sized(self.ccx, i);
        visit::walk_item(self, i, ());
    }
}

pub fn check_item_types(ccx: &CrateCtxt, krate: &ast::Crate) {
    let mut visit = CheckItemTypesVisitor { ccx: ccx };
    visit::walk_crate(&mut visit, krate, ());

    ccx.tcx.sess.abort_if_errors();

    let mut visit = CheckItemSizedTypesVisitor { ccx: ccx };
    visit::walk_crate(&mut visit, krate, ());
}

fn check_bare_fn(ccx: &CrateCtxt,
                 decl: &ast::FnDecl,
                 body: &ast::Block,
                 id: ast::NodeId,
                 fty: ty::t,
                 param_env: ty::ParameterEnvironment) {
    // Compute the fty from point of view of inside fn
    // (replace any type-scheme with a type)
    let fty = fty.subst(ccx.tcx, &param_env.free_substs);

    match ty::get(fty).sty {
        ty::ty_bare_fn(ref fn_ty) => {
            let inh = Inherited::new(ccx.tcx, param_env);
            let fcx = check_fn(ccx, fn_ty.fn_style, &fn_ty.sig,
                               decl, id, body, &inh);

            vtable::resolve_in_block(&fcx, body);
            regionck::regionck_fn(&fcx, body);
            writeback::resolve_type_vars_in_fn(&fcx, decl, body);
        }
        _ => ccx.tcx.sess.impossible_case(body.span,
                                 "check_bare_fn: function type expected")
    }
}

struct GatherLocalsVisitor<'a> {
    fcx: &'a FnCtxt<'a>
}

impl<'a> GatherLocalsVisitor<'a> {
    fn assign(&mut self, nid: ast::NodeId, ty_opt: Option<ty::t>) {
            match ty_opt {
                None => {
                    // infer the variable's type
                    let var_id = self.fcx.infcx().next_ty_var_id();
                    let var_ty = ty::mk_var(self.fcx.tcx(), var_id);
                    self.fcx.inh.locals.borrow_mut().insert(nid, var_ty);
                }
                Some(typ) => {
                    // take type that the user specified
                    self.fcx.inh.locals.borrow_mut().insert(nid, typ);
                }
            }
    }
}

impl<'a> Visitor<()> for GatherLocalsVisitor<'a> {
    // Add explicitly-declared locals.
    fn visit_local(&mut self, local: &ast::Local, _: ()) {
        let o_ty = match local.ty.node {
            ast::TyInfer => None,
            _ => Some(self.fcx.to_ty(local.ty))
        };
        self.assign(local.id, o_ty);
        debug!("Local variable {} is assigned type {}",
               self.fcx.pat_to_str(local.pat),
               self.fcx.infcx().ty_to_str(
                   self.fcx.inh.locals.borrow().get_copy(&local.id)));
        visit::walk_local(self, local, ());
    }

    // Add pattern bindings.
    fn visit_pat(&mut self, p: &ast::Pat, _: ()) {
            match p.node {
              ast::PatIdent(_, ref path, _)
                  if pat_util::pat_is_binding(&self.fcx.ccx.tcx.def_map, p) => {
                self.assign(p.id, None);
                debug!("Pattern binding {} is assigned to {}",
                       token::get_ident(path.segments.get(0).identifier),
                       self.fcx.infcx().ty_to_str(
                           self.fcx.inh.locals.borrow().get_copy(&p.id)));
              }
              _ => {}
            }
            visit::walk_pat(self, p, ());

    }

    fn visit_block(&mut self, b: &ast::Block, _: ()) {
        // non-obvious: the `blk` variable maps to region lb, so
        // we have to keep this up-to-date.  This
        // is... unfortunate.  It'd be nice to not need this.
        self.fcx.with_region_lb(b.id, || visit::walk_block(self, b, ()));
    }

    // Don't descend into fns and items
    fn visit_fn(&mut self, _: &visit::FnKind, _: &ast::FnDecl,
                _: &ast::Block, _: Span, _: ast::NodeId, _: ()) { }
    fn visit_item(&mut self, _: &ast::Item, _: ()) { }

}

fn check_fn<'a>(ccx: &'a CrateCtxt<'a>,
                fn_style: ast::FnStyle,
                fn_sig: &ty::FnSig,
                decl: &ast::FnDecl,
                id: ast::NodeId,
                body: &ast::Block,
                inherited: &'a Inherited<'a>) -> FnCtxt<'a>
{
    /*!
     * Helper used by check_bare_fn and check_expr_fn.  Does the
     * grungy work of checking a function body and returns the
     * function context used for that purpose, since in the case of a
     * fn item there is still a bit more to do.
     *
     * - ...
     * - inherited: other fields inherited from the enclosing fn (if any)
     */

    let tcx = ccx.tcx;
    let err_count_on_creation = tcx.sess.err_count();

    // First, we have to replace any bound regions in the fn type with free ones.
    // The free region references will be bound the node_id of the body block.
    let (_, fn_sig) = replace_late_bound_regions_in_fn_sig(tcx, fn_sig, |br| {
        ty::ReFree(ty::FreeRegion {scope_id: body.id, bound_region: br})
    });

    relate_free_regions(tcx, &fn_sig);

    let arg_tys = fn_sig.inputs.as_slice();
    let ret_ty = fn_sig.output;

    debug!("check_fn(arg_tys={:?}, ret_ty={:?})",
           arg_tys.iter().map(|&a| ppaux::ty_to_str(tcx, a)).collect::<Vec<String>>(),
           ppaux::ty_to_str(tcx, ret_ty));

    // Create the function context.  This is either derived from scratch or,
    // in the case of function expressions, based on the outer context.
    let fcx = FnCtxt {
        writeback_errors: Cell::new(false),
        err_count_on_creation: err_count_on_creation,
        ret_ty: ret_ty,
        ps: RefCell::new(FnStyleState::function(fn_style, id)),
        region_lb: Cell::new(body.id),
        inh: inherited,
        ccx: ccx
    };

    {

        let mut visit = GatherLocalsVisitor { fcx: &fcx, };
        // Add formal parameters.
        for (arg_ty, input) in arg_tys.iter().zip(decl.inputs.iter()) {
            // Create type variables for each argument.
            pat_util::pat_bindings(&tcx.def_map,
                                   input.pat,
                                   |_bm, pat_id, _sp, _path| {
                                       visit.assign(pat_id, None);
                                   });

            // Check the pattern.
            let pcx = pat_ctxt {
                fcx: &fcx,
                map: pat_id_map(&tcx.def_map, input.pat),
            };
            _match::check_pat(&pcx, input.pat, *arg_ty);
        }

        visit.visit_block(body, ());
    }

    check_block_with_expected(&fcx, body, Some(ret_ty));

    // We unify the tail expr's type with the
    // function result type, if there is a tail expr.
    match body.expr {
        Some(tail_expr) => {
            // Special case: we print a special error if there appears
            // to be do-block/for-loop confusion
            demand::coerce_with_fn(&fcx, tail_expr.span,
                fcx.ret_ty, tail_expr,
                |sp, a, e, s| {
                    fcx.report_mismatched_return_types(sp, e, a, s);
                });
        }
        None => {}
    }

    for (input, arg) in decl.inputs.iter().zip(arg_tys.iter()) {
        fcx.write_ty(input.id, *arg);
    }

    fcx
}

fn span_for_field(tcx: &ty::ctxt, field: &ty::field_ty, struct_id: ast::DefId) -> Span {
    assert!(field.id.krate == ast::LOCAL_CRATE);
    let item = match tcx.map.find(struct_id.node) {
        Some(ast_map::NodeItem(item)) => item,
        None => fail!("node not in ast map: {}", struct_id.node),
        _ => fail!("expected item, found {}", tcx.map.node_to_str(struct_id.node))
    };

    match item.node {
        ast::ItemStruct(struct_def, _) => {
            match struct_def.fields.iter().find(|f| match f.node.kind {
                ast::NamedField(ident, _) => ident.name == field.name,
                _ => false,
            }) {
                Some(f) => f.span,
                None => {
                    tcx.sess
                       .bug(format!("Could not find field {}",
                                    token::get_name(field.name)).as_slice())
                }
            }
        },
        _ => tcx.sess.bug("Field found outside of a struct?"),
    }
}

// Check struct fields are uniquely named wrt parents.
fn check_for_field_shadowing(tcx: &ty::ctxt,
                             id: ast::DefId) {
    let struct_fields = tcx.struct_fields.borrow();
    let fields = struct_fields.get(&id);

    let superstructs = tcx.superstructs.borrow();
    let super_struct = superstructs.get(&id);
    match *super_struct {
        Some(parent_id) => {
            let super_fields = ty::lookup_struct_fields(tcx, parent_id);
            for f in fields.iter() {
                match super_fields.iter().find(|sf| f.name == sf.name) {
                    Some(prev_field) => {
                        tcx.sess.span_err(span_for_field(tcx, f, id),
                            format!("field `{}` hides field declared in \
                                     super-struct",
                                    token::get_name(f.name)).as_slice());
                        tcx.sess.span_note(span_for_field(tcx, prev_field, parent_id),
                            "previously declared here");
                    },
                    None => {}
                }
            }
        },
        None => {}
    }
}

fn check_fields_sized(tcx: &ty::ctxt,
                      struct_def: &ast::StructDef) {
    let len = struct_def.fields.len();
    if len == 0 {
        return;
    }
    for f in struct_def.fields.slice_to(len - 1).iter() {
        let t = ty::node_id_to_type(tcx, f.node.id);
        if !ty::type_is_sized(tcx, t) {
            match f.node.kind {
                ast::NamedField(ident, _) => {
                    tcx.sess.span_err(
                        f.span,
                        format!("type `{}` is dynamically sized. \
                                 dynamically sized types may only \
                                 appear as the type of the final \
                                 field in a struct",
                                 token::get_ident(ident)).as_slice());
                }
                ast::UnnamedField(_) => {
                    tcx.sess.span_err(f.span, "dynamically sized type in field");
                }
            }
        }
    }
}

pub fn check_struct(ccx: &CrateCtxt, id: ast::NodeId, span: Span) {
    let tcx = ccx.tcx;

    check_representable(tcx, span, id, "struct");
    check_instantiable(tcx, span, id);

    // Check there are no overlapping fields in super-structs
    check_for_field_shadowing(tcx, local_def(id));

    if ty::lookup_simd(tcx, local_def(id)) {
        check_simd(tcx, span, id);
    }
}

pub fn check_item_sized(ccx: &CrateCtxt, it: &ast::Item) {
    debug!("check_item(it.id={}, it.ident={})",
           it.id,
           ty::item_path_str(ccx.tcx, local_def(it.id)));
    let _indenter = indenter();

    match it.node {
        ast::ItemEnum(ref enum_definition, _) => {
            check_enum_variants_sized(ccx,
                                      enum_definition.variants.as_slice());
        }
        ast::ItemStruct(..) => {
            check_fields_sized(ccx.tcx, ccx.tcx.map.expect_struct(it.id));
        }
        _ => {}
    }
}

pub fn check_item(ccx: &CrateCtxt, it: &ast::Item) {
    debug!("check_item(it.id={}, it.ident={})",
           it.id,
           ty::item_path_str(ccx.tcx, local_def(it.id)));
    let _indenter = indenter();

    match it.node {
      ast::ItemStatic(_, _, e) => check_const(ccx, it.span, e, it.id),
      ast::ItemEnum(ref enum_definition, _) => {
        check_enum_variants(ccx,
                            it.span,
                            enum_definition.variants.as_slice(),
                            it.id);
      }
      ast::ItemFn(decl, _, _, _, body) => {
        let fn_tpt = ty::lookup_item_type(ccx.tcx, ast_util::local_def(it.id));

        let param_env = ty::construct_parameter_environment(
                ccx.tcx,
                None,
                fn_tpt.generics.type_param_defs(),
                [],
                [],
                fn_tpt.generics.region_param_defs.as_slice(),
                body.id);

        check_bare_fn(ccx, decl, body, it.id, fn_tpt.ty, param_env);
      }
      ast::ItemImpl(_, ref opt_trait_ref, _, ref ms) => {
        debug!("ItemImpl {} with id {}", token::get_ident(it.ident), it.id);

        let impl_tpt = ty::lookup_item_type(ccx.tcx, ast_util::local_def(it.id));
        for m in ms.iter() {
            check_method_body(ccx, &impl_tpt.generics, None, *m);
        }

        match *opt_trait_ref {
            Some(ref ast_trait_ref) => {
                let impl_trait_ref =
                    ty::node_id_to_trait_ref(ccx.tcx, ast_trait_ref.ref_id);
                check_impl_methods_against_trait(ccx,
                                             it.span,
                                             &impl_tpt.generics,
                                             ast_trait_ref,
                                             &*impl_trait_ref,
                                             ms.as_slice());
                vtable::resolve_impl(ccx.tcx, it, &impl_tpt.generics, &*impl_trait_ref);
            }
            None => { }
        }

      }
      ast::ItemTrait(_, _, _, ref trait_methods) => {
        let trait_def = ty::lookup_trait_def(ccx.tcx, local_def(it.id));
        for trait_method in (*trait_methods).iter() {
            match *trait_method {
                Required(..) => {
                    // Nothing to do, since required methods don't have
                    // bodies to check.
                }
                Provided(m) => {
                    check_method_body(ccx, &trait_def.generics,
                                      Some(trait_def.trait_ref.clone()), m);
                }
            }
        }
      }
      ast::ItemStruct(..) => {
        check_struct(ccx, it.id, it.span);
      }
      ast::ItemTy(ref t, ref generics) => {
        let tpt_ty = ty::node_id_to_type(ccx.tcx, it.id);
        check_bounds_are_used(ccx, t.span, &generics.ty_params, tpt_ty);
      }
      ast::ItemForeignMod(ref m) => {
        if m.abi == abi::RustIntrinsic {
            for item in m.items.iter() {
                check_intrinsic_type(ccx, *item);
            }
        } else {
            for item in m.items.iter() {
                let tpt = ty::lookup_item_type(ccx.tcx, local_def(item.id));
                if tpt.generics.has_type_params() {
                    ccx.tcx.sess.span_err(item.span, "foreign items may not have type parameters");
                }

                match item.node {
                    ast::ForeignItemFn(ref fn_decl, _) => {
                        if fn_decl.variadic && m.abi != abi::C {
                            ccx.tcx.sess.span_err(
                                item.span, "variadic function must have C calling convention");
                        }
                    }
                    _ => {}
                }
            }
        }
      }
      _ => {/* nothing to do */ }
    }
}

fn check_method_body(ccx: &CrateCtxt,
                     item_generics: &ty::Generics,
                     self_bound: Option<Rc<ty::TraitRef>>,
                     method: &ast::Method) {
    /*!
     * Type checks a method body.
     *
     * # Parameters
     * - `item_generics`: generics defined on the impl/trait that contains
     *   the method
     * - `self_bound`: bound for the `Self` type parameter, if any
     * - `method`: the method definition
     */

    debug!("check_method_body(item_generics={}, \
            self_bound={}, \
            method.id={})",
            item_generics.repr(ccx.tcx),
            self_bound.repr(ccx.tcx),
            method.id);
    let method_def_id = local_def(method.id);
    let method_ty = ty::method(ccx.tcx, method_def_id);
    let method_generics = &method_ty.generics;

    let param_env =
        ty::construct_parameter_environment(
            ccx.tcx,
            self_bound,
            item_generics.type_param_defs(),
            method_generics.type_param_defs(),
            item_generics.region_param_defs(),
            method_generics.region_param_defs(),
            method.body.id);

    let fty = ty::node_id_to_type(ccx.tcx, method.id);

    check_bare_fn(ccx, method.decl, method.body, method.id, fty, param_env);
}

fn check_impl_methods_against_trait(ccx: &CrateCtxt,
                                    impl_span: Span,
                                    impl_generics: &ty::Generics,
                                    ast_trait_ref: &ast::TraitRef,
                                    impl_trait_ref: &ty::TraitRef,
                                    impl_methods: &[@ast::Method]) {
    // Locate trait methods
    let tcx = ccx.tcx;
    let trait_methods = ty::trait_methods(tcx, impl_trait_ref.def_id);

    // Check existing impl methods to see if they are both present in trait
    // and compatible with trait signature
    for impl_method in impl_methods.iter() {
        let impl_method_def_id = local_def(impl_method.id);
        let impl_method_ty = ty::method(ccx.tcx, impl_method_def_id);

        // If this is an impl of a trait method, find the corresponding
        // method definition in the trait.
        let opt_trait_method_ty =
            trait_methods.iter().
            find(|tm| tm.ident.name == impl_method_ty.ident.name);
        match opt_trait_method_ty {
            Some(trait_method_ty) => {
                compare_impl_method(ccx.tcx,
                                    impl_generics,
                                    &*impl_method_ty,
                                    impl_method.span,
                                    impl_method.body.id,
                                    &**trait_method_ty,
                                    &impl_trait_ref.substs);
            }
            None => {
                tcx.sess.span_err(
                    impl_method.span,
                    format!(
                        "method `{}` is not a member of trait `{}`",
                        token::get_ident(impl_method_ty.ident),
                        pprust::path_to_str(&ast_trait_ref.path)).as_slice());
            }
        }
    }

    // Check for missing methods from trait
    let provided_methods = ty::provided_trait_methods(tcx,
                                                      impl_trait_ref.def_id);
    let mut missing_methods = Vec::new();
    for trait_method in trait_methods.iter() {
        let is_implemented =
            impl_methods.iter().any(
                |m| m.ident.name == trait_method.ident.name);
        let is_provided =
            provided_methods.iter().any(
                |m| m.ident.name == trait_method.ident.name);
        if !is_implemented && !is_provided {
            missing_methods.push(
                format!("`{}`", token::get_ident(trait_method.ident)));
        }
    }

    if !missing_methods.is_empty() {
        tcx.sess.span_err(
            impl_span,
            format!("not all trait methods implemented, missing: {}",
                    missing_methods.connect(", ")).as_slice());
    }
}

/**
 * Checks that a method from an impl/class conforms to the signature of
 * the same method as declared in the trait.
 *
 * # Parameters
 *
 * - impl_generics: the generics declared on the impl itself (not the method!)
 * - impl_m: type of the method we are checking
 * - impl_m_span: span to use for reporting errors
 * - impl_m_body_id: id of the method body
 * - trait_m: the method in the trait
 * - trait_substs: the substitutions used on the type of the trait
 */
fn compare_impl_method(tcx: &ty::ctxt,
                       impl_generics: &ty::Generics,
                       impl_m: &ty::Method,
                       impl_m_span: Span,
                       impl_m_body_id: ast::NodeId,
                       trait_m: &ty::Method,
                       trait_substs: &subst::Substs) {
    debug!("compare_impl_method()");
    let infcx = infer::new_infer_ctxt(tcx);

    let impl_tps = impl_generics.type_param_defs().len();

    // Try to give more informative error messages about self typing
    // mismatches.  Note that any mismatch will also be detected
    // below, where we construct a canonical function type that
    // includes the self parameter as a normal parameter.  It's just
    // that the error messages you get out of this code are a bit more
    // inscrutable, particularly for cases where one method has no
    // self.
    match (&trait_m.explicit_self, &impl_m.explicit_self) {
        (&ast::SelfStatic, &ast::SelfStatic) => {}
        (&ast::SelfStatic, _) => {
            tcx.sess.span_err(
                impl_m_span,
                format!("method `{}` has a `{}` declaration in the impl, \
                        but not in the trait",
                        token::get_ident(trait_m.ident),
                        pprust::explicit_self_to_str(
                            impl_m.explicit_self)).as_slice());
            return;
        }
        (_, &ast::SelfStatic) => {
            tcx.sess.span_err(
                impl_m_span,
                format!("method `{}` has a `{}` declaration in the trait, \
                        but not in the impl",
                        token::get_ident(trait_m.ident),
                        pprust::explicit_self_to_str(
                            trait_m.explicit_self)).as_slice());
            return;
        }
        _ => {
            // Let the type checker catch other errors below
        }
    }

    let num_impl_m_type_params = impl_m.generics.type_param_defs().len();
    let num_trait_m_type_params = trait_m.generics.type_param_defs().len();
    if num_impl_m_type_params != num_trait_m_type_params {
        tcx.sess.span_err(
            impl_m_span,
            format!("method `{method}` has {nimpl, plural, =1{# type parameter} \
                                                        other{# type parameters}}, \
                     but its trait declaration has {ntrait, plural, =1{# type parameter} \
                                                                 other{# type parameters}}",
                    method = token::get_ident(trait_m.ident),
                    nimpl = num_impl_m_type_params,
                    ntrait = num_trait_m_type_params).as_slice());
        return;
    }

    if impl_m.fty.sig.inputs.len() != trait_m.fty.sig.inputs.len() {
        tcx.sess.span_err(
            impl_m_span,
            format!("method `{method}` has {nimpl, plural, =1{# parameter} \
                                                        other{# parameters}} \
                     but the declaration in trait `{trait}` has {ntrait}",
                 method = token::get_ident(trait_m.ident),
                 nimpl = impl_m.fty.sig.inputs.len(),
                 trait = ty::item_path_str(tcx, trait_m.def_id),
                 ntrait = trait_m.fty.sig.inputs.len()).as_slice());
        return;
    }

    let it = trait_m.generics.type_param_defs().iter()
        .zip(impl_m.generics.type_param_defs().iter());

    for (i, (trait_param_def, impl_param_def)) in it.enumerate() {
        // Check that the impl does not require any builtin-bounds
        // that the trait does not guarantee:
        let extra_bounds =
            impl_param_def.bounds.builtin_bounds -
            trait_param_def.bounds.builtin_bounds;
        if !extra_bounds.is_empty() {
           tcx.sess.span_err(
               impl_m_span,
               format!("in method `{}`, \
                       type parameter {} requires `{}`, \
                       which is not required by \
                       the corresponding type parameter \
                       in the trait declaration",
                       token::get_ident(trait_m.ident),
                       i,
                       extra_bounds.user_string(tcx)).as_slice());
           return;
        }

        // FIXME(#2687)---we should be checking that the bounds of the
        // trait imply the bounds of the subtype, but it appears we
        // are...not checking this.
        if impl_param_def.bounds.trait_bounds.len() !=
            trait_param_def.bounds.trait_bounds.len()
        {
            tcx.sess.span_err(
                impl_m_span,
                format!("in method `{method}`, \
                        type parameter {typaram} has \
                        {nimpl, plural, =1{# trait bound} other{# trait bounds}}, \
                        but the corresponding type parameter in \
                        the trait declaration has \
                        {ntrait, plural, =1{# trait bound} other{# trait bounds}}",
                        method = token::get_ident(trait_m.ident),
                        typaram = i,
                        nimpl = impl_param_def.bounds.trait_bounds.len(),
                        ntrait = trait_param_def.bounds
                                                .trait_bounds
                                                .len()).as_slice());
            return;
        }
    }

    // Create a substitution that maps the type parameters on the impl
    // to themselves and which replace any references to bound regions
    // in the self type with free regions.  So, for example, if the
    // impl type is "&'a str", then this would replace the self
    // type with a free region `self`.
    let dummy_impl_tps: Vec<ty::t> =
        impl_generics.type_param_defs().iter().enumerate().
        map(|(i,t)| ty::mk_param(tcx, i, t.def_id)).
        collect();
    let dummy_method_tps: Vec<ty::t> =
        impl_m.generics.type_param_defs().iter().enumerate().
        map(|(i,t)| ty::mk_param(tcx, i + impl_tps, t.def_id)).
        collect();
    let dummy_impl_regions: Vec<ty::Region> =
        impl_generics.region_param_defs().iter().
        map(|l| ty::ReFree(ty::FreeRegion {
                scope_id: impl_m_body_id,
                bound_region: ty::BrNamed(l.def_id, l.name)})).
        collect();
    let dummy_substs = subst::Substs {
        tps: dummy_impl_tps.append(dummy_method_tps.as_slice()),
        regions: subst::NonerasedRegions(dummy_impl_regions),
        self_ty: None };

    // Create a bare fn type for trait/impl
    // It'd be nice to refactor so as to provide the bare fn types instead.
    let trait_fty = ty::mk_bare_fn(tcx, trait_m.fty.clone());
    let impl_fty = ty::mk_bare_fn(tcx, impl_m.fty.clone());

    // Perform substitutions so that the trait/impl methods are expressed
    // in terms of the same set of type/region parameters:
    // - replace trait type parameters with those from `trait_substs`,
    //   except with any reference to bound self replaced with `dummy_self_r`
    // - replace method parameters on the trait with fresh, dummy parameters
    //   that correspond to the parameters we will find on the impl
    // - replace self region with a fresh, dummy region
    let impl_fty = {
        debug!("impl_fty (pre-subst): {}", ppaux::ty_to_str(tcx, impl_fty));
        impl_fty.subst(tcx, &dummy_substs)
    };
    debug!("impl_fty (post-subst): {}", ppaux::ty_to_str(tcx, impl_fty));
    let trait_fty = {
        let subst::Substs { regions: trait_regions,
                            tps: trait_tps,
                            self_ty: self_ty } = trait_substs.subst(tcx, &dummy_substs);
        let substs = subst::Substs {
            regions: trait_regions,
            tps: trait_tps.append(dummy_method_tps.as_slice()),
            self_ty: self_ty,
        };
        debug!("trait_fty (pre-subst): {} substs={}",
               trait_fty.repr(tcx), substs.repr(tcx));
        trait_fty.subst(tcx, &substs)
    };
    debug!("trait_fty (post-subst): {}", trait_fty.repr(tcx));

    match infer::mk_subty(&infcx, false, infer::MethodCompatCheck(impl_m_span),
                          impl_fty, trait_fty) {
        Ok(()) => {}
        Err(ref terr) => {
            tcx.sess.span_err(
                impl_m_span,
                format!("method `{}` has an incompatible type for trait: {}",
                        token::get_ident(trait_m.ident),
                        ty::type_err_to_str(tcx, terr)).as_slice());
            ty::note_and_explain_type_err(tcx, terr);
        }
    }
}

impl<'a> AstConv for FnCtxt<'a> {
    fn tcx<'a>(&'a self) -> &'a ty::ctxt { self.ccx.tcx }

    fn get_item_ty(&self, id: ast::DefId) -> ty::ty_param_bounds_and_ty {
        ty::lookup_item_type(self.tcx(), id)
    }

    fn get_trait_def(&self, id: ast::DefId) -> Rc<ty::TraitDef> {
        ty::lookup_trait_def(self.tcx(), id)
    }

    fn ty_infer(&self, _span: Span) -> ty::t {
        self.infcx().next_ty_var()
    }
}

impl<'a> FnCtxt<'a> {
    pub fn infcx<'b>(&'b self) -> &'b infer::InferCtxt<'a> {
        &self.inh.infcx
    }

    pub fn err_count_since_creation(&self) -> uint {
        self.ccx.tcx.sess.err_count() - self.err_count_on_creation
    }

    pub fn vtable_context<'a>(&'a self) -> VtableContext<'a> {
        VtableContext {
            infcx: self.infcx(),
            param_env: &self.inh.param_env
        }
    }
}

impl<'a> RegionScope for infer::InferCtxt<'a> {
    fn anon_regions(&self, span: Span, count: uint)
                    -> Result<Vec<ty::Region> , ()> {
        Ok(Vec::from_fn(count, |_| {
            self.next_region_var(infer::MiscVariable(span))
        }))
    }
}

impl<'a> FnCtxt<'a> {
    pub fn tag(&self) -> String {
        format!("{}", self as *FnCtxt)
    }

    pub fn local_ty(&self, span: Span, nid: ast::NodeId) -> ty::t {
        match self.inh.locals.borrow().find(&nid) {
            Some(&t) => t,
            None => {
                self.tcx().sess.span_bug(
                    span,
                    format!("no type for local variable {:?}",
                            nid).as_slice());
            }
        }
    }

    #[inline]
    pub fn write_ty(&self, node_id: ast::NodeId, ty: ty::t) {
        debug!("write_ty({}, {}) in fcx {}",
               node_id, ppaux::ty_to_str(self.tcx(), ty), self.tag());
        self.inh.node_types.borrow_mut().insert(node_id, ty);
    }

    pub fn write_substs(&self, node_id: ast::NodeId, substs: ty::ItemSubsts) {
        if !substs.substs.is_noop() {
            debug!("write_substs({}, {}) in fcx {}",
                   node_id,
                   substs.repr(self.tcx()),
                   self.tag());

            self.inh.item_substs.borrow_mut().insert(node_id, substs);
        }
    }

    pub fn write_ty_substs(&self,
                           node_id: ast::NodeId,
                           ty: ty::t,
                           substs: ty::ItemSubsts) {
        let ty = ty.subst(self.tcx(), &substs.substs);
        self.write_ty(node_id, ty);
        self.write_substs(node_id, substs);
    }

    pub fn write_autoderef_adjustment(&self,
                                      node_id: ast::NodeId,
                                      derefs: uint) {
        if derefs == 0 { return; }
        self.write_adjustment(
            node_id,
            ty::AutoDerefRef(ty::AutoDerefRef {
                autoderefs: derefs,
                autoref: None })
        );
    }

    pub fn write_adjustment(&self,
                            node_id: ast::NodeId,
                            adj: ty::AutoAdjustment) {
        debug!("write_adjustment(node_id={:?}, adj={:?})", node_id, adj);
        self.inh.adjustments.borrow_mut().insert(node_id, adj);
    }

    pub fn write_nil(&self, node_id: ast::NodeId) {
        self.write_ty(node_id, ty::mk_nil());
    }
    pub fn write_bot(&self, node_id: ast::NodeId) {
        self.write_ty(node_id, ty::mk_bot());
    }
    pub fn write_error(&self, node_id: ast::NodeId) {
        self.write_ty(node_id, ty::mk_err());
    }

    pub fn to_ty(&self, ast_t: &ast::Ty) -> ty::t {
        ast_ty_to_ty(self, self.infcx(), ast_t)
    }

    pub fn pat_to_str(&self, pat: &ast::Pat) -> String {
        pat.repr(self.tcx())
    }

    pub fn expr_ty(&self, ex: &ast::Expr) -> ty::t {
        match self.inh.node_types.borrow().find(&ex.id) {
            Some(&t) => t,
            None => {
                self.tcx().sess.bug(format!("no type for expr in fcx {}",
                                            self.tag()).as_slice());
            }
        }
    }

    pub fn node_ty(&self, id: ast::NodeId) -> ty::t {
        match self.inh.node_types.borrow().find(&id) {
            Some(&t) => t,
            None => {
                self.tcx().sess.bug(
                    format!("no type for node {}: {} in fcx {}",
                            id, self.tcx().map.node_to_str(id),
                            self.tag()).as_slice());
            }
        }
    }

    pub fn method_ty_substs(&self, id: ast::NodeId) -> subst::Substs {
        match self.inh.method_map.borrow().find(&MethodCall::expr(id)) {
            Some(method) => method.substs.clone(),
            None => {
                self.tcx().sess.bug(
                    format!("no method entry for node {}: {} in fcx {}",
                            id, self.tcx().map.node_to_str(id),
                            self.tag()).as_slice());
            }
        }
    }

    pub fn opt_node_ty_substs(&self,
                              id: ast::NodeId,
                              f: |&ty::ItemSubsts|) {
        match self.inh.item_substs.borrow().find(&id) {
            Some(s) => { f(s) }
            None => { }
        }
    }

    pub fn mk_subty(&self,
                    a_is_expected: bool,
                    origin: infer::TypeOrigin,
                    sub: ty::t,
                    sup: ty::t)
                    -> Result<(), ty::type_err> {
        infer::mk_subty(self.infcx(), a_is_expected, origin, sub, sup)
    }

    pub fn can_mk_subty(&self, sub: ty::t, sup: ty::t)
                        -> Result<(), ty::type_err> {
        infer::can_mk_subty(self.infcx(), sub, sup)
    }

    pub fn mk_assignty(&self,
                       expr: &ast::Expr,
                       sub: ty::t,
                       sup: ty::t)
                       -> Result<(), ty::type_err> {
        match infer::mk_coercety(self.infcx(),
                                 false,
                                 infer::ExprAssignable(expr.span),
                                 sub,
                                 sup) {
            Ok(None) => Ok(()),
            Err(ref e) => Err((*e)),
            Ok(Some(adjustment)) => {
                self.write_adjustment(expr.id, adjustment);
                Ok(())
            }
        }
    }

    pub fn mk_eqty(&self,
                   a_is_expected: bool,
                   origin: infer::TypeOrigin,
                   sub: ty::t,
                   sup: ty::t)
                   -> Result<(), ty::type_err> {
        infer::mk_eqty(self.infcx(), a_is_expected, origin, sub, sup)
    }

    pub fn mk_subr(&self,
                   a_is_expected: bool,
                   origin: infer::SubregionOrigin,
                   sub: ty::Region,
                   sup: ty::Region) {
        infer::mk_subr(self.infcx(), a_is_expected, origin, sub, sup)
    }

    pub fn with_region_lb<R>(&self, lb: ast::NodeId, f: || -> R) -> R {
        let old_region_lb = self.region_lb.get();
        self.region_lb.set(lb);
        let v = f();
        self.region_lb.set(old_region_lb);
        v
    }

    pub fn type_error_message(&self,
                              sp: Span,
                              mk_msg: |String| -> String,
                              actual_ty: ty::t,
                              err: Option<&ty::type_err>) {
        self.infcx().type_error_message(sp, mk_msg, actual_ty, err);
    }

    pub fn report_mismatched_return_types(&self,
                                          sp: Span,
                                          e: ty::t,
                                          a: ty::t,
                                          err: &ty::type_err) {
        // Derived error
        if ty::type_is_error(e) || ty::type_is_error(a) {
            return;
        }
        self.infcx().report_mismatched_types(sp, e, a, err)
    }

    pub fn report_mismatched_types(&self,
                                   sp: Span,
                                   e: ty::t,
                                   a: ty::t,
                                   err: &ty::type_err) {
        self.infcx().report_mismatched_types(sp, e, a, err)
    }
}

pub enum LvaluePreference {
    PreferMutLvalue,
    NoPreference
}

pub fn autoderef<T>(fcx: &FnCtxt, sp: Span, base_ty: ty::t,
                    expr_id: Option<ast::NodeId>,
                    mut lvalue_pref: LvaluePreference,
                    should_stop: |ty::t, uint| -> Option<T>)
                    -> (ty::t, uint, Option<T>) {
    /*!
     * Executes an autoderef loop for the type `t`. At each step, invokes
     * `should_stop` to decide whether to terminate the loop. Returns
     * the final type and number of derefs that it performed.
     *
     * Note: this method does not modify the adjustments table. The caller is
     * responsible for inserting an AutoAdjustment record into the `fcx`
     * using one of the suitable methods.
     */

    let mut t = base_ty;
    for autoderefs in range(0, fcx.tcx().sess.recursion_limit.get()) {
        let resolved_t = structurally_resolved_type(fcx, sp, t);

        match should_stop(resolved_t, autoderefs) {
            Some(x) => return (resolved_t, autoderefs, Some(x)),
            None => {}
        }

        // Otherwise, deref if type is derefable:
        let mt = match ty::deref(resolved_t, false) {
            Some(mt) => Some(mt),
            None => {
                let method_call =
                    expr_id.map(|id| MethodCall::autoderef(id, autoderefs as u32));
                try_overloaded_deref(fcx, sp, method_call, None, resolved_t, lvalue_pref)
            }
        };
        match mt {
            Some(mt) => {
                t = mt.ty;
                if mt.mutbl == ast::MutImmutable {
                    lvalue_pref = NoPreference;
                }
            }
            None => return (resolved_t, autoderefs, None)
        }
    }

    // We've reached the recursion limit, error gracefully.
    fcx.tcx().sess.span_err(sp,
        format!("reached the recursion limit while auto-dereferencing {}",
                base_ty.repr(fcx.tcx())).as_slice());
    (ty::mk_err(), 0, None)
}

fn try_overloaded_deref(fcx: &FnCtxt,
                        span: Span,
                        method_call: Option<MethodCall>,
                        base_expr: Option<&ast::Expr>,
                        base_ty: ty::t,
                        lvalue_pref: LvaluePreference)
                        -> Option<ty::mt> {
    // Try DerefMut first, if preferred.
    let method = match (lvalue_pref, fcx.tcx().lang_items.deref_mut_trait()) {
        (PreferMutLvalue, Some(trait_did)) => {
            method::lookup_in_trait(fcx, span, base_expr.map(|x| &*x),
                                    token::intern("deref_mut"), trait_did,
                                    base_ty, [], DontAutoderefReceiver, IgnoreStaticMethods)
        }
        _ => None
    };

    // Otherwise, fall back to Deref.
    let method = match (method, fcx.tcx().lang_items.deref_trait()) {
        (None, Some(trait_did)) => {
            method::lookup_in_trait(fcx, span, base_expr.map(|x| &*x),
                                    token::intern("deref"), trait_did,
                                    base_ty, [], DontAutoderefReceiver, IgnoreStaticMethods)
        }
        (method, _) => method
    };

    match method {
        Some(method) => {
            let ref_ty = ty::ty_fn_ret(method.ty);
            match method_call {
                Some(method_call) => {
                    fcx.inh.method_map.borrow_mut().insert(method_call, method);
                }
                None => {}
            }
            ty::deref(ref_ty, true)
        }
        None => None
    }
}

// AST fragment checking
pub fn check_lit(fcx: &FnCtxt, lit: &ast::Lit) -> ty::t {
    let tcx = fcx.ccx.tcx;

    match lit.node {
        ast::LitStr(..) => ty::mk_str_slice(tcx, ty::ReStatic, ast::MutImmutable),
        ast::LitBinary(..) => {
            ty::mk_slice(tcx, ty::ReStatic, ty::mt{ ty: ty::mk_u8(), mutbl: ast::MutImmutable })
        }
        ast::LitChar(_) => ty::mk_char(),
        ast::LitInt(_, t) => ty::mk_mach_int(t),
        ast::LitUint(_, t) => ty::mk_mach_uint(t),
        ast::LitIntUnsuffixed(_) => {
            // An unsuffixed integer literal could have any integral type,
            // so we create an integral type variable for it.
            ty::mk_int_var(tcx, fcx.infcx().next_int_var_id())
        }
        ast::LitFloat(_, t) => ty::mk_mach_float(t),
        ast::LitFloatUnsuffixed(_) => {
            // An unsuffixed floating point literal could have any floating point
            // type, so we create a floating point type variable for it.
            ty::mk_float_var(tcx, fcx.infcx().next_float_var_id())
        }
        ast::LitNil => ty::mk_nil(),
        ast::LitBool(_) => ty::mk_bool()
    }
}

pub fn valid_range_bounds(ccx: &CrateCtxt,
                          from: &ast::Expr,
                          to: &ast::Expr)
                       -> Option<bool> {
    match const_eval::compare_lit_exprs(ccx.tcx, from, to) {
        Some(val) => Some(val <= 0),
        None => None
    }
}

pub fn check_expr_has_type(
    fcx: &FnCtxt, expr: &ast::Expr,
    expected: ty::t) {
    check_expr_with_unifier(fcx, expr, Some(expected), NoPreference, || {
        demand::suptype(fcx, expr.span, expected, fcx.expr_ty(expr));
    });
}

fn check_expr_coercable_to_type(fcx: &FnCtxt, expr: &ast::Expr, expected: ty::t) {
    check_expr_with_unifier(fcx, expr, Some(expected), NoPreference, || {
        demand::coerce(fcx, expr.span, expected, expr)
    });
}

fn check_expr_with_hint(fcx: &FnCtxt, expr: &ast::Expr, expected: ty::t) {
    check_expr_with_unifier(fcx, expr, Some(expected), NoPreference, || ())
}

fn check_expr_with_opt_hint(fcx: &FnCtxt, expr: &ast::Expr,
                            expected: Option<ty::t>)  {
    check_expr_with_unifier(fcx, expr, expected, NoPreference, || ())
}

fn check_expr_with_opt_hint_and_lvalue_pref(fcx: &FnCtxt,
                                            expr: &ast::Expr,
                                            expected: Option<ty::t>,
                                            lvalue_pref: LvaluePreference) {
    check_expr_with_unifier(fcx, expr, expected, lvalue_pref, || ())
}

fn check_expr(fcx: &FnCtxt, expr: &ast::Expr)  {
    check_expr_with_unifier(fcx, expr, None, NoPreference, || ())
}

fn check_expr_with_lvalue_pref(fcx: &FnCtxt, expr: &ast::Expr,
                               lvalue_pref: LvaluePreference)  {
    check_expr_with_unifier(fcx, expr, None, lvalue_pref, || ())
}


// determine the `self` type, using fresh variables for all variables
// declared on the impl declaration e.g., `impl<A,B> for ~[(A,B)]`
// would return ($0, $1) where $0 and $1 are freshly instantiated type
// variables.
pub fn impl_self_ty(vcx: &VtableContext,
                    span: Span, // (potential) receiver for this impl
                    did: ast::DefId)
                 -> ty_param_substs_and_ty {
    let tcx = vcx.tcx();

    let ity = ty::lookup_item_type(tcx, did);
    let (n_tps, rps, raw_ty) =
        (ity.generics.type_param_defs().len(),
         ity.generics.region_param_defs(),
         ity.ty);

    let rps = vcx.infcx.region_vars_for_defs(span, rps);
    let tps = vcx.infcx.next_ty_vars(n_tps);

    let substs = subst::Substs {
        regions: subst::NonerasedRegions(rps),
        self_ty: None,
        tps: tps,
    };
    let substd_ty = raw_ty.subst(tcx, &substs);

    ty_param_substs_and_ty { substs: substs, ty: substd_ty }
}

// Only for fields! Returns <none> for methods>
// Indifferent to privacy flags
pub fn lookup_field_ty(tcx: &ty::ctxt,
                       class_id: ast::DefId,
                       items: &[ty::field_ty],
                       fieldname: ast::Name,
                       substs: &subst::Substs) -> Option<ty::t> {

    let o_field = items.iter().find(|f| f.name == fieldname);
    o_field.map(|f| ty::lookup_field_type(tcx, class_id, f.id, substs))
}

// Controls whether the arguments are automatically referenced. This is useful
// for overloaded binary and unary operators.
pub enum DerefArgs {
    DontDerefArgs,
    DoDerefArgs
}

// Given the provenance of a static method, returns the generics of the static
// method's container.
fn generics_of_static_method_container(type_context: &ty::ctxt,
                                       provenance: def::MethodProvenance)
                                       -> ty::Generics {
    match provenance {
        def::FromTrait(trait_def_id) => {
            ty::lookup_trait_def(type_context, trait_def_id).generics.clone()
        }
        def::FromImpl(impl_def_id) => {
            ty::lookup_item_type(type_context, impl_def_id).generics.clone()
        }
    }
}

// Verifies that type parameters supplied in paths are in the right
// locations.
fn check_type_parameter_positions_in_path(function_context: &FnCtxt,
                                          path: &ast::Path,
                                          def: def::Def) {
    // We only care about checking the case in which the path has two or
    // more segments.
    if path.segments.len() < 2 {
        return
    }

    // Verify that no lifetimes or type parameters are present anywhere
    // except the final two elements of the path.
    for i in range(0, path.segments.len() - 2) {
        for lifetime in path.segments.get(i).lifetimes.iter() {
            function_context.tcx()
                .sess
                .span_err(lifetime.span,
                          "lifetime parameters may not \
                          appear here");
            break;
        }

        for typ in path.segments.get(i).types.iter() {
            function_context.tcx()
                            .sess
                            .span_err(typ.span,
                                      "type parameters may not appear here");
            break;
        }
    }

    // If there are no parameters at all, there is nothing more to do; the
    // rest of typechecking will (attempt to) infer everything.
    if path.segments
           .iter()
           .all(|s| s.lifetimes.is_empty() && s.types.is_empty()) {
        return
    }

    match def {
        // If this is a static method of a trait or implementation, then
        // ensure that the segment of the path which names the trait or
        // implementation (the penultimate segment) is annotated with the
        // right number of type parameters.
        def::DefStaticMethod(_, provenance, _) => {
            let generics =
                generics_of_static_method_container(function_context.ccx.tcx,
                                                    provenance);
            let name = match provenance {
                def::FromTrait(_) => "trait",
                def::FromImpl(_) => "impl",
            };

            let trait_segment = &path.segments.get(path.segments.len() - 2);

            // Make sure lifetime parameterization agrees with the trait or
            // implementation type.
            let trait_region_parameter_count = generics.region_param_defs().len();
            let supplied_region_parameter_count = trait_segment.lifetimes.len();
            if trait_region_parameter_count != supplied_region_parameter_count
                && supplied_region_parameter_count != 0 {
                function_context.tcx()
                    .sess
                    .span_err(path.span,
                              format!("expected {nexpected, plural, =1{# lifetime parameter} \
                                                                 other{# lifetime parameters}}, \
                                       found {nsupplied, plural, =1{# lifetime parameter} \
                                                              other{# lifetime parameters}}",
                                      nexpected = trait_region_parameter_count,
                                      nsupplied = supplied_region_parameter_count).as_slice());
            }

            // Make sure the number of type parameters supplied on the trait
            // or implementation segment equals the number of type parameters
            // on the trait or implementation definition.
            let formal_ty_param_count = generics.type_param_defs().len();
            let required_ty_param_count = generics.type_param_defs().iter()
                                                  .take_while(|x| x.default.is_none())
                                                  .count();
            let supplied_ty_param_count = trait_segment.types.len();
            if supplied_ty_param_count < required_ty_param_count {
                let msg = if required_ty_param_count < generics.type_param_defs().len() {
                    format!("the {trait_or_impl} referenced by this path needs at least \
                             {nexpected, plural, =1{# type parameter} \
                                              other{# type parameters}}, \
                             but {nsupplied, plural, =1{# type parameter} \
                                                  other{# type parameters}} were supplied",
                            trait_or_impl = name,
                            nexpected = required_ty_param_count,
                            nsupplied = supplied_ty_param_count)
                } else {
                    format!("the {trait_or_impl} referenced by this path needs \
                             {nexpected, plural, =1{# type parameter} \
                                              other{# type parameters}}, \
                             but {nsupplied, plural, =1{# type parameter} \
                                                  other{# type parameters}} were supplied",
                            trait_or_impl = name,
                            nexpected = required_ty_param_count,
                            nsupplied = supplied_ty_param_count)
                };
                function_context.tcx().sess.span_err(path.span,
                                                     msg.as_slice())
            } else if supplied_ty_param_count > formal_ty_param_count {
                let msg = if required_ty_param_count < generics.type_param_defs().len() {
                    format!("the {trait_or_impl} referenced by this path needs at most \
                             {nexpected, plural, =1{# type parameter} \
                                              other{# type parameters}}, \
                             but {nsupplied, plural, =1{# type parameter} \
                                                  other{# type parameters}} were supplied",
                            trait_or_impl = name,
                            nexpected = formal_ty_param_count,
                            nsupplied = supplied_ty_param_count)
                } else {
                    format!("the {trait_or_impl} referenced by this path needs \
                             {nexpected, plural, =1{# type parameter} \
                                              other{# type parameters}}, \
                             but {nsupplied, plural, =1{# type parameter} \
                                                  other{# type parameters}} were supplied",
                            trait_or_impl = name,
                            nexpected = formal_ty_param_count,
                            nsupplied = supplied_ty_param_count)
                };
                function_context.tcx().sess.span_err(path.span,
                                                     msg.as_slice())
            }
        }
        _ => {
            // Verify that no lifetimes or type parameters are present on
            // the penultimate segment of the path.
            let segment = &path.segments.get(path.segments.len() - 2);
            for lifetime in segment.lifetimes.iter() {
                function_context.tcx()
                    .sess
                    .span_err(lifetime.span,
                              "lifetime parameters may not
                              appear here");
                break;
            }
            for typ in segment.types.iter() {
                function_context.tcx()
                                .sess
                                .span_err(typ.span,
                                          "type parameters may not appear \
                                           here");
                break;
            }
        }
    }
}

/// Invariant:
/// If an expression has any sub-expressions that result in a type error,
/// inspecting that expression's type with `ty::type_is_error` will return
/// true. Likewise, if an expression is known to diverge, inspecting its
/// type with `ty::type_is_bot` will return true (n.b.: since Rust is
/// strict, _|_ can appear in the type of an expression that does not,
/// itself, diverge: for example, fn() -> _|_.)
/// Note that inspecting a type's structure *directly* may expose the fact
/// that there are actually multiple representations for both `ty_err` and
/// `ty_bot`, so avoid that when err and bot need to be handled differently.
fn check_expr_with_unifier(fcx: &FnCtxt,
                           expr: &ast::Expr,
                           expected: Option<ty::t>,
                           lvalue_pref: LvaluePreference,
                           unifier: ||) {
    debug!(">> typechecking");

    fn check_method_argument_types(
        fcx: &FnCtxt,
        sp: Span,
        method_fn_ty: ty::t,
        callee_expr: &ast::Expr,
        args: &[@ast::Expr],
        deref_args: DerefArgs) -> ty::t {
        // HACK(eddyb) ignore provided self (it has special typeck rules).
        let args = args.slice_from(1);
        if ty::type_is_error(method_fn_ty) {
            let err_inputs = err_args(args.len());
            check_argument_types(fcx, sp, err_inputs.as_slice(), callee_expr,
                                 args, deref_args, false);
            method_fn_ty
        } else {
            match ty::get(method_fn_ty).sty {
                ty::ty_bare_fn(ref fty) => {
                    // HACK(eddyb) ignore self in the definition (see above).
                    check_argument_types(fcx, sp, fty.sig.inputs.slice_from(1),
                                         callee_expr, args, deref_args,
                                         fty.sig.variadic);
                    fty.sig.output
                }
                _ => {
                    fcx.tcx().sess.span_bug(callee_expr.span,
                                            "method without bare fn type");
                }
            }
        }
    }

    fn check_argument_types(fcx: &FnCtxt,
                            sp: Span,
                            fn_inputs: &[ty::t],
                            callee_expr: &ast::Expr,
                            args: &[@ast::Expr],
                            deref_args: DerefArgs,
                            variadic: bool) {
        /*!
         *
         * Generic function that factors out common logic from
         * function calls, method calls and overloaded operators.
         */

        let tcx = fcx.ccx.tcx;

        // Grab the argument types, supplying fresh type variables
        // if the wrong number of arguments were supplied
        let supplied_arg_count = args.len();
        let expected_arg_count = fn_inputs.len();
        let formal_tys = if expected_arg_count == supplied_arg_count {
            fn_inputs.iter().map(|a| *a).collect()
        } else if variadic {
            if supplied_arg_count >= expected_arg_count {
                fn_inputs.iter().map(|a| *a).collect()
            } else {
                let msg = format!(
                    "this function takes at least {nexpected, plural, =1{# parameter} \
                                                                   other{# parameters}} \
                     but {nsupplied, plural, =1{# parameter was} \
                                          other{# parameters were}} supplied",
                     nexpected = expected_arg_count,
                     nsupplied = supplied_arg_count);

                tcx.sess.span_err(sp, msg.as_slice());

                err_args(supplied_arg_count)
            }
        } else {
            let msg = format!(
                "this function takes {nexpected, plural, =1{# parameter} \
                                                      other{# parameters}} \
                 but {nsupplied, plural, =1{# parameter was} \
                                      other{# parameters were}} supplied",
                 nexpected = expected_arg_count,
                 nsupplied = supplied_arg_count);

            tcx.sess.span_err(sp, msg.as_slice());

            err_args(supplied_arg_count)
        };

        debug!("check_argument_types: formal_tys={:?}",
               formal_tys.iter().map(|t| fcx.infcx().ty_to_str(*t)).collect::<Vec<String>>());

        // Check the arguments.
        // We do this in a pretty awful way: first we typecheck any arguments
        // that are not anonymous functions, then we typecheck the anonymous
        // functions. This is so that we have more information about the types
        // of arguments when we typecheck the functions. This isn't really the
        // right way to do this.
        let xs = [false, true];
        for check_blocks in xs.iter() {
            let check_blocks = *check_blocks;
            debug!("check_blocks={}", check_blocks);

            // More awful hacks: before we check the blocks, try to do
            // an "opportunistic" vtable resolution of any trait
            // bounds on the call.
            if check_blocks {
                vtable::early_resolve_expr(callee_expr, fcx, true);
            }

            // For variadic functions, we don't have a declared type for all of
            // the arguments hence we only do our usual type checking with
            // the arguments who's types we do know.
            let t = if variadic {
                expected_arg_count
            } else {
                supplied_arg_count
            };
            for (i, arg) in args.iter().take(t).enumerate() {
                let is_block = match arg.node {
                    ast::ExprFnBlock(..) |
                    ast::ExprProc(..) => true,
                    _ => false
                };

                if is_block == check_blocks {
                    debug!("checking the argument");
                    let mut formal_ty = *formal_tys.get(i);

                    match deref_args {
                        DoDerefArgs => {
                            match ty::get(formal_ty).sty {
                                ty::ty_rptr(_, mt) => formal_ty = mt.ty,
                                ty::ty_err => (),
                                _ => {
                                    // So we hit this case when one implements the
                                    // operator traits but leaves an argument as
                                    // just T instead of &T. We'll catch it in the
                                    // mismatch impl/trait method phase no need to
                                    // ICE here.
                                    // See: #11450
                                    formal_ty = ty::mk_err();
                                }
                            }
                        }
                        DontDerefArgs => {}
                    }

                    check_expr_coercable_to_type(fcx, *arg, formal_ty);

                }
            }
        }

        // We also need to make sure we at least write the ty of the other
        // arguments which we skipped above.
        if variadic {
            for arg in args.iter().skip(expected_arg_count) {
                check_expr(fcx, *arg);

                // There are a few types which get autopromoted when passed via varargs
                // in C but we just error out instead and require explicit casts.
                let arg_ty = structurally_resolved_type(fcx, arg.span, fcx.expr_ty(*arg));
                match ty::get(arg_ty).sty {
                    ty::ty_float(ast::TyF32) => {
                        fcx.type_error_message(arg.span,
                                               |t| {
                            format!("can't pass an {} to variadic \
                                     function, cast to c_double", t)
                        }, arg_ty, None);
                    }
                    ty::ty_int(ast::TyI8) | ty::ty_int(ast::TyI16) | ty::ty_bool => {
                        fcx.type_error_message(arg.span, |t| {
                            format!("can't pass {} to variadic \
                                     function, cast to c_int",
                                           t)
                        }, arg_ty, None);
                    }
                    ty::ty_uint(ast::TyU8) | ty::ty_uint(ast::TyU16) => {
                        fcx.type_error_message(arg.span, |t| {
                            format!("can't pass {} to variadic \
                                     function, cast to c_uint",
                                           t)
                        }, arg_ty, None);
                    }
                    _ => {}
                }
            }
        }
    }

    fn err_args(len: uint) -> Vec<ty::t> {
        Vec::from_fn(len, |_| ty::mk_err())
    }

    fn write_call(fcx: &FnCtxt, call_expr: &ast::Expr, output: ty::t) {
        fcx.write_ty(call_expr.id, output);
    }

    // A generic function for doing all of the checking for call expressions
    fn check_call(fcx: &FnCtxt,
                  call_expr: &ast::Expr,
                  f: &ast::Expr,
                  args: &[@ast::Expr]) {
        // Index expressions need to be handled separately, to inform them
        // that they appear in call position.
        check_expr(fcx, f);

        // Store the type of `f` as the type of the callee
        let fn_ty = fcx.expr_ty(f);

        // Extract the function signature from `in_fty`.
        let fn_sty = structure_of(fcx, f.span, fn_ty);

        // This is the "default" function signature, used in case of error.
        // In that case, we check each argument against "error" in order to
        // set up all the node type bindings.
        let error_fn_sig = FnSig {
            binder_id: ast::CRATE_NODE_ID,
            inputs: err_args(args.len()),
            output: ty::mk_err(),
            variadic: false
        };

        let fn_sig = match *fn_sty {
            ty::ty_bare_fn(ty::BareFnTy {sig: ref sig, ..}) |
            ty::ty_closure(box ty::ClosureTy {sig: ref sig, ..}) => sig,
            _ => {
                fcx.type_error_message(call_expr.span, |actual| {
                    format!("expected function but found `{}`", actual)
                }, fn_ty, None);
                &error_fn_sig
            }
        };

        // Replace any bound regions that appear in the function
        // signature with region variables
        let (_, fn_sig) = replace_late_bound_regions_in_fn_sig(fcx.tcx(), fn_sig, |br| {
            fcx.infcx().next_region_var(infer::LateBoundRegion(call_expr.span, br))
        });

        // Call the generic checker.
        check_argument_types(fcx, call_expr.span, fn_sig.inputs.as_slice(), f,
                             args, DontDerefArgs, fn_sig.variadic);

        write_call(fcx, call_expr, fn_sig.output);
    }

    // Checks a method call.
    fn check_method_call(fcx: &FnCtxt,
                         expr: &ast::Expr,
                         method_name: ast::SpannedIdent,
                         args: &[@ast::Expr],
                         tps: &[ast::P<ast::Ty>]) {
        let rcvr = args[0];
        // We can't know if we need &mut self before we look up the method,
        // so treat the receiver as mutable just in case - only explicit
        // overloaded dereferences care about the distinction.
        check_expr_with_lvalue_pref(fcx, rcvr, PreferMutLvalue);

        // no need to check for bot/err -- callee does that
        let expr_t = structurally_resolved_type(fcx,
                                                expr.span,
                                                fcx.expr_ty(rcvr));

        let tps = tps.iter().map(|&ast_ty| fcx.to_ty(ast_ty)).collect::<Vec<_>>();
        let fn_ty = match method::lookup(fcx, expr, rcvr,
                                         method_name.node.name,
                                         expr_t, tps.as_slice(),
                                         DontDerefArgs,
                                         CheckTraitsAndInherentMethods,
                                         AutoderefReceiver, IgnoreStaticMethods) {
            Some(method) => {
                let method_ty = method.ty;
                let method_call = MethodCall::expr(expr.id);
                fcx.inh.method_map.borrow_mut().insert(method_call, method);
                method_ty
            }
            None => {
                debug!("(checking method call) failing expr is {}", expr.id);

                fcx.type_error_message(method_name.span,
                  |actual| {
                      format!("type `{}` does not implement any \
                               method in scope named `{}`",
                              actual,
                              token::get_ident(method_name.node))
                  },
                  expr_t,
                  None);

                // Add error type for the result
                fcx.write_error(expr.id);

                // Check for potential static matches (missing self parameters)
                method::lookup(fcx,
                               expr,
                               rcvr,
                               method_name.node.name,
                               expr_t,
                               tps.as_slice(),
                               DontDerefArgs,
                               CheckTraitsAndInherentMethods,
                               DontAutoderefReceiver,
                               ReportStaticMethods);

                ty::mk_err()
            }
        };

        // Call the generic checker.
        let ret_ty = check_method_argument_types(fcx, method_name.span,
                                                 fn_ty, expr, args,
                                                 DontDerefArgs);

        write_call(fcx, expr, ret_ty);
    }

    // A generic function for checking the then and else in an if
    // or if-check
    fn check_then_else(fcx: &FnCtxt,
                       cond_expr: &ast::Expr,
                       then_blk: &ast::Block,
                       opt_else_expr: Option<@ast::Expr>,
                       id: ast::NodeId,
                       sp: Span,
                       expected: Option<ty::t>) {
        check_expr_has_type(fcx, cond_expr, ty::mk_bool());

        let branches_ty = match opt_else_expr {
            Some(else_expr) => {
                check_block_with_expected(fcx, then_blk, expected);
                let then_ty = fcx.node_ty(then_blk.id);
                check_expr_with_opt_hint(fcx, else_expr, expected);
                let else_ty = fcx.expr_ty(else_expr);
                infer::common_supertype(fcx.infcx(),
                                        infer::IfExpression(sp),
                                        true,
                                        then_ty,
                                        else_ty)
            }
            None => {
                check_block_no_value(fcx, then_blk);
                ty::mk_nil()
            }
        };

        let cond_ty = fcx.expr_ty(cond_expr);
        let if_ty = if ty::type_is_error(cond_ty) {
            ty::mk_err()
        } else if ty::type_is_bot(cond_ty) {
            ty::mk_bot()
        } else {
            branches_ty
        };

        fcx.write_ty(id, if_ty);
    }

    fn lookup_op_method(fcx: &FnCtxt,
                        op_ex: &ast::Expr,
                        self_t: ty::t,
                        opname: ast::Name,
                        trait_did: Option<ast::DefId>,
                        args: &[@ast::Expr],
                        autoderef_receiver: AutoderefReceiverFlag,
                        unbound_method: ||) -> ty::t {
        let method = match trait_did {
            Some(trait_did) => {
                method::lookup_in_trait(fcx, op_ex.span, Some(&*args[0]), opname,
                                        trait_did, self_t, [], autoderef_receiver,
                                        IgnoreStaticMethods)
            }
            None => None
        };
        match method {
            Some(method) => {
                let method_ty = method.ty;
                // HACK(eddyb) Fully qualified path to work around a resolve bug.
                let method_call = ::middle::typeck::MethodCall::expr(op_ex.id);
                fcx.inh.method_map.borrow_mut().insert(method_call, method);
                check_method_argument_types(fcx, op_ex.span,
                                            method_ty, op_ex,
                                            args, DoDerefArgs)
            }
            None => {
                unbound_method();
                // Check the args anyway
                // so we get all the error messages
                let expected_ty = ty::mk_err();
                check_method_argument_types(fcx, op_ex.span,
                                            expected_ty, op_ex,
                                            args, DoDerefArgs);
                ty::mk_err()
            }
        }
    }

    // could be either an expr_binop or an expr_assign_binop
    fn check_binop(fcx: &FnCtxt,
                   expr: &ast::Expr,
                   op: ast::BinOp,
                   lhs: @ast::Expr,
                   rhs: @ast::Expr,
                   is_binop_assignment: IsBinopAssignment) {
        let tcx = fcx.ccx.tcx;

        let lvalue_pref = match is_binop_assignment {
            BinopAssignment => PreferMutLvalue,
            SimpleBinop => NoPreference
        };
        check_expr_with_lvalue_pref(fcx, lhs, lvalue_pref);

        // Callee does bot / err checking
        let lhs_t = structurally_resolved_type(fcx, lhs.span,
                                               fcx.expr_ty(lhs));

        if ty::type_is_integral(lhs_t) && ast_util::is_shift_binop(op) {
            // Shift is a special case: rhs can be any integral type
            check_expr(fcx, rhs);
            let rhs_t = fcx.expr_ty(rhs);
            require_integral(fcx, rhs.span, rhs_t);
            fcx.write_ty(expr.id, lhs_t);
            return;
        }

        if ty::is_binopable(tcx, lhs_t, op) {
            let tvar = fcx.infcx().next_ty_var();
            demand::suptype(fcx, expr.span, tvar, lhs_t);
            check_expr_has_type(fcx, rhs, tvar);

            let result_t = match op {
                ast::BiEq | ast::BiNe | ast::BiLt | ast::BiLe | ast::BiGe |
                ast::BiGt => {
                    if ty::type_is_simd(tcx, lhs_t) {
                        if ty::type_is_fp(ty::simd_type(tcx, lhs_t)) {
                            fcx.type_error_message(expr.span,
                                |actual| {
                                    format!("binary comparison \
                                             operation `{}` not \
                                             supported for floating \
                                             point SIMD vector `{}`",
                                            ast_util::binop_to_str(op),
                                            actual)
                                },
                                lhs_t,
                                None
                            );
                            ty::mk_err()
                        } else {
                            lhs_t
                        }
                    } else {
                        ty::mk_bool()
                    }
                },
                _ => lhs_t,
            };

            fcx.write_ty(expr.id, result_t);
            return;
        }

        if op == ast::BiOr || op == ast::BiAnd {
            // This is an error; one of the operands must have the wrong
            // type
            fcx.write_error(expr.id);
            fcx.write_error(rhs.id);
            fcx.type_error_message(expr.span,
                                   |actual| {
                    format!("binary operation `{}` cannot be applied \
                             to type `{}`",
                            ast_util::binop_to_str(op),
                            actual)
                },
                lhs_t,
                None)
        }

        // Check for overloaded operators if not an assignment.
        let result_t = if is_binop_assignment == SimpleBinop {
            check_user_binop(fcx, expr, lhs, lhs_t, op, rhs)
        } else {
            fcx.type_error_message(expr.span,
                                   |actual| {
                                        format!("binary assignment \
                                                 operation `{}=` \
                                                 cannot be applied to \
                                                 type `{}`",
                                                ast_util::binop_to_str(op),
                                                actual)
                                   },
                                   lhs_t,
                                   None);
            check_expr(fcx, rhs);
            ty::mk_err()
        };

        fcx.write_ty(expr.id, result_t);
        if ty::type_is_error(result_t) {
            fcx.write_ty(rhs.id, result_t);
        }
    }

    fn check_user_binop(fcx: &FnCtxt,
                        ex: &ast::Expr,
                        lhs_expr: @ast::Expr,
                        lhs_resolved_t: ty::t,
                        op: ast::BinOp,
                        rhs: @ast::Expr) -> ty::t {
        let tcx = fcx.ccx.tcx;
        let lang = &tcx.lang_items;
        let (name, trait_did) = match op {
            ast::BiAdd => ("add", lang.add_trait()),
            ast::BiSub => ("sub", lang.sub_trait()),
            ast::BiMul => ("mul", lang.mul_trait()),
            ast::BiDiv => ("div", lang.div_trait()),
            ast::BiRem => ("rem", lang.rem_trait()),
            ast::BiBitXor => ("bitxor", lang.bitxor_trait()),
            ast::BiBitAnd => ("bitand", lang.bitand_trait()),
            ast::BiBitOr => ("bitor", lang.bitor_trait()),
            ast::BiShl => ("shl", lang.shl_trait()),
            ast::BiShr => ("shr", lang.shr_trait()),
            ast::BiLt => ("lt", lang.ord_trait()),
            ast::BiLe => ("le", lang.ord_trait()),
            ast::BiGe => ("ge", lang.ord_trait()),
            ast::BiGt => ("gt", lang.ord_trait()),
            ast::BiEq => ("eq", lang.eq_trait()),
            ast::BiNe => ("ne", lang.eq_trait()),
            ast::BiAnd | ast::BiOr => {
                check_expr(fcx, rhs);
                return ty::mk_err();
            }
        };
        lookup_op_method(fcx, ex, lhs_resolved_t, token::intern(name),
                         trait_did, [lhs_expr, rhs], DontAutoderefReceiver, || {
            fcx.type_error_message(ex.span, |actual| {
                format!("binary operation `{}` cannot be applied to type `{}`",
                        ast_util::binop_to_str(op),
                        actual)
            }, lhs_resolved_t, None)
        })
    }

    fn check_user_unop(fcx: &FnCtxt,
                       op_str: &str,
                       mname: &str,
                       trait_did: Option<ast::DefId>,
                       ex: &ast::Expr,
                       rhs_expr: @ast::Expr,
                       rhs_t: ty::t) -> ty::t {
       lookup_op_method(fcx, ex, rhs_t, token::intern(mname),
                        trait_did, [rhs_expr], DontAutoderefReceiver, || {
            fcx.type_error_message(ex.span, |actual| {
                format!("cannot apply unary operator `{}` to type `{}`",
                        op_str, actual)
            }, rhs_t, None);
        })
    }

    // Resolves `expected` by a single level if it is a variable and passes it
    // through the `unpack` function.  It there is no expected type or
    // resolution is not possible (e.g., no constraints yet present), just
    // returns `none`.
    fn unpack_expected<O>(
                       fcx: &FnCtxt,
                       expected: Option<ty::t>,
                       unpack: |&ty::sty| -> Option<O>)
                       -> Option<O> {
        match expected {
            Some(t) => {
                match resolve_type(fcx.infcx(), t, force_tvar) {
                    Ok(t) => unpack(&ty::get(t).sty),
                    _ => None
                }
            }
            _ => None
        }
    }

    fn check_expr_fn(fcx: &FnCtxt,
                     expr: &ast::Expr,
                     store: ty::TraitStore,
                     decl: &ast::FnDecl,
                     body: ast::P<ast::Block>,
                     expected: Option<ty::t>) {
        let tcx = fcx.ccx.tcx;

        // Find the expected input/output types (if any). Substitute
        // fresh bound regions for any bound regions we find in the
        // expected types so as to avoid capture.
        let expected_sty = unpack_expected(fcx,
                                           expected,
                                           |x| Some((*x).clone()));
        let (expected_sig,
             expected_onceness,
             expected_bounds) = {
            match expected_sty {
                Some(ty::ty_closure(ref cenv)) => {
                    let (_, sig) =
                        replace_late_bound_regions_in_fn_sig(
                            tcx, &cenv.sig,
                            |_| fcx.inh.infcx.fresh_bound_region(expr.id));
                    let onceness = match (&store, &cenv.store) {
                        // As the closure type and onceness go, only three
                        // combinations are legit:
                        //      once closure
                        //      many closure
                        //      once proc
                        // If the actual and expected closure type disagree with
                        // each other, set expected onceness to be always Once or
                        // Many according to the actual type. Otherwise, it will
                        // yield either an illegal "many proc" or a less known
                        // "once closure" in the error message.
                        (&ty::UniqTraitStore, &ty::UniqTraitStore) |
                        (&ty::RegionTraitStore(..), &ty::RegionTraitStore(..)) =>
                            cenv.onceness,
                        (&ty::UniqTraitStore, _) => ast::Once,
                        (&ty::RegionTraitStore(..), _) => ast::Many,
                    };
                    (Some(sig), onceness, cenv.bounds)
                }
                _ => {
                    // Not an error! Means we're inferring the closure type
                    let mut bounds = ty::empty_builtin_bounds();
                    let onceness = match expr.node {
                        ast::ExprProc(..) => {
                            bounds.add(ty::BoundSend);
                            ast::Once
                        }
                        _ => ast::Many
                    };
                    (None, onceness, bounds)
                }
            }
        };

        // construct the function type
        let fn_ty = astconv::ty_of_closure(fcx,
                                           expr.id,
                                           ast::NormalFn,
                                           expected_onceness,
                                           expected_bounds,
                                           store,
                                           decl,
                                           expected_sig);
        let fty_sig = fn_ty.sig.clone();
        let fty = ty::mk_closure(tcx, fn_ty);
        debug!("check_expr_fn fty={}", fcx.infcx().ty_to_str(fty));

        fcx.write_ty(expr.id, fty);

        // If the closure is a stack closure and hasn't had some non-standard
        // style inferred for it, then check it under its parent's style.
        // Otherwise, use its own
        let (inherited_style, id) = match store {
            ty::RegionTraitStore(..) => (fcx.ps.borrow().fn_style,
                                         fcx.ps.borrow().def),
            ty::UniqTraitStore => (ast::NormalFn, expr.id)
        };

        check_fn(fcx.ccx, inherited_style, &fty_sig,
                 decl, id, body, fcx.inh);
    }


    // Check field access expressions
    fn check_field(fcx: &FnCtxt,
                   expr: &ast::Expr,
                   lvalue_pref: LvaluePreference,
                   base: &ast::Expr,
                   field: ast::Name,
                   tys: &[ast::P<ast::Ty>]) {
        let tcx = fcx.ccx.tcx;
        check_expr_with_lvalue_pref(fcx, base, lvalue_pref);
        let expr_t = structurally_resolved_type(fcx, expr.span,
                                                fcx.expr_ty(base));
        // FIXME(eddyb) #12808 Integrate privacy into this auto-deref loop.
        let (_, autoderefs, field_ty) =
            autoderef(fcx, expr.span, expr_t, Some(base.id), lvalue_pref, |base_t, _| {
                match ty::get(base_t).sty {
                    ty::ty_struct(base_id, ref substs) => {
                        debug!("struct named {}", ppaux::ty_to_str(tcx, base_t));
                        let fields = ty::lookup_struct_fields(tcx, base_id);
                        lookup_field_ty(tcx, base_id, fields.as_slice(), field, &(*substs))
                    }
                    _ => None
                }
            });
        match field_ty {
            Some(field_ty) => {
                fcx.write_ty(expr.id, field_ty);
                fcx.write_autoderef_adjustment(base.id, autoderefs);
                return;
            }
            None => {}
        }

        let tps: Vec<ty::t> = tys.iter().map(|&ty| fcx.to_ty(ty)).collect();
        match method::lookup(fcx,
                             expr,
                             base,
                             field,
                             expr_t,
                             tps.as_slice(),
                             DontDerefArgs,
                             CheckTraitsAndInherentMethods,
                             AutoderefReceiver,
                             IgnoreStaticMethods) {
            Some(_) => {
                fcx.type_error_message(
                    expr.span,
                    |actual| {
                        format!("attempted to take value of method `{}` on type \
                                 `{}`", token::get_name(field), actual)
                    },
                    expr_t, None);

                tcx.sess.span_note(expr.span,
                    "maybe a missing `()` to call it? If not, try an anonymous function.");
            }

            None => {
                fcx.type_error_message(
                    expr.span,
                    |actual| {
                        format!("attempted access of field `{}` on \
                                        type `{}`, but no field with that \
                                        name was found",
                                       token::get_name(field),
                                       actual)
                    },
                    expr_t, None);
            }
        }

        fcx.write_error(expr.id);
    }

    fn check_struct_or_variant_fields(fcx: &FnCtxt,
                                      struct_ty: ty::t,
                                      span: Span,
                                      class_id: ast::DefId,
                                      node_id: ast::NodeId,
                                      substitutions: subst::Substs,
                                      field_types: &[ty::field_ty],
                                      ast_fields: &[ast::Field],
                                      check_completeness: bool)  {
        let tcx = fcx.ccx.tcx;

        let mut class_field_map = HashMap::new();
        let mut fields_found = 0;
        for field in field_types.iter() {
            class_field_map.insert(field.name, (field.id, false));
        }

        let mut error_happened = false;

        // Typecheck each field.
        for field in ast_fields.iter() {
            let mut expected_field_type = ty::mk_err();

            let pair = class_field_map.find(&field.ident.node.name).map(|x| *x);
            match pair {
                None => {
                    fcx.type_error_message(
                      field.ident.span,
                      |actual| {
                          format!("structure `{}` has no field named `{}`",
                                  actual, token::get_ident(field.ident.node))
                      },
                      struct_ty,
                      None);
                    error_happened = true;
                }
                Some((_, true)) => {
                    tcx.sess.span_err(
                        field.ident.span,
                        format!("field `{}` specified more than once",
                                token::get_ident(field.ident
                                                      .node)).as_slice());
                    error_happened = true;
                }
                Some((field_id, false)) => {
                    expected_field_type =
                        ty::lookup_field_type(
                            tcx, class_id, field_id, &substitutions);
                    class_field_map.insert(
                        field.ident.node.name, (field_id, true));
                    fields_found += 1;
                }
            }
            // Make sure to give a type to the field even if there's
            // an error, so we can continue typechecking
            check_expr_coercable_to_type(
                    fcx,
                    field.expr,
                    expected_field_type);
        }

        if error_happened {
            fcx.write_error(node_id);
        }

        if check_completeness && !error_happened {
            // Make sure the programmer specified all the fields.
            assert!(fields_found <= field_types.len());
            if fields_found < field_types.len() {
                let mut missing_fields = Vec::new();
                for class_field in field_types.iter() {
                    let name = class_field.name;
                    let (_, seen) = *class_field_map.get(&name);
                    if !seen {
                        missing_fields.push(
                            format!("`{}`", token::get_name(name).get()))
                    }
                }

                tcx.sess.span_err(span,
                    format!(
                        "missing {nfields, plural, =1{field} other{fields}}: {fields}",
                        nfields = missing_fields.len(),
                        fields = missing_fields.connect(", ")).as_slice());
             }
        }

        if !error_happened {
            fcx.write_ty(node_id, ty::mk_struct(fcx.ccx.tcx,
                                class_id, substitutions));
        }
    }

    fn check_struct_constructor(fcx: &FnCtxt,
                                id: ast::NodeId,
                                span: codemap::Span,
                                class_id: ast::DefId,
                                fields: &[ast::Field],
                                base_expr: Option<@ast::Expr>) {
        let tcx = fcx.ccx.tcx;

        // Look up the number of type parameters and the raw type, and
        // determine whether the class is region-parameterized.
        let item_type = ty::lookup_item_type(tcx, class_id);
        let type_parameter_count = item_type.generics.type_param_defs().len();
        let region_param_defs = item_type.generics.region_param_defs();
        let raw_type = item_type.ty;

        // Generate the struct type.
        let regions = fcx.infcx().region_vars_for_defs(span, region_param_defs);
        let type_parameters = fcx.infcx().next_ty_vars(type_parameter_count);
        let substitutions = subst::Substs {
            regions: subst::NonerasedRegions(regions),
            self_ty: None,
            tps: type_parameters
        };

        let mut struct_type = raw_type.subst(tcx, &substitutions);

        // Look up and check the fields.
        let class_fields = ty::lookup_struct_fields(tcx, class_id);
        check_struct_or_variant_fields(fcx,
                                       struct_type,
                                       span,
                                       class_id,
                                       id,
                                       substitutions,
                                       class_fields.as_slice(),
                                       fields,
                                       base_expr.is_none());
        if ty::type_is_error(fcx.node_ty(id)) {
            struct_type = ty::mk_err();
        }

        // Check the base expression if necessary.
        match base_expr {
            None => {}
            Some(base_expr) => {
                check_expr_has_type(fcx, base_expr, struct_type);
                if ty::type_is_bot(fcx.node_ty(base_expr.id)) {
                    struct_type = ty::mk_bot();
                }
            }
        }

        // Write in the resulting type.
        fcx.write_ty(id, struct_type);
    }

    fn check_struct_enum_variant(fcx: &FnCtxt,
                                 id: ast::NodeId,
                                 span: codemap::Span,
                                 enum_id: ast::DefId,
                                 variant_id: ast::DefId,
                                 fields: &[ast::Field]) {
        let tcx = fcx.ccx.tcx;

        // Look up the number of type parameters and the raw type, and
        // determine whether the enum is region-parameterized.
        let item_type = ty::lookup_item_type(tcx, enum_id);
        let type_parameter_count = item_type.generics.type_param_defs().len();
        let region_param_defs = item_type.generics.region_param_defs();
        let raw_type = item_type.ty;

        // Generate the enum type.
        let regions = fcx.infcx().region_vars_for_defs(span, region_param_defs);
        let type_parameters = fcx.infcx().next_ty_vars(type_parameter_count);
        let substitutions = subst::Substs {
            regions: subst::NonerasedRegions(regions),
            self_ty: None,
            tps: type_parameters
        };

        let enum_type = raw_type.subst(tcx, &substitutions);

        // Look up and check the enum variant fields.
        let variant_fields = ty::lookup_struct_fields(tcx, variant_id);
        check_struct_or_variant_fields(fcx,
                                       enum_type,
                                       span,
                                       variant_id,
                                       id,
                                       substitutions,
                                       variant_fields.as_slice(),
                                       fields,
                                       true);
        fcx.write_ty(id, enum_type);
    }

    let tcx = fcx.ccx.tcx;
    let id = expr.id;
    match expr.node {
        ast::ExprVstore(ev, vst) => {
            let typ = match ev.node {
                ast::ExprVec(ref args) => {
                    let mutability = match vst {
                        ast::ExprVstoreMutSlice => ast::MutMutable,
                        _ => ast::MutImmutable,
                    };
                    let mut any_error = false;
                    let mut any_bot = false;
                    let t: ty::t = fcx.infcx().next_ty_var();
                    for e in args.iter() {
                        check_expr_has_type(fcx, *e, t);
                        let arg_t = fcx.expr_ty(*e);
                        if ty::type_is_error(arg_t) {
                            any_error = true;
                        }
                        else if ty::type_is_bot(arg_t) {
                            any_bot = true;
                        }
                    }
                    if any_error {
                        ty::mk_err()
                    } else if any_bot {
                        ty::mk_bot()
                    } else {
                        ast_expr_vstore_to_ty(fcx, ev, vst, ||
                            ty::mt{ ty: ty::mk_vec(tcx,
                                                   ty::mt {ty: t, mutbl: mutability},
                                                   None),
                                                   mutbl: mutability })
                    }
                }
                ast::ExprRepeat(element, count_expr) => {
                    check_expr_with_hint(fcx, count_expr, ty::mk_uint());
                    let _ = ty::eval_repeat_count(fcx, count_expr);
                    let mutability = match vst {
                        ast::ExprVstoreMutSlice => ast::MutMutable,
                        _ => ast::MutImmutable,
                    };
                    let t = fcx.infcx().next_ty_var();
                    check_expr_has_type(fcx, element, t);
                    let arg_t = fcx.expr_ty(element);
                    if ty::type_is_error(arg_t) {
                        ty::mk_err()
                    } else if ty::type_is_bot(arg_t) {
                        ty::mk_bot()
                    } else {
                        ast_expr_vstore_to_ty(fcx, ev, vst, ||
                            ty::mt{ ty: ty::mk_vec(tcx,
                                                   ty::mt {ty: t, mutbl: mutability},
                                                   None),
                                                   mutbl: mutability})
                    }
                }
                ast::ExprLit(_) => {
                    let error = if vst == ast::ExprVstoreSlice {
                        "`&\"string\"` has been removed; use `\"string\"` instead"
                    } else {
                        "`~\"string\"` has been removed; use `\"string\".to_string()` instead"
                    };
                    tcx.sess.span_err(expr.span, error);
                    ty::mk_err()
                }
                _ => tcx.sess.span_bug(expr.span, "vstore modifier on non-sequence"),
            };
            fcx.write_ty(ev.id, typ);
            fcx.write_ty(id, typ);
        }

      ast::ExprBox(place, subexpr) => {
          check_expr(fcx, place);
          check_expr(fcx, subexpr);

          let mut checked = false;
          match place.node {
              ast::ExprPath(ref path) => {
                  // FIXME(pcwalton): For now we hardcode the two permissible
                  // places: the exchange heap and the managed heap.
                  let definition = lookup_def(fcx, path.span, place.id);
                  let def_id = definition.def_id();
                  match tcx.lang_items
                           .items
                           .get(ExchangeHeapLangItem as uint) {
                      &Some(item_def_id) if def_id == item_def_id => {
                          fcx.write_ty(id, ty::mk_uniq(tcx,
                                                       fcx.expr_ty(subexpr)));
                          checked = true
                      }
                      &Some(_) | &None => {}
                  }
                  if !checked {
                      match tcx.lang_items
                               .items
                               .get(ManagedHeapLangItem as uint) {
                          &Some(item_def_id) if def_id == item_def_id => {
                              // Assign the magic `Gc<T>` struct.
                              let gc_struct_id =
                                  match tcx.lang_items
                                           .require(GcLangItem) {
                                      Ok(id) => id,
                                      Err(msg) => {
                                          tcx.sess.span_err(expr.span,
                                                            msg.as_slice());
                                          ast::DefId {
                                              krate: ast::CRATE_NODE_ID,
                                              node: ast::DUMMY_NODE_ID,
                                          }
                                      }
                                  };
                              let regions =
                                  subst::NonerasedRegions(Vec::new());
                              let sty = ty::mk_struct(tcx,
                                                      gc_struct_id,
                                                      subst::Substs {
                                                        self_ty: None,
                                                        tps: vec!(
                                                            fcx.expr_ty(
                                                                subexpr)
                                                        ),
                                                        regions: regions,
                                                      });
                              fcx.write_ty(id, sty);
                              checked = true
                          }
                          &Some(_) | &None => {}
                      }
                  }
              }
              _ => {}
          }

          if !checked {
              tcx.sess.span_err(expr.span,
                                "only the managed heap and exchange heap are \
                                 currently supported");
              fcx.write_ty(id, ty::mk_err());
          }
      }

      ast::ExprLit(lit) => {
        let typ = check_lit(fcx, lit);
        fcx.write_ty(id, typ);
      }
      ast::ExprBinary(op, lhs, rhs) => {
        check_binop(fcx, expr, op, lhs, rhs, SimpleBinop);

        let lhs_ty = fcx.expr_ty(lhs);
        let rhs_ty = fcx.expr_ty(rhs);
        if ty::type_is_error(lhs_ty) ||
            ty::type_is_error(rhs_ty) {
            fcx.write_error(id);
        }
        else if ty::type_is_bot(lhs_ty) ||
          (ty::type_is_bot(rhs_ty) && !ast_util::lazy_binop(op)) {
            fcx.write_bot(id);
        }
      }
      ast::ExprAssignOp(op, lhs, rhs) => {
        check_binop(fcx, expr, op, lhs, rhs, BinopAssignment);

        let lhs_t = fcx.expr_ty(lhs);
        let result_t = fcx.expr_ty(expr);
        demand::suptype(fcx, expr.span, result_t, lhs_t);

        let tcx = fcx.tcx();
        if !ty::expr_is_lval(tcx, lhs) {
            tcx.sess.span_err(lhs.span, "illegal left-hand side expression");
        }

        // Overwrite result of check_binop...this preserves existing behavior
        // but seems quite dubious with regard to user-defined methods
        // and so forth. - Niko
        if !ty::type_is_error(result_t)
            && !ty::type_is_bot(result_t) {
            fcx.write_nil(expr.id);
        }
      }
      ast::ExprUnary(unop, oprnd) => {
        let exp_inner = unpack_expected(fcx, expected, |sty| {
            match unop {
                ast::UnBox | ast::UnUniq => match *sty {
                    ty::ty_box(ty) | ty::ty_uniq(ty) => Some(ty),
                    _ => None
                },
                ast::UnNot | ast::UnNeg => expected,
                ast::UnDeref => None
            }
        });
        let lvalue_pref = match unop {
            ast::UnDeref => lvalue_pref,
            _ => NoPreference
        };
        check_expr_with_opt_hint_and_lvalue_pref(fcx, oprnd, exp_inner, lvalue_pref);
        let mut oprnd_t = fcx.expr_ty(oprnd);
        if !ty::type_is_error(oprnd_t) && !ty::type_is_bot(oprnd_t) {
            match unop {
                ast::UnBox => {
                    oprnd_t = ty::mk_box(tcx, oprnd_t)
                }
                ast::UnUniq => {
                    oprnd_t = ty::mk_uniq(tcx, oprnd_t);
                }
                ast::UnDeref => {
                    oprnd_t = structurally_resolved_type(fcx, expr.span, oprnd_t);
                    oprnd_t = match ty::deref(oprnd_t, true) {
                        Some(mt) => mt.ty,
                        None => match try_overloaded_deref(fcx, expr.span,
                                                           Some(MethodCall::expr(expr.id)),
                                                           Some(&*oprnd), oprnd_t, lvalue_pref) {
                            Some(mt) => mt.ty,
                            None => {
                                let is_newtype = match ty::get(oprnd_t).sty {
                                    ty::ty_struct(did, ref substs) => {
                                        let fields = ty::struct_fields(fcx.tcx(), did, substs);
                                        fields.len() == 1
                                        && fields.get(0).ident ==
                                        token::special_idents::unnamed_field
                                    }
                                    _ => false
                                };
                                if is_newtype {
                                    // This is an obsolete struct deref
                                    tcx.sess.span_err(expr.span,
                                        "single-field tuple-structs can \
                                         no longer be dereferenced");
                                } else {
                                    fcx.type_error_message(expr.span, |actual| {
                                        format!("type `{}` cannot be \
                                                dereferenced", actual)
                                    }, oprnd_t, None);
                                }
                                ty::mk_err()
                            }
                        }
                    };
                }
                ast::UnNot => {
                    oprnd_t = structurally_resolved_type(fcx, oprnd.span,
                                                         oprnd_t);
                    if !(ty::type_is_integral(oprnd_t) ||
                         ty::get(oprnd_t).sty == ty::ty_bool) {
                        oprnd_t = check_user_unop(fcx, "!", "not",
                                                  tcx.lang_items.not_trait(),
                                                  expr, oprnd, oprnd_t);
                    }
                }
                ast::UnNeg => {
                    oprnd_t = structurally_resolved_type(fcx, oprnd.span,
                                                         oprnd_t);
                    if !(ty::type_is_integral(oprnd_t) ||
                         ty::type_is_fp(oprnd_t)) {
                        oprnd_t = check_user_unop(fcx, "-", "neg",
                                                  tcx.lang_items.neg_trait(),
                                                  expr, oprnd, oprnd_t);
                    }
                }
            }
        }
        fcx.write_ty(id, oprnd_t);
      }
      ast::ExprAddrOf(mutbl, oprnd) => {
          let hint = unpack_expected(
              fcx, expected,
              |sty| match *sty { ty::ty_rptr(_, ref mt) => Some(mt.ty),
                                 _ => None });
        let lvalue_pref = match mutbl {
            ast::MutMutable => PreferMutLvalue,
            ast::MutImmutable => NoPreference
        };
        check_expr_with_opt_hint_and_lvalue_pref(fcx, oprnd, hint, lvalue_pref);

        // Note: at this point, we cannot say what the best lifetime
        // is to use for resulting pointer.  We want to use the
        // shortest lifetime possible so as to avoid spurious borrowck
        // errors.  Moreover, the longest lifetime will depend on the
        // precise details of the value whose address is being taken
        // (and how long it is valid), which we don't know yet until type
        // inference is complete.
        //
        // Therefore, here we simply generate a region variable.  The
        // region inferencer will then select the ultimate value.
        // Finally, borrowck is charged with guaranteeing that the
        // value whose address was taken can actually be made to live
        // as long as it needs to live.
        let region = fcx.infcx().next_region_var(
            infer::AddrOfRegion(expr.span));

        let tm = ty::mt { ty: fcx.expr_ty(oprnd), mutbl: mutbl };
        let oprnd_t = if ty::type_is_error(tm.ty) {
            ty::mk_err()
        } else if ty::type_is_bot(tm.ty) {
            ty::mk_bot()
        }
        else {
            ty::mk_rptr(tcx, region, tm)
        };
        fcx.write_ty(id, oprnd_t);
      }
      ast::ExprPath(ref pth) => {
        let defn = lookup_def(fcx, pth.span, id);

        check_type_parameter_positions_in_path(fcx, pth, defn);
        let tpt = ty_param_bounds_and_ty_for_def(fcx, expr.span, defn);
        instantiate_path(fcx, pth, tpt, defn, expr.span, expr.id);
      }
      ast::ExprInlineAsm(ref ia) => {
          for &(_, input) in ia.inputs.iter() {
              check_expr(fcx, input);
          }
          for &(_, out) in ia.outputs.iter() {
              check_expr(fcx, out);
          }
          fcx.write_nil(id);
      }
      ast::ExprMac(_) => tcx.sess.bug("unexpanded macro"),
      ast::ExprBreak(_) => { fcx.write_bot(id); }
      ast::ExprAgain(_) => { fcx.write_bot(id); }
      ast::ExprRet(expr_opt) => {
        let ret_ty = fcx.ret_ty;
        match expr_opt {
          None => match fcx.mk_eqty(false, infer::Misc(expr.span),
                                    ret_ty, ty::mk_nil()) {
            Ok(_) => { /* fall through */ }
            Err(_) => {
                tcx.sess.span_err(
                    expr.span,
                    "`return;` in function returning non-nil");
            }
          },
          Some(e) => {
              check_expr_coercable_to_type(fcx, e, ret_ty);
          }
        }
        fcx.write_bot(id);
      }
      ast::ExprParen(a) => {
        check_expr_with_opt_hint_and_lvalue_pref(fcx, a, expected, lvalue_pref);
        fcx.write_ty(id, fcx.expr_ty(a));
      }
      ast::ExprAssign(lhs, rhs) => {
        check_expr_with_lvalue_pref(fcx, lhs, PreferMutLvalue);

        let tcx = fcx.tcx();
        if !ty::expr_is_lval(tcx, lhs) {
            tcx.sess.span_err(lhs.span, "illegal left-hand side expression");
        }

        let lhs_ty = fcx.expr_ty(lhs);
        check_expr_has_type(fcx, rhs, lhs_ty);
        let rhs_ty = fcx.expr_ty(rhs);

        if ty::type_is_error(lhs_ty) || ty::type_is_error(rhs_ty) {
            fcx.write_error(id);
        } else if ty::type_is_bot(lhs_ty) || ty::type_is_bot(rhs_ty) {
            fcx.write_bot(id);
        } else {
            fcx.write_nil(id);
        }
      }
      ast::ExprIf(cond, then_blk, opt_else_expr) => {
        check_then_else(fcx, cond, then_blk, opt_else_expr,
                        id, expr.span, expected);
      }
      ast::ExprWhile(cond, body) => {
        check_expr_has_type(fcx, cond, ty::mk_bool());
        check_block_no_value(fcx, body);
        let cond_ty = fcx.expr_ty(cond);
        let body_ty = fcx.node_ty(body.id);
        if ty::type_is_error(cond_ty) || ty::type_is_error(body_ty) {
            fcx.write_error(id);
        }
        else if ty::type_is_bot(cond_ty) {
            fcx.write_bot(id);
        }
        else {
            fcx.write_nil(id);
        }
      }
      ast::ExprForLoop(..) =>
          fail!("non-desugared expr_for_loop"),
      ast::ExprLoop(body, _) => {
        check_block_no_value(fcx, (body));
        if !may_break(tcx, expr.id, body) {
            fcx.write_bot(id);
        }
        else {
            fcx.write_nil(id);
        }
      }
      ast::ExprMatch(discrim, ref arms) => {
        _match::check_match(fcx, expr, discrim, arms.as_slice());
      }
      ast::ExprFnBlock(decl, body) => {
        let region = astconv::opt_ast_region_to_region(fcx,
                                                       fcx.infcx(),
                                                       expr.span,
                                                       &None);
        check_expr_fn(fcx,
                      expr,
                      ty::RegionTraitStore(region, ast::MutMutable),
                      decl,
                      body,
                      expected);
      }
      ast::ExprProc(decl, body) => {
        check_expr_fn(fcx,
                      expr,
                      ty::UniqTraitStore,
                      decl,
                      body,
                      expected);
      }
      ast::ExprBlock(b) => {
        check_block_with_expected(fcx, b, expected);
        fcx.write_ty(id, fcx.node_ty(b.id));
      }
      ast::ExprCall(f, ref args) => {
          check_call(fcx, expr, f, args.as_slice());
          let f_ty = fcx.expr_ty(f);
          let (args_bot, args_err) = args.iter().fold((false, false),
             |(rest_bot, rest_err), a| {
                 // is this not working?
                 let a_ty = fcx.expr_ty(*a);
                 (rest_bot || ty::type_is_bot(a_ty),
                  rest_err || ty::type_is_error(a_ty))});
          if ty::type_is_error(f_ty) || args_err {
              fcx.write_error(id);
          }
          else if ty::type_is_bot(f_ty) || args_bot {
              fcx.write_bot(id);
          }
      }
      ast::ExprMethodCall(ident, ref tps, ref args) => {
        check_method_call(fcx, expr, ident, args.as_slice(), tps.as_slice());
        let mut arg_tys = args.iter().map(|a| fcx.expr_ty(*a));
        let (args_bot, args_err) = arg_tys.fold((false, false),
             |(rest_bot, rest_err), a| {
              (rest_bot || ty::type_is_bot(a),
               rest_err || ty::type_is_error(a))});
        if args_err {
            fcx.write_error(id);
        } else if args_bot {
            fcx.write_bot(id);
        }
      }
      ast::ExprCast(e, t) => {
        check_expr(fcx, e);
        let t_1 = fcx.to_ty(t);
        let t_e = fcx.expr_ty(e);

        debug!("t_1={}", fcx.infcx().ty_to_str(t_1));
        debug!("t_e={}", fcx.infcx().ty_to_str(t_e));

        if ty::type_is_error(t_e) {
            fcx.write_error(id);
        }
        else if ty::type_is_bot(t_e) {
            fcx.write_bot(id);
        }
        else {
            match ty::get(t_1).sty {
                // This will be looked up later on
                ty::ty_trait(..) => (),

                _ => {
                    if ty::type_is_nil(t_e) {
                        fcx.type_error_message(expr.span, |actual| {
                            format!("cast from nil: `{}` as `{}`",
                                    actual,
                                    fcx.infcx().ty_to_str(t_1))
                        }, t_e, None);
                    } else if ty::type_is_nil(t_1) {
                        fcx.type_error_message(expr.span, |actual| {
                            format!("cast to nil: `{}` as `{}`",
                                    actual,
                                    fcx.infcx().ty_to_str(t_1))
                        }, t_e, None);
                    }

                    let t1 = structurally_resolved_type(fcx, e.span, t_1);
                    let te = structurally_resolved_type(fcx, e.span, t_e);
                    let t_1_is_scalar = type_is_scalar(fcx, expr.span, t_1);
                    let t_1_is_char = type_is_char(fcx, expr.span, t_1);
                    let t_1_is_bare_fn = type_is_bare_fn(fcx, expr.span, t_1);

                    // casts to scalars other than `char` and `bare fn` are trivial
                    let t_1_is_trivial = t_1_is_scalar &&
                        !t_1_is_char && !t_1_is_bare_fn;

                    if type_is_c_like_enum(fcx, expr.span, t_e) && t_1_is_trivial {
                        // casts from C-like enums are allowed
                    } else if t_1_is_char {
                        let te = fcx.infcx().resolve_type_vars_if_possible(te);
                        if ty::get(te).sty != ty::ty_uint(ast::TyU8) {
                            fcx.type_error_message(expr.span, |actual| {
                                format!("only `u8` can be cast as \
                                         `char`, not `{}`", actual)
                            }, t_e, None);
                        }
                    } else if ty::get(t1).sty == ty::ty_bool {
                        fcx.tcx()
                           .sess
                           .span_err(expr.span,
                                     "cannot cast as `bool`, compare with \
                                      zero instead");
                    } else if type_is_region_ptr(fcx, expr.span, t_e) &&
                        type_is_unsafe_ptr(fcx, expr.span, t_1) {

                        fn is_vec(t: ty::t) -> bool {
                            match ty::get(t).sty {
                                ty::ty_vec(..) => true,
                                ty::ty_ptr(ty::mt{ty: t, ..}) | ty::ty_rptr(_, ty::mt{ty: t, ..}) |
                                ty::ty_box(t) | ty::ty_uniq(t) => match ty::get(t).sty {
                                    ty::ty_vec(_, None) => true,
                                    _ => false,
                                },
                                _ => false
                            }
                        }
                        fn types_compatible(fcx: &FnCtxt, sp: Span,
                                            t1: ty::t, t2: ty::t) -> bool {
                            if !is_vec(t1) {
                                false
                            } else {
                                let el = ty::sequence_element_type(fcx.tcx(),
                                                                   t1);
                                infer::mk_eqty(fcx.infcx(), false,
                                               infer::Misc(sp), el, t2).is_ok()
                            }
                        }

                        // Due to the limitations of LLVM global constants,
                        // region pointers end up pointing at copies of
                        // vector elements instead of the original values.
                        // To allow unsafe pointers to work correctly, we
                        // need to special-case obtaining an unsafe pointer
                        // from a region pointer to a vector.

                        /* this cast is only allowed from &[T] to *T or
                        &T to *T. */
                        match (&ty::get(te).sty, &ty::get(t_1).sty) {
                            (&ty::ty_rptr(_, mt1), &ty::ty_ptr(mt2))
                            if types_compatible(fcx, e.span,
                                                mt1.ty, mt2.ty) => {
                                /* this case is allowed */
                            }
                            _ => {
                                demand::coerce(fcx, e.span, t_1, e);
                            }
                        }
                    } else if !(type_is_scalar(fcx,expr.span,t_e)
                                && t_1_is_trivial) {
                        /*
                        If more type combinations should be supported than are
                        supported here, then file an enhancement issue and
                        record the issue number in this comment.
                        */
                        fcx.type_error_message(expr.span, |actual| {
                            format!("non-scalar cast: `{}` as `{}`",
                                    actual,
                                    fcx.infcx().ty_to_str(t_1))
                        }, t_e, None);
                    }
                }
            }
            fcx.write_ty(id, t_1);
        }
      }
      ast::ExprVec(ref args) => {
        let t: ty::t = fcx.infcx().next_ty_var();
        for e in args.iter() {
            check_expr_has_type(fcx, *e, t);
        }
        let typ = ty::mk_vec(tcx, ty::mt {ty: t, mutbl: ast::MutImmutable},
                             Some(args.len()));
        fcx.write_ty(id, typ);
      }
      ast::ExprRepeat(element, count_expr) => {
        check_expr_with_hint(fcx, count_expr, ty::mk_uint());
        let count = ty::eval_repeat_count(fcx, count_expr);
        let t: ty::t = fcx.infcx().next_ty_var();
        check_expr_has_type(fcx, element, t);
        let element_ty = fcx.expr_ty(element);
        if ty::type_is_error(element_ty) {
            fcx.write_error(id);
        }
        else if ty::type_is_bot(element_ty) {
            fcx.write_bot(id);
        }
        else {
            let t = ty::mk_vec(tcx, ty::mt {ty: t, mutbl: ast::MutImmutable},
                               Some(count));
            fcx.write_ty(id, t);
        }
      }
      ast::ExprTup(ref elts) => {
        let flds = unpack_expected(fcx, expected, |sty| {
            match *sty {
                ty::ty_tup(ref flds) => Some((*flds).clone()),
                _ => None
            }
        });
        let mut bot_field = false;
        let mut err_field = false;

        let elt_ts = elts.iter().enumerate().map(|(i, e)| {
            let opt_hint = match flds {
                Some(ref fs) if i < fs.len() => Some(*fs.get(i)),
                _ => None
            };
            check_expr_with_opt_hint(fcx, *e, opt_hint);
            let t = fcx.expr_ty(*e);
            err_field = err_field || ty::type_is_error(t);
            bot_field = bot_field || ty::type_is_bot(t);
            t
        }).collect();
        if bot_field {
            fcx.write_bot(id);
        } else if err_field {
            fcx.write_error(id);
        } else {
            let typ = ty::mk_tup(tcx, elt_ts);
            fcx.write_ty(id, typ);
        }
      }
      ast::ExprStruct(ref path, ref fields, base_expr) => {
        // Resolve the path.
        let def = tcx.def_map.borrow().find(&id).map(|i| *i);
        match def {
            Some(def::DefStruct(type_def_id)) => {
                check_struct_constructor(fcx, id, expr.span, type_def_id,
                                         fields.as_slice(), base_expr);
            }
            Some(def::DefVariant(enum_id, variant_id, _)) => {
                check_struct_enum_variant(fcx, id, expr.span, enum_id,
                                          variant_id, fields.as_slice());
            }
            _ => {
                tcx.sess.span_bug(path.span,
                                  "structure constructor does not name a structure type");
            }
        }
      }
      ast::ExprField(base, field, ref tys) => {
        check_field(fcx, expr, lvalue_pref, base, field.name, tys.as_slice());
      }
      ast::ExprIndex(base, idx) => {
          check_expr_with_lvalue_pref(fcx, base, lvalue_pref);
          check_expr(fcx, idx);
          let raw_base_t = fcx.expr_ty(base);
          let idx_t = fcx.expr_ty(idx);
          if ty::type_is_error(raw_base_t) || ty::type_is_bot(raw_base_t) {
              fcx.write_ty(id, raw_base_t);
          } else if ty::type_is_error(idx_t) || ty::type_is_bot(idx_t) {
              fcx.write_ty(id, idx_t);
          } else {
              let (base_t, autoderefs, field_ty) =
                autoderef(fcx, expr.span, raw_base_t, Some(base.id),
                          lvalue_pref, |base_t, _| ty::index(base_t));
              match field_ty {
                  Some(mt) => {
                      check_expr_has_type(fcx, idx, ty::mk_uint());
                      fcx.write_ty(id, mt.ty);
                      fcx.write_autoderef_adjustment(base.id, autoderefs);
                  }
                  None => {
                      let resolved = structurally_resolved_type(fcx,
                                                                expr.span,
                                                                raw_base_t);
                      let ret_ty = lookup_op_method(fcx,
                                                    expr,
                                                    resolved,
                                                    token::intern("index"),
                                                    tcx.lang_items.index_trait(),
                                                    [base, idx],
                                                    AutoderefReceiver,
                                                    || {
                        fcx.type_error_message(expr.span,
                                               |actual| {
                                                    format!("cannot index a \
                                                             value of type \
                                                             `{}`", actual)
                                               },
                                               base_t,
                                               None);
                      });
                      fcx.write_ty(id, ret_ty);
                  }
              }
          }
       }
    }

    debug!("type of expr({}) {} is...", expr.id,
           syntax::print::pprust::expr_to_str(expr));
    debug!("... {}, expected is {}",
           ppaux::ty_to_str(tcx, fcx.expr_ty(expr)),
           match expected {
               Some(t) => ppaux::ty_to_str(tcx, t),
               _ => "empty".to_string()
           });

    unifier();
}

pub fn require_uint(fcx: &FnCtxt, sp: Span, t: ty::t) {
    if !type_is_uint(fcx, sp, t) {
        fcx.type_error_message(sp, |actual| {
            format!("mismatched types: expected `uint` type but found `{}`",
                    actual)
        }, t, None);
    }
}

pub fn require_integral(fcx: &FnCtxt, sp: Span, t: ty::t) {
    if !type_is_integral(fcx, sp, t) {
        fcx.type_error_message(sp, |actual| {
            format!("mismatched types: expected integral type but found `{}`",
                    actual)
        }, t, None);
    }
}

pub fn check_decl_initializer(fcx: &FnCtxt,
                              nid: ast::NodeId,
                              init: &ast::Expr)
                            {
    let local_ty = fcx.local_ty(init.span, nid);
    check_expr_coercable_to_type(fcx, init, local_ty)
}

pub fn check_decl_local(fcx: &FnCtxt, local: &ast::Local)  {
    let tcx = fcx.ccx.tcx;

    let t = fcx.local_ty(local.span, local.id);
    fcx.write_ty(local.id, t);

    match local.init {
        Some(init) => {
            check_decl_initializer(fcx, local.id, init);
            let init_ty = fcx.expr_ty(init);
            if ty::type_is_error(init_ty) || ty::type_is_bot(init_ty) {
                fcx.write_ty(local.id, init_ty);
            }
        }
        _ => {}
    }

    let pcx = pat_ctxt {
        fcx: fcx,
        map: pat_id_map(&tcx.def_map, local.pat),
    };
    _match::check_pat(&pcx, local.pat, t);
    let pat_ty = fcx.node_ty(local.pat.id);
    if ty::type_is_error(pat_ty) || ty::type_is_bot(pat_ty) {
        fcx.write_ty(local.id, pat_ty);
    }
}

pub fn check_stmt(fcx: &FnCtxt, stmt: &ast::Stmt)  {
    let node_id;
    let mut saw_bot = false;
    let mut saw_err = false;
    match stmt.node {
      ast::StmtDecl(decl, id) => {
        node_id = id;
        match decl.node {
          ast::DeclLocal(ref l) => {
              check_decl_local(fcx, *l);
              let l_t = fcx.node_ty(l.id);
              saw_bot = saw_bot || ty::type_is_bot(l_t);
              saw_err = saw_err || ty::type_is_error(l_t);
          }
          ast::DeclItem(_) => {/* ignore for now */ }
        }
      }
      ast::StmtExpr(expr, id) => {
        node_id = id;
        // Check with expected type of ()
        check_expr_has_type(fcx, expr, ty::mk_nil());
        let expr_ty = fcx.expr_ty(expr);
        saw_bot = saw_bot || ty::type_is_bot(expr_ty);
        saw_err = saw_err || ty::type_is_error(expr_ty);
      }
      ast::StmtSemi(expr, id) => {
        node_id = id;
        check_expr(fcx, expr);
        let expr_ty = fcx.expr_ty(expr);
        saw_bot |= ty::type_is_bot(expr_ty);
        saw_err |= ty::type_is_error(expr_ty);
      }
      ast::StmtMac(..) => fcx.ccx.tcx.sess.bug("unexpanded macro")
    }
    if saw_bot {
        fcx.write_bot(node_id);
    }
    else if saw_err {
        fcx.write_error(node_id);
    }
    else {
        fcx.write_nil(node_id)
    }
}

pub fn check_block_no_value(fcx: &FnCtxt, blk: &ast::Block)  {
    check_block_with_expected(fcx, blk, Some(ty::mk_nil()));
    let blkty = fcx.node_ty(blk.id);
    if ty::type_is_error(blkty) {
        fcx.write_error(blk.id);
    }
    else if ty::type_is_bot(blkty) {
        fcx.write_bot(blk.id);
    }
    else {
        let nilty = ty::mk_nil();
        demand::suptype(fcx, blk.span, nilty, blkty);
    }
}

pub fn check_block_with_expected(fcx: &FnCtxt,
                                 blk: &ast::Block,
                                 expected: Option<ty::t>) {
    let prev = {
        let mut fcx_ps = fcx.ps.borrow_mut();
        let fn_style_state = fcx_ps.recurse(blk);
        replace(&mut *fcx_ps, fn_style_state)
    };

    fcx.with_region_lb(blk.id, || {
        let mut warned = false;
        let mut last_was_bot = false;
        let mut any_bot = false;
        let mut any_err = false;
        for s in blk.stmts.iter() {
            check_stmt(fcx, *s);
            let s_id = ast_util::stmt_id(*s);
            let s_ty = fcx.node_ty(s_id);
            if last_was_bot && !warned && match s.node {
                  ast::StmtDecl(decl, _) => {
                      match decl.node {
                          ast::DeclLocal(_) => true,
                          _ => false,
                      }
                  }
                  ast::StmtExpr(_, _) | ast::StmtSemi(_, _) => true,
                  _ => false
                } {
                fcx.ccx
                   .tcx
                   .sess
                   .add_lint(UnreachableCode,
                             s_id,
                             s.span,
                             "unreachable statement".to_string());
                warned = true;
            }
            if ty::type_is_bot(s_ty) {
                last_was_bot = true;
            }
            any_bot = any_bot || ty::type_is_bot(s_ty);
            any_err = any_err || ty::type_is_error(s_ty);
        }
        match blk.expr {
            None => if any_err {
                fcx.write_error(blk.id);
            }
            else if any_bot {
                fcx.write_bot(blk.id);
            }
            else  {
                fcx.write_nil(blk.id);
            },
            Some(e) => {
                if any_bot && !warned {
                    fcx.ccx.tcx.sess
                       .add_lint(UnreachableCode,
                                 e.id,
                                 e.span,
                                 "unreachable expression".to_string());
                }
                let ety = match expected {
                    Some(ety) => {
                        check_expr_coercable_to_type(fcx, e, ety);
                        ety
                    },
                    None => {
                        check_expr(fcx, e);
                        fcx.expr_ty(e)
                    }
                };
                fcx.write_ty(blk.id, ety);
                if any_err {
                    fcx.write_error(blk.id);
                } else if any_bot {
                    fcx.write_bot(blk.id);
                }
            }
        };
    });

    *fcx.ps.borrow_mut() = prev;
}

pub fn check_const(ccx: &CrateCtxt,
                   sp: Span,
                   e: &ast::Expr,
                   id: ast::NodeId) {
    let inh = blank_inherited_fields(ccx);
    let rty = ty::node_id_to_type(ccx.tcx, id);
    let fcx = blank_fn_ctxt(ccx, &inh, rty, e.id);
    let declty = fcx.ccx.tcx.tcache.borrow().get(&local_def(id)).ty;
    check_const_with_ty(&fcx, sp, e, declty);
}

pub fn check_const_with_ty(fcx: &FnCtxt,
                           _: Span,
                           e: &ast::Expr,
                           declty: ty::t) {
    // Gather locals in statics (because of block expressions).
    // This is technically uneccessary because locals in static items are forbidden,
    // but prevents type checking from blowing up before const checking can properly
    // emit a error.
    GatherLocalsVisitor { fcx: fcx }.visit_expr(e, ());

    check_expr(fcx, e);
    let cty = fcx.expr_ty(e);
    demand::suptype(fcx, e.span, declty, cty);
    regionck::regionck_expr(fcx, e);
    writeback::resolve_type_vars_in_expr(fcx, e);
}

/// Checks whether a type can be represented in memory. In particular, it
/// identifies types that contain themselves without indirection through a
/// pointer, which would mean their size is unbounded. This is different from
/// the question of whether a type can be instantiated. See the definition of
/// `check_instantiable`.
pub fn check_representable(tcx: &ty::ctxt,
                           sp: Span,
                           item_id: ast::NodeId,
                           designation: &str) -> bool {
    let rty = ty::node_id_to_type(tcx, item_id);

    // Check that it is possible to represent this type. This call identifies
    // (1) types that contain themselves and (2) types that contain a different
    // recursive type. It is only necessary to throw an error on those that
    // contain themselves. For case 2, there must be an inner type that will be
    // caught by case 1.
    match ty::is_type_representable(tcx, sp, rty) {
      ty::SelfRecursive => {
        tcx.sess.span_err(
          sp, format!("illegal recursive {} type; \
                       wrap the inner value in a box to make it representable",
                      designation).as_slice());
        return false
      }
      ty::Representable | ty::ContainsRecursive => (),
    }
    return true
}

/// Checks whether a type can be created without an instance of itself.
/// This is similar but different from the question of whether a type
/// can be represented.  For example, the following type:
///
///     enum foo { None, Some(foo) }
///
/// is instantiable but is not representable.  Similarly, the type
///
///     enum foo { Some(@foo) }
///
/// is representable, but not instantiable.
pub fn check_instantiable(tcx: &ty::ctxt,
                          sp: Span,
                          item_id: ast::NodeId)
                          -> bool {
    let item_ty = ty::node_id_to_type(tcx, item_id);
    if !ty::is_instantiable(tcx, item_ty) {
        tcx.sess
           .span_err(sp,
                     format!("this type cannot be instantiated without an \
                              instance of itself; consider using \
                              `Option<{}>`",
                             ppaux::ty_to_str(tcx, item_ty)).as_slice());
        false
    } else {
        true
    }
}

pub fn check_simd(tcx: &ty::ctxt, sp: Span, id: ast::NodeId) {
    let t = ty::node_id_to_type(tcx, id);
    if ty::type_needs_subst(t) {
        tcx.sess.span_err(sp, "SIMD vector cannot be generic");
        return;
    }
    match ty::get(t).sty {
        ty::ty_struct(did, ref substs) => {
            let fields = ty::lookup_struct_fields(tcx, did);
            if fields.is_empty() {
                tcx.sess.span_err(sp, "SIMD vector cannot be empty");
                return;
            }
            let e = ty::lookup_field_type(tcx, did, fields.get(0).id, substs);
            if !fields.iter().all(
                         |f| ty::lookup_field_type(tcx, did, f.id, substs) == e) {
                tcx.sess.span_err(sp, "SIMD vector should be homogeneous");
                return;
            }
            if !ty::type_is_machine(e) {
                tcx.sess.span_err(sp, "SIMD vector element type should be \
                                       machine type");
                return;
            }
        }
        _ => ()
    }
}

pub fn check_enum_variants_sized(ccx: &CrateCtxt,
                                 vs: &[ast::P<ast::Variant>]) {
    for &v in vs.iter() {
        match v.node.kind {
            ast::TupleVariantKind(ref args) if args.len() > 0 => {
                let ctor_ty = ty::node_id_to_type(ccx.tcx, v.node.id);
                let arg_tys: Vec<ty::t> = ty::ty_fn_args(ctor_ty).iter().map(|a| *a).collect();
                let len = arg_tys.len();
                if len == 0 {
                    return;
                }
                for (i, t) in arg_tys.slice_to(len - 1).iter().enumerate() {
                    // Allow the last field in an enum to be unsized.
                    // We want to do this so that we can support smart pointers.
                    // A struct value with an unsized final field is itself
                    // unsized and we must track this in the type system.
                    if !ty::type_is_sized(ccx.tcx, *t) {
                        ccx.tcx
                           .sess
                           .span_err(
                               args.get(i).ty.span,
                               format!("type `{}` is dynamically sized. \
                                        dynamically sized types may only \
                                        appear as the final type in a \
                                        variant",
                                       ppaux::ty_to_str(ccx.tcx,
                                                        *t)).as_slice());
                    }
                }
            },
            ast::StructVariantKind(struct_def) => check_fields_sized(ccx.tcx, struct_def),
            _ => {}
        }
    }
}

pub fn check_enum_variants(ccx: &CrateCtxt,
                           sp: Span,
                           vs: &[ast::P<ast::Variant>],
                           id: ast::NodeId) {

    fn disr_in_range(ccx: &CrateCtxt,
                     ty: attr::IntType,
                     disr: ty::Disr) -> bool {
        fn uint_in_range(ccx: &CrateCtxt, ty: ast::UintTy, disr: ty::Disr) -> bool {
            match ty {
                ast::TyU8 => disr as u8 as Disr == disr,
                ast::TyU16 => disr as u16 as Disr == disr,
                ast::TyU32 => disr as u32 as Disr == disr,
                ast::TyU64 => disr as u64 as Disr == disr,
                ast::TyU => uint_in_range(ccx, ccx.tcx.sess.targ_cfg.uint_type, disr)
            }
        }
        fn int_in_range(ccx: &CrateCtxt, ty: ast::IntTy, disr: ty::Disr) -> bool {
            match ty {
                ast::TyI8 => disr as i8 as Disr == disr,
                ast::TyI16 => disr as i16 as Disr == disr,
                ast::TyI32 => disr as i32 as Disr == disr,
                ast::TyI64 => disr as i64 as Disr == disr,
                ast::TyI => int_in_range(ccx, ccx.tcx.sess.targ_cfg.int_type, disr)
            }
        }
        match ty {
            attr::UnsignedInt(ty) => uint_in_range(ccx, ty, disr),
            attr::SignedInt(ty) => int_in_range(ccx, ty, disr)
        }
    }

    fn do_check(ccx: &CrateCtxt,
                vs: &[ast::P<ast::Variant>],
                id: ast::NodeId,
                hint: attr::ReprAttr)
                -> Vec<Rc<ty::VariantInfo>> {

        let rty = ty::node_id_to_type(ccx.tcx, id);
        let mut variants: Vec<Rc<ty::VariantInfo>> = Vec::new();
        let mut disr_vals: Vec<ty::Disr> = Vec::new();
        let mut prev_disr_val: Option<ty::Disr> = None;

        for &v in vs.iter() {

            // If the discriminant value is specified explicitly in the enum check whether the
            // initialization expression is valid, otherwise use the last value plus one.
            let mut current_disr_val = match prev_disr_val {
                Some(prev_disr_val) => prev_disr_val + 1,
                None => ty::INITIAL_DISCRIMINANT_VALUE
            };

            match v.node.disr_expr {
                Some(e) => {
                    debug!("disr expr, checking {}", pprust::expr_to_str(e));

                    let inh = blank_inherited_fields(ccx);
                    let fcx = blank_fn_ctxt(ccx, &inh, rty, e.id);
                    let declty = ty::mk_int_var(ccx.tcx, fcx.infcx().next_int_var_id());
                    check_const_with_ty(&fcx, e.span, e, declty);
                    // check_expr (from check_const pass) doesn't guarantee
                    // that the expression is in a form that eval_const_expr can
                    // handle, so we may still get an internal compiler error

                    match const_eval::eval_const_expr_partial(ccx.tcx, e) {
                        Ok(const_eval::const_int(val)) => current_disr_val = val as Disr,
                        Ok(const_eval::const_uint(val)) => current_disr_val = val as Disr,
                        Ok(_) => {
                            ccx.tcx.sess.span_err(e.span, "expected signed integer constant");
                        }
                        Err(ref err) => {
                            ccx.tcx
                               .sess
                               .span_err(e.span,
                                         format!("expected constant: {}",
                                                 *err).as_slice());
                        }
                    }
                },
                None => ()
            };

            // Check for duplicate discriminant values
            if disr_vals.contains(&current_disr_val) {
                ccx.tcx.sess.span_err(v.span, "discriminant value already exists");
            }
            // Check for unrepresentable discriminant values
            match hint {
                attr::ReprAny | attr::ReprExtern => (),
                attr::ReprInt(sp, ity) => {
                    if !disr_in_range(ccx, ity, current_disr_val) {
                        ccx.tcx.sess.span_err(v.span,
                                              "discriminant value outside specified type");
                        ccx.tcx.sess.span_note(sp, "discriminant type specified here");
                    }
                }
            }
            disr_vals.push(current_disr_val);

            let variant_info = Rc::new(VariantInfo::from_ast_variant(ccx.tcx, v,
                                                                     current_disr_val));
            prev_disr_val = Some(current_disr_val);

            variants.push(variant_info);
        }

        return variants;
    }

    let hint = ty::lookup_repr_hint(ccx.tcx, ast::DefId { krate: ast::LOCAL_CRATE, node: id });
    if hint != attr::ReprAny && vs.len() <= 1 {
        let msg = if vs.len() == 1 {
            "unsupported representation for univariant enum"
        } else {
            "unsupported representation for zero-variant enum"
        };
        ccx.tcx.sess.span_err(sp, msg)
    }

    let variants = do_check(ccx, vs, id, hint);

    // cache so that ty::enum_variants won't repeat this work
    ccx.tcx.enum_var_cache.borrow_mut().insert(local_def(id), Rc::new(variants));

    check_representable(ccx.tcx, sp, id, "enum");

    // Check that it is possible to instantiate this enum:
    //
    // This *sounds* like the same that as representable, but it's
    // not.  See def'n of `check_instantiable()` for details.
    check_instantiable(ccx.tcx, sp, id);
}

pub fn lookup_def(fcx: &FnCtxt, sp: Span, id: ast::NodeId) -> def::Def {
    lookup_def_ccx(fcx.ccx, sp, id)
}

// Returns the type parameter count and the type for the given definition.
pub fn ty_param_bounds_and_ty_for_def(fcx: &FnCtxt,
                                      sp: Span,
                                      defn: def::Def)
                                   -> ty_param_bounds_and_ty {
    match defn {
      def::DefArg(nid, _) | def::DefLocal(nid, _) |
      def::DefBinding(nid, _) => {
          let typ = fcx.local_ty(sp, nid);
          return no_params(typ);
      }
      def::DefFn(id, _) | def::DefStaticMethod(id, _, _) |
      def::DefStatic(id, _) | def::DefVariant(_, id, _) |
      def::DefStruct(id) => {
        return ty::lookup_item_type(fcx.ccx.tcx, id);
      }
      def::DefUpvar(_, inner, _, _) => {
        return ty_param_bounds_and_ty_for_def(fcx, sp, *inner);
      }
      def::DefTrait(_) |
      def::DefTy(_) |
      def::DefPrimTy(_) |
      def::DefTyParam(..)=> {
        fcx.ccx.tcx.sess.span_bug(sp, "expected value but found type");
      }
      def::DefMod(..) | def::DefForeignMod(..) => {
        fcx.ccx.tcx.sess.span_bug(sp, "expected value but found module");
      }
      def::DefUse(..) => {
        fcx.ccx.tcx.sess.span_bug(sp, "expected value but found use");
      }
      def::DefRegion(..) => {
        fcx.ccx.tcx.sess.span_bug(sp, "expected value but found region");
      }
      def::DefTyParamBinder(..) => {
        fcx.ccx.tcx.sess.span_bug(sp, "expected value but found type parameter");
      }
      def::DefLabel(..) => {
        fcx.ccx.tcx.sess.span_bug(sp, "expected value but found label");
      }
      def::DefSelfTy(..) => {
        fcx.ccx.tcx.sess.span_bug(sp, "expected value but found self ty");
      }
      def::DefMethod(..) => {
        fcx.ccx.tcx.sess.span_bug(sp, "expected value but found method");
      }
    }
}

// Instantiates the given path, which must refer to an item with the given
// number of type parameters and type.
pub fn instantiate_path(fcx: &FnCtxt,
                        pth: &ast::Path,
                        tpt: ty_param_bounds_and_ty,
                        def: def::Def,
                        span: Span,
                        node_id: ast::NodeId) {
    debug!(">>> instantiate_path");

    let ty_param_count = tpt.generics.type_param_defs().len();
    let ty_param_req = tpt.generics.type_param_defs().iter()
                                                   .take_while(|x| x.default.is_none())
                                                   .count();
    let mut ty_substs_len = 0;
    for segment in pth.segments.iter() {
        ty_substs_len += segment.types.len()
    }

    debug!("tpt={} ty_param_count={:?} ty_substs_len={:?}",
           tpt.repr(fcx.tcx()),
           ty_param_count,
           ty_substs_len);

    // determine the region parameters, using the value given by the user
    // (if any) and otherwise using a fresh region variable
    let num_expected_regions = tpt.generics.region_param_defs().len();
    let num_supplied_regions = pth.segments.last().unwrap().lifetimes.len();
    let regions = if num_expected_regions == num_supplied_regions {
        pth.segments.last().unwrap().lifetimes
            .iter()
            .map(|l| ast_region_to_region(fcx.tcx(), l))
            .collect()
    } else {
        if num_supplied_regions != 0 {
            fcx.ccx.tcx.sess.span_err(
                span,
                format!("expected {nexpected, plural, =1{# lifetime parameter} \
                                                   other{# lifetime parameters}}, \
                         found {nsupplied, plural, =1{# lifetime parameter} \
                                                other{# lifetime parameters}}",
                        nexpected = num_expected_regions,
                        nsupplied = num_supplied_regions).as_slice());
        }

        fcx.infcx().region_vars_for_defs(span, tpt.generics.region_param_defs.as_slice())
    };
    let regions = subst::NonerasedRegions(regions);

    // Special case: If there is a self parameter, omit it from the list of
    // type parameters.
    //
    // Here we calculate the "user type parameter count", which is the number
    // of type parameters actually manifest in the AST. This will differ from
    // the internal type parameter count when there are self types involved.
    let (user_ty_param_count, user_ty_param_req, self_parameter_index) = match def {
        def::DefStaticMethod(_, provenance @ def::FromTrait(_), _) => {
            let generics = generics_of_static_method_container(fcx.ccx.tcx,
                                                               provenance);
            (ty_param_count - 1, ty_param_req - 1, Some(generics.type_param_defs().len()))
        }
        _ => (ty_param_count, ty_param_req, None),
    };

    // determine values for type parameters, using the values given by
    // the user (if any) and otherwise using fresh type variables
    let (tps, regions) = if ty_substs_len == 0 {
        (fcx.infcx().next_ty_vars(ty_param_count), regions)
    } else if ty_param_count == 0 {
        fcx.ccx.tcx.sess.span_err
            (span, "this item does not take type parameters");
        (fcx.infcx().next_ty_vars(ty_param_count), regions)
    } else if ty_substs_len > user_ty_param_count {
        let expected = if user_ty_param_req < user_ty_param_count {
            "expected at most"
        } else {
            "expected"
        };
        fcx.ccx.tcx.sess.span_err
            (span,
             format!("too many type parameters provided: {} {}, found {}",
                  expected, user_ty_param_count, ty_substs_len).as_slice());
        (fcx.infcx().next_ty_vars(ty_param_count), regions)
    } else if ty_substs_len < user_ty_param_req {
        let expected = if user_ty_param_req < user_ty_param_count {
            "expected at least"
        } else {
            "expected"
        };
        fcx.ccx.tcx.sess.span_err(
            span,
            format!("not enough type parameters provided: {} {}, found {}",
                    expected,
                    user_ty_param_req,
                    ty_substs_len).as_slice());
        (fcx.infcx().next_ty_vars(ty_param_count), regions)
    } else {
        if ty_substs_len > user_ty_param_req
            && !fcx.tcx().sess.features.default_type_params.get() {
            fcx.tcx().sess.span_err(pth.span, "default type parameters are \
                                               experimental and possibly buggy");
            fcx.tcx().sess.span_note(pth.span, "add #![feature(default_type_params)] \
                                                to the crate attributes to enable");
        }

        // Build up the list of type parameters, inserting the self parameter
        // at the appropriate position.
        let mut tps = Vec::new();
        let mut pushed = false;
        for (i, ty) in pth.segments.iter()
                                   .flat_map(|segment| segment.types.iter())
                                   .map(|&ast_type| fcx.to_ty(ast_type))
                                   .enumerate() {
            match self_parameter_index {
                Some(index) if index == i => {
                    tps.push(*fcx.infcx().next_ty_vars(1).get(0));
                    pushed = true;
                }
                _ => {}
            }
            tps.push(ty)
        }

        let mut substs = subst::Substs {
            regions: regions,
            self_ty: None,
            tps: tps
        };

        let defaults = tpt.generics.type_param_defs().iter()
                          .enumerate().filter_map(|(i, x)| {
            match self_parameter_index {
                Some(index) if index == i => None,
                _ => Some(x.default)
            }
        });
        for (i, default) in defaults.skip(ty_substs_len).enumerate() {
            match self_parameter_index {
                Some(index) if index == i + ty_substs_len => {
                    substs.tps.push(*fcx.infcx().next_ty_vars(1).get(0));
                    pushed = true;
                }
                _ => {}
            }
            match default {
                Some(default) => {
                    let ty = default.subst_spanned(fcx.tcx(), &substs, Some(span));
                    substs.tps.push(ty);
                }
                None => {
                    fcx.tcx().sess.span_bug(span,
                        "missing default for a not explicitely provided type param")
                }
            }
        }

        // If the self parameter goes at the end, insert it there.
        if !pushed && self_parameter_index.is_some() {
            substs.tps.push(*fcx.infcx().next_ty_vars(1).get(0))
        }

        assert_eq!(substs.tps.len(), ty_param_count)

        let subst::Substs {tps, regions, ..} = substs;
        (tps, regions)
    };

    let substs = subst::Substs { regions: regions,
                                 self_ty: None,
                                 tps: tps };

    fcx.write_ty_substs(node_id, tpt.ty, ty::ItemSubsts {
        substs: substs,
    });

    debug!("<<<");
}

// Resolves `typ` by a single level if `typ` is a type variable.  If no
// resolution is possible, then an error is reported.
pub fn structurally_resolved_type(fcx: &FnCtxt, sp: Span, tp: ty::t) -> ty::t {
    match infer::resolve_type(fcx.infcx(), tp, force_tvar) {
        Ok(t_s) if !ty::type_is_ty_var(t_s) => t_s,
        _ => {
            fcx.type_error_message(sp, |_actual| {
                "the type of this value must be known in this \
                 context".to_string()
            }, tp, None);
            demand::suptype(fcx, sp, ty::mk_err(), tp);
            tp
        }
    }
}

// Returns the one-level-deep structure of the given type.
pub fn structure_of<'a>(fcx: &FnCtxt, sp: Span, typ: ty::t)
                        -> &'a ty::sty {
    &ty::get(structurally_resolved_type(fcx, sp, typ)).sty
}

pub fn type_is_integral(fcx: &FnCtxt, sp: Span, typ: ty::t) -> bool {
    let typ_s = structurally_resolved_type(fcx, sp, typ);
    return ty::type_is_integral(typ_s);
}

pub fn type_is_uint(fcx: &FnCtxt, sp: Span, typ: ty::t) -> bool {
    let typ_s = structurally_resolved_type(fcx, sp, typ);
    return ty::type_is_uint(typ_s);
}

pub fn type_is_scalar(fcx: &FnCtxt, sp: Span, typ: ty::t) -> bool {
    let typ_s = structurally_resolved_type(fcx, sp, typ);
    return ty::type_is_scalar(typ_s);
}

pub fn type_is_char(fcx: &FnCtxt, sp: Span, typ: ty::t) -> bool {
    let typ_s = structurally_resolved_type(fcx, sp, typ);
    return ty::type_is_char(typ_s);
}

pub fn type_is_bare_fn(fcx: &FnCtxt, sp: Span, typ: ty::t) -> bool {
    let typ_s = structurally_resolved_type(fcx, sp, typ);
    return ty::type_is_bare_fn(typ_s);
}

pub fn type_is_unsafe_ptr(fcx: &FnCtxt, sp: Span, typ: ty::t) -> bool {
    let typ_s = structurally_resolved_type(fcx, sp, typ);
    return ty::type_is_unsafe_ptr(typ_s);
}

pub fn type_is_region_ptr(fcx: &FnCtxt, sp: Span, typ: ty::t) -> bool {
    let typ_s = structurally_resolved_type(fcx, sp, typ);
    return ty::type_is_region_ptr(typ_s);
}

pub fn type_is_c_like_enum(fcx: &FnCtxt, sp: Span, typ: ty::t) -> bool {
    let typ_s = structurally_resolved_type(fcx, sp, typ);
    return ty::type_is_c_like_enum(fcx.ccx.tcx, typ_s);
}

pub fn ast_expr_vstore_to_ty(fcx: &FnCtxt,
                             e: &ast::Expr,
                             v: ast::ExprVstore,
                             mk_inner: || -> ty::mt)
                             -> ty::t {
    match v {
        ast::ExprVstoreUniq => ty::mk_uniq(fcx.ccx.tcx, mk_inner().ty),
        ast::ExprVstoreSlice | ast::ExprVstoreMutSlice => {
            match e.node {
                ast::ExprLit(..) => {
                    // string literals and *empty slices* live in static memory
                    ty::mk_rptr(fcx.ccx.tcx, ty::ReStatic, mk_inner())
                }
                ast::ExprVec(ref elements) if elements.len() == 0 => {
                    // string literals and *empty slices* live in static memory
                    ty::mk_rptr(fcx.ccx.tcx, ty::ReStatic, mk_inner())
                }
                ast::ExprRepeat(..) |
                ast::ExprVec(..) => {
                    // vector literals are temporaries on the stack
                    match fcx.tcx().region_maps.temporary_scope(e.id) {
                        Some(scope) => ty::mk_rptr(fcx.ccx.tcx, ty::ReScope(scope), mk_inner()),
                        None => ty::mk_rptr(fcx.ccx.tcx, ty::ReStatic, mk_inner()),
                    }
                }
                _ => {
                    fcx.ccx.tcx.sess.span_bug(e.span,
                                              "vstore with unexpected \
                                               contents")
                }
            }
        }
    }
}

// Returns true if b contains a break that can exit from b
pub fn may_break(cx: &ty::ctxt, id: ast::NodeId, b: ast::P<ast::Block>) -> bool {
    // First: is there an unlabeled break immediately
    // inside the loop?
    (loop_query(b, |e| {
        match *e {
            ast::ExprBreak(_) => true,
            _ => false
        }
    })) ||
   // Second: is there a labeled break with label
   // <id> nested anywhere inside the loop?
    (block_query(b, |e| {
        match e.node {
            ast::ExprBreak(Some(_)) => {
                match cx.def_map.borrow().find(&e.id) {
                    Some(&def::DefLabel(loop_id)) if id == loop_id => true,
                    _ => false,
                }
            }
            _ => false
        }}))
}

pub fn check_bounds_are_used(ccx: &CrateCtxt,
                             span: Span,
                             tps: &OwnedSlice<ast::TyParam>,
                             ty: ty::t) {
    debug!("check_bounds_are_used(n_tps={}, ty={})",
           tps.len(), ppaux::ty_to_str(ccx.tcx, ty));

    // make a vector of booleans initially false, set to true when used
    if tps.len() == 0u { return; }
    let mut tps_used = Vec::from_elem(tps.len(), false);

    ty::walk_ty(ty, |t| {
            match ty::get(t).sty {
                ty::ty_param(param_ty {idx, ..}) => {
                    debug!("Found use of ty param \\#{}", idx);
                    *tps_used.get_mut(idx) = true;
                }
                _ => ()
            }
        });

    for (i, b) in tps_used.iter().enumerate() {
        if !*b {
            ccx.tcx.sess.span_err(
                span,
                format!("type parameter `{}` is unused",
                        token::get_ident(tps.get(i).ident)).as_slice());
        }
    }
}

pub fn check_intrinsic_type(ccx: &CrateCtxt, it: &ast::ForeignItem) {
    fn param(ccx: &CrateCtxt, n: uint) -> ty::t {
        ty::mk_param(ccx.tcx, n, local_def(0))
    }

    let tcx = ccx.tcx;
    let name = token::get_ident(it.ident);
    let (n_tps, inputs, output) = if name.get().starts_with("atomic_") {
        let split : Vec<&str> = name.get().split('_').collect();
        assert!(split.len() >= 2, "Atomic intrinsic not correct format");

        //We only care about the operation here
        match *split.get(1) {
            "cxchg" => (1, vec!(ty::mk_mut_ptr(tcx, param(ccx, 0)),
                                param(ccx, 0),
                                param(ccx, 0)),
                        param(ccx, 0)),
            "load" => (1, vec!(ty::mk_imm_ptr(tcx, param(ccx, 0))),
                       param(ccx, 0)),
            "store" => (1, vec!(ty::mk_mut_ptr(tcx, param(ccx, 0)), param(ccx, 0)),
                        ty::mk_nil()),

            "xchg" | "xadd" | "xsub" | "and"  | "nand" | "or" | "xor" | "max" |
            "min"  | "umax" | "umin" => {
                (1, vec!(ty::mk_mut_ptr(tcx, param(ccx, 0)), param(ccx, 0)),
                 param(ccx, 0))
            }
            "fence" => {
                (0, Vec::new(), ty::mk_nil())
            }
            op => {
                tcx.sess.span_err(it.span,
                                  format!("unrecognized atomic operation \
                                           function: `{}`",
                                          op).as_slice());
                return;
            }
        }

    } else {
        match name.get() {
            "abort" => (0, Vec::new(), ty::mk_bot()),
            "breakpoint" => (0, Vec::new(), ty::mk_nil()),
            "size_of" |
            "pref_align_of" | "min_align_of" => (1u, Vec::new(), ty::mk_uint()),
            "init" => (1u, Vec::new(), param(ccx, 0u)),
            "uninit" => (1u, Vec::new(), param(ccx, 0u)),
            "forget" => (1u, vec!( param(ccx, 0) ), ty::mk_nil()),
            "transmute" => (2, vec!( param(ccx, 0) ), param(ccx, 1)),
            "move_val_init" => {
                (1u,
                 vec!(
                    ty::mk_mut_rptr(tcx, ty::ReLateBound(it.id, ty::BrAnon(0)), param(ccx, 0)),
                    param(ccx, 0u)
                  ),
               ty::mk_nil())
            }
            "needs_drop" => (1u, Vec::new(), ty::mk_bool()),
            "owns_managed" => (1u, Vec::new(), ty::mk_bool()),

            "get_tydesc" => {
              let tydesc_ty = match ty::get_tydesc_ty(ccx.tcx) {
                  Ok(t) => t,
                  Err(s) => { tcx.sess.span_fatal(it.span, s.as_slice()); }
              };
              let td_ptr = ty::mk_ptr(ccx.tcx, ty::mt {
                  ty: tydesc_ty,
                  mutbl: ast::MutImmutable
              });
              (1u, Vec::new(), td_ptr)
            }
            "type_id" => {
                let langid = ccx.tcx.lang_items.require(TypeIdLangItem);
                match langid {
                    Ok(did) => (1u, Vec::new(), ty::mk_struct(ccx.tcx, did, subst::Substs {
                                                 self_ty: None,
                                                 tps: Vec::new(),
                                                 regions: subst::NonerasedRegions(Vec::new())
                                                 }) ),
                    Err(msg) => {
                        tcx.sess.span_fatal(it.span, msg.as_slice());
                    }
                }
            },
            "visit_tydesc" => {
              let tydesc_ty = match ty::get_tydesc_ty(ccx.tcx) {
                  Ok(t) => t,
                  Err(s) => { tcx.sess.span_fatal(it.span, s.as_slice()); }
              };
              let region = ty::ReLateBound(it.id, ty::BrAnon(0));
              let visitor_object_ty = match ty::visitor_object_ty(tcx, region) {
                  Ok((_, vot)) => vot,
                  Err(s) => { tcx.sess.span_fatal(it.span, s.as_slice()); }
              };

              let td_ptr = ty::mk_ptr(ccx.tcx, ty::mt {
                  ty: tydesc_ty,
                  mutbl: ast::MutImmutable
              });
              (0, vec!( td_ptr, visitor_object_ty ), ty::mk_nil())
            }
            "offset" => {
              (1,
               vec!(
                  ty::mk_ptr(tcx, ty::mt {
                      ty: param(ccx, 0),
                      mutbl: ast::MutImmutable
                  }),
                  ty::mk_int()
               ),
               ty::mk_ptr(tcx, ty::mt {
                   ty: param(ccx, 0),
                   mutbl: ast::MutImmutable
               }))
            }
            "copy_memory" | "copy_nonoverlapping_memory" |
            "volatile_copy_memory" | "volatile_copy_nonoverlapping_memory" => {
              (1,
               vec!(
                  ty::mk_ptr(tcx, ty::mt {
                      ty: param(ccx, 0),
                      mutbl: ast::MutMutable
                  }),
                  ty::mk_ptr(tcx, ty::mt {
                      ty: param(ccx, 0),
                      mutbl: ast::MutImmutable
                  }),
                  ty::mk_uint()
               ),
               ty::mk_nil())
            }
            "set_memory" | "volatile_set_memory" => {
              (1,
               vec!(
                  ty::mk_ptr(tcx, ty::mt {
                      ty: param(ccx, 0),
                      mutbl: ast::MutMutable
                  }),
                  ty::mk_u8(),
                  ty::mk_uint()
               ),
               ty::mk_nil())
            }
            "sqrtf32" => (0, vec!( ty::mk_f32() ), ty::mk_f32()),
            "sqrtf64" => (0, vec!( ty::mk_f64() ), ty::mk_f64()),
            "powif32" => {
               (0,
                vec!( ty::mk_f32(), ty::mk_i32() ),
                ty::mk_f32())
            }
            "powif64" => {
               (0,
                vec!( ty::mk_f64(), ty::mk_i32() ),
                ty::mk_f64())
            }
            "sinf32" => (0, vec!( ty::mk_f32() ), ty::mk_f32()),
            "sinf64" => (0, vec!( ty::mk_f64() ), ty::mk_f64()),
            "cosf32" => (0, vec!( ty::mk_f32() ), ty::mk_f32()),
            "cosf64" => (0, vec!( ty::mk_f64() ), ty::mk_f64()),
            "powf32" => {
               (0,
                vec!( ty::mk_f32(), ty::mk_f32() ),
                ty::mk_f32())
            }
            "powf64" => {
               (0,
                vec!( ty::mk_f64(), ty::mk_f64() ),
                ty::mk_f64())
            }
            "expf32"   => (0, vec!( ty::mk_f32() ), ty::mk_f32()),
            "expf64"   => (0, vec!( ty::mk_f64() ), ty::mk_f64()),
            "exp2f32"  => (0, vec!( ty::mk_f32() ), ty::mk_f32()),
            "exp2f64"  => (0, vec!( ty::mk_f64() ), ty::mk_f64()),
            "logf32"   => (0, vec!( ty::mk_f32() ), ty::mk_f32()),
            "logf64"   => (0, vec!( ty::mk_f64() ), ty::mk_f64()),
            "log10f32" => (0, vec!( ty::mk_f32() ), ty::mk_f32()),
            "log10f64" => (0, vec!( ty::mk_f64() ), ty::mk_f64()),
            "log2f32"  => (0, vec!( ty::mk_f32() ), ty::mk_f32()),
            "log2f64"  => (0, vec!( ty::mk_f64() ), ty::mk_f64()),
            "fmaf32" => {
                (0,
                 vec!( ty::mk_f32(), ty::mk_f32(), ty::mk_f32() ),
                 ty::mk_f32())
            }
            "fmaf64" => {
                (0,
                 vec!( ty::mk_f64(), ty::mk_f64(), ty::mk_f64() ),
                 ty::mk_f64())
            }
            "fabsf32"      => (0, vec!( ty::mk_f32() ), ty::mk_f32()),
            "fabsf64"      => (0, vec!( ty::mk_f64() ), ty::mk_f64()),
            "copysignf32"  => (0, vec!( ty::mk_f32(), ty::mk_f32() ), ty::mk_f32()),
            "copysignf64"  => (0, vec!( ty::mk_f64(), ty::mk_f64() ), ty::mk_f64()),
            "floorf32"     => (0, vec!( ty::mk_f32() ), ty::mk_f32()),
            "floorf64"     => (0, vec!( ty::mk_f64() ), ty::mk_f64()),
            "ceilf32"      => (0, vec!( ty::mk_f32() ), ty::mk_f32()),
            "ceilf64"      => (0, vec!( ty::mk_f64() ), ty::mk_f64()),
            "truncf32"     => (0, vec!( ty::mk_f32() ), ty::mk_f32()),
            "truncf64"     => (0, vec!( ty::mk_f64() ), ty::mk_f64()),
            "rintf32"      => (0, vec!( ty::mk_f32() ), ty::mk_f32()),
            "rintf64"      => (0, vec!( ty::mk_f64() ), ty::mk_f64()),
            "nearbyintf32" => (0, vec!( ty::mk_f32() ), ty::mk_f32()),
            "nearbyintf64" => (0, vec!( ty::mk_f64() ), ty::mk_f64()),
            "roundf32"     => (0, vec!( ty::mk_f32() ), ty::mk_f32()),
            "roundf64"     => (0, vec!( ty::mk_f64() ), ty::mk_f64()),
            "ctpop8"       => (0, vec!( ty::mk_u8()  ), ty::mk_u8()),
            "ctpop16"      => (0, vec!( ty::mk_u16() ), ty::mk_u16()),
            "ctpop32"      => (0, vec!( ty::mk_u32() ), ty::mk_u32()),
            "ctpop64"      => (0, vec!( ty::mk_u64() ), ty::mk_u64()),
            "ctlz8"        => (0, vec!( ty::mk_u8()  ), ty::mk_u8()),
            "ctlz16"       => (0, vec!( ty::mk_u16() ), ty::mk_u16()),
            "ctlz32"       => (0, vec!( ty::mk_u32() ), ty::mk_u32()),
            "ctlz64"       => (0, vec!( ty::mk_u64() ), ty::mk_u64()),
            "cttz8"        => (0, vec!( ty::mk_u8()  ), ty::mk_u8()),
            "cttz16"       => (0, vec!( ty::mk_u16() ), ty::mk_u16()),
            "cttz32"       => (0, vec!( ty::mk_u32() ), ty::mk_u32()),
            "cttz64"       => (0, vec!( ty::mk_u64() ), ty::mk_u64()),
            "bswap16"      => (0, vec!( ty::mk_u16() ), ty::mk_u16()),
            "bswap32"      => (0, vec!( ty::mk_u32() ), ty::mk_u32()),
            "bswap64"      => (0, vec!( ty::mk_u64() ), ty::mk_u64()),

            "volatile_load" =>
                (1, vec!( ty::mk_imm_ptr(tcx, param(ccx, 0)) ), param(ccx, 0)),
            "volatile_store" =>
                (1, vec!( ty::mk_mut_ptr(tcx, param(ccx, 0)), param(ccx, 0) ), ty::mk_nil()),

            "i8_add_with_overflow" | "i8_sub_with_overflow" | "i8_mul_with_overflow" =>
                (0, vec!(ty::mk_i8(), ty::mk_i8()),
                ty::mk_tup(tcx, vec!(ty::mk_i8(), ty::mk_bool()))),

            "i16_add_with_overflow" | "i16_sub_with_overflow" | "i16_mul_with_overflow" =>
                (0, vec!(ty::mk_i16(), ty::mk_i16()),
                ty::mk_tup(tcx, vec!(ty::mk_i16(), ty::mk_bool()))),

            "i32_add_with_overflow" | "i32_sub_with_overflow" | "i32_mul_with_overflow" =>
                (0, vec!(ty::mk_i32(), ty::mk_i32()),
                ty::mk_tup(tcx, vec!(ty::mk_i32(), ty::mk_bool()))),

            "i64_add_with_overflow" | "i64_sub_with_overflow" | "i64_mul_with_overflow" =>
                (0, vec!(ty::mk_i64(), ty::mk_i64()),
                ty::mk_tup(tcx, vec!(ty::mk_i64(), ty::mk_bool()))),

            "u8_add_with_overflow" | "u8_sub_with_overflow" | "u8_mul_with_overflow" =>
                (0, vec!(ty::mk_u8(), ty::mk_u8()),
                ty::mk_tup(tcx, vec!(ty::mk_u8(), ty::mk_bool()))),

            "u16_add_with_overflow" | "u16_sub_with_overflow" | "u16_mul_with_overflow" =>
                (0, vec!(ty::mk_u16(), ty::mk_u16()),
                ty::mk_tup(tcx, vec!(ty::mk_u16(), ty::mk_bool()))),

            "u32_add_with_overflow" | "u32_sub_with_overflow" | "u32_mul_with_overflow"=>
                (0, vec!(ty::mk_u32(), ty::mk_u32()),
                ty::mk_tup(tcx, vec!(ty::mk_u32(), ty::mk_bool()))),

            "u64_add_with_overflow" | "u64_sub_with_overflow"  | "u64_mul_with_overflow" =>
                (0, vec!(ty::mk_u64(), ty::mk_u64()),
                ty::mk_tup(tcx, vec!(ty::mk_u64(), ty::mk_bool()))),

            ref other => {
                tcx.sess.span_err(it.span,
                                  format!("unrecognized intrinsic function: `{}`",
                                          *other).as_slice());
                return;
            }
        }
    };
    let fty = ty::mk_bare_fn(tcx, ty::BareFnTy {
        fn_style: ast::UnsafeFn,
        abi: abi::RustIntrinsic,
        sig: FnSig {
            binder_id: it.id,
            inputs: inputs,
            output: output,
            variadic: false,
        }
    });
    let i_ty = ty::lookup_item_type(ccx.tcx, local_def(it.id));
    let i_n_tps = i_ty.generics.type_param_defs().len();
    if i_n_tps != n_tps {
        tcx.sess.span_err(it.span,
                          format!("intrinsic has wrong number of type \
                                   parameters: found {}, expected {}",
                                  i_n_tps,
                                  n_tps).as_slice());
    } else {
        require_same_types(tcx,
                           None,
                           false,
                           it.span,
                           i_ty.ty,
                           fty,
                           || {
                format!("intrinsic has wrong type: expected `{}`",
                        ppaux::ty_to_str(ccx.tcx, fty))
            });
    }
}
