import std.map.hashmap;
import std.option;
import std.option.some;
import std.option.none;

import util.common.new_str_hash;
import util.common.spanned;
import util.common.span;
import util.common.ty_mach;
import util.common.append;

import front.ast;
import front.ast.fn_decl;
import front.ast.ident;
import front.ast.path;
import front.ast.mutability;
import front.ast.ty;
import front.ast.expr;
import front.ast.stmt;
import front.ast.block;
import front.ast.item;
import front.ast.view_item;
import front.ast.meta_item;
import front.ast.native_item;
import front.ast.arg;
import front.ast.pat;
import front.ast.decl;
import front.ast.arm;
import front.ast.def;
import front.ast.def_id;
import front.ast.ann;

import std._uint;
import std._vec;

type ast_fold[ENV] =
    @rec
    (
     // Path fold.
     (fn(&ENV e, &span sp, ast.path_ p) -> path)  fold_path,

     // Type folds.
     (fn(&ENV e, &span sp) -> @ty)                fold_ty_nil,
     (fn(&ENV e, &span sp) -> @ty)                fold_ty_bool,
     (fn(&ENV e, &span sp) -> @ty)                fold_ty_int,
     (fn(&ENV e, &span sp) -> @ty)                fold_ty_uint,
     (fn(&ENV e, &span sp, ty_mach tm) -> @ty)    fold_ty_machine,
     (fn(&ENV e, &span sp) -> @ty)                fold_ty_char,
     (fn(&ENV e, &span sp) -> @ty)                fold_ty_str,
     (fn(&ENV e, &span sp, @ty t) -> @ty)         fold_ty_box,
     (fn(&ENV e, &span sp, @ty t) -> @ty)         fold_ty_vec,

     (fn(&ENV e, &span sp, vec[@ty] elts) -> @ty) fold_ty_tup,

     (fn(&ENV e, &span sp,
         vec[ast.ty_field] elts) -> @ty)          fold_ty_rec,

     (fn(&ENV e, &span sp,
         vec[ast.ty_method] meths) -> @ty)        fold_ty_obj,

     (fn(&ENV e, &span sp,
         ast.proto proto,
         vec[rec(ast.mode mode, @ty ty)] inputs,
         @ty output) -> @ty)                      fold_ty_fn,

     (fn(&ENV e, &span sp, ast.path p,
         &option.t[def] d) -> @ty)                fold_ty_path,

     (fn(&ENV e, &span sp, @ty t) -> @ty)         fold_ty_mutable,

     // Expr folds.
     (fn(&ENV e, &span sp,
         vec[@expr] es, ann a) -> @expr)          fold_expr_vec,

     (fn(&ENV e, &span sp,
         vec[ast.elt] es, ann a) -> @expr)        fold_expr_tup,

     (fn(&ENV e, &span sp,
         vec[ast.field] fields,
         option.t[@expr] base, ann a) -> @expr)   fold_expr_rec,

     (fn(&ENV e, &span sp,
         @expr f, vec[@expr] args,
         ann a) -> @expr)                         fold_expr_call,

     (fn(&ENV e, &span sp,
         @expr f, vec[option.t[@expr]] args,
         ann a) -> @expr)                         fold_expr_bind,

     (fn(&ENV e, &span sp,
         ast.binop,
         @expr lhs, @expr rhs,
         ann a) -> @expr)                         fold_expr_binary,

     (fn(&ENV e, &span sp,
         ast.unop, @expr e,
         ann a) -> @expr)                         fold_expr_unary,

     (fn(&ENV e, &span sp,
         @ast.lit, ann a) -> @expr)               fold_expr_lit,

     (fn(&ENV e, &span sp,
         @ast.expr e, @ast.ty ty,
         ann a) -> @expr)                         fold_expr_cast,

     (fn(&ENV e, &span sp,
         @expr cond, &block thn,
         &vec[tup(@expr, block)] elifs,
         &option.t[block] els,
         ann a) -> @expr)                         fold_expr_if,

     (fn(&ENV e, &span sp,
         @decl decl, @expr seq, &block body,
         ann a) -> @expr)                         fold_expr_for,

     (fn(&ENV e, &span sp,
         @decl decl, @expr seq, &block body,
         ann a) -> @expr)                         fold_expr_for_each,

     (fn(&ENV e, &span sp,
         @expr cond, &block body,
         ann a) -> @expr)                         fold_expr_while,

     (fn(&ENV e, &span sp,
         &block body, @expr cond,
         ann a) -> @expr)                         fold_expr_do_while,

     (fn(&ENV e, &span sp,
         @expr e, vec[arm] arms,
         ann a) -> @expr)                         fold_expr_alt,

     (fn(&ENV e, &span sp,
         &block blk, ann a) -> @expr)             fold_expr_block,

     (fn(&ENV e, &span sp,
         @expr lhs, @expr rhs,
         ann a) -> @expr)                         fold_expr_assign,

     (fn(&ENV e, &span sp,
         ast.binop,
         @expr lhs, @expr rhs,
         ann a) -> @expr)                         fold_expr_assign_op,

     (fn(&ENV e, &span sp,
         @expr e, ident i,
         ann a) -> @expr)                         fold_expr_field,

     (fn(&ENV e, &span sp,
         @expr e, @expr ix,
         ann a) -> @expr)                         fold_expr_index,

     (fn(&ENV e, &span sp,
         &path p,
         &option.t[def] d,
         ann a) -> @expr)                         fold_expr_path,

     (fn(&ENV e, &span sp,
         &path p, vec[@expr] args,
         option.t[@expr] body,
         @expr expanded,
         ann a) -> @expr)                         fold_expr_ext,

     (fn(&ENV e, &span sp) -> @expr)              fold_expr_fail,

     (fn(&ENV e, &span sp,
         &option.t[@expr] rv) -> @expr)           fold_expr_ret,

     (fn(&ENV e, &span sp,
         &option.t[@expr] rv) -> @expr)           fold_expr_put,

     (fn(&ENV e, &span sp,
         @expr e) -> @expr)                       fold_expr_be,

     (fn(&ENV e, &span sp,
         @expr e) -> @expr)                       fold_expr_log,

     (fn(&ENV e, &span sp,
         @expr e) -> @expr)                       fold_expr_check_expr,

     // Decl folds.
     (fn(&ENV e, &span sp,
         @ast.local local) -> @decl)              fold_decl_local,

     (fn(&ENV e, &span sp,
         @item item) -> @decl)                    fold_decl_item,


     // Pat folds.
     (fn(&ENV e, &span sp,
         ann a) -> @pat)                          fold_pat_wild,

     (fn(&ENV e, &span sp,
         @ast.lit lit, ann a) -> @pat)            fold_pat_lit,

     (fn(&ENV e, &span sp,
         ident i, def_id did, ann a) -> @pat)     fold_pat_bind,

     (fn(&ENV e, &span sp,
         path p, vec[@pat] args,
         option.t[ast.variant_def] d,
         ann a) -> @pat)                          fold_pat_tag,


     // Stmt folds.
     (fn(&ENV e, &span sp,
         @decl decl) -> @stmt)                    fold_stmt_decl,

     (fn(&ENV e, &span sp,
         @expr e) -> @stmt)                       fold_stmt_expr,

     // Item folds.
     (fn(&ENV e, &span sp, ident ident,
         @ty t, @expr e,
         def_id id, ann a) -> @item)              fold_item_const,

     (fn(&ENV e, &span sp, ident ident,
         &ast._fn f,
         vec[ast.ty_param] ty_params,
         def_id id, ann a) -> @item)              fold_item_fn,

     (fn(&ENV e, &span sp, ident ident,
         &ast.fn_decl decl,
         vec[ast.ty_param] ty_params,
         def_id id, ann a) -> @native_item)       fold_native_item_fn,

     (fn(&ENV e, &span sp, ident ident,
         &ast._mod m, def_id id) -> @item)        fold_item_mod,

     (fn(&ENV e, &span sp, ident ident,
         &ast.native_mod m, def_id id) -> @item)  fold_item_native_mod,

     (fn(&ENV e, &span sp, ident ident,
         @ty t, vec[ast.ty_param] ty_params,
         def_id id, ann a) -> @item)              fold_item_ty,

     (fn(&ENV e, &span sp, ident ident,
         def_id id) -> @native_item)              fold_native_item_ty,

     (fn(&ENV e, &span sp, ident ident,
         vec[ast.variant] variants,
         vec[ast.ty_param] ty_params,
         def_id id) -> @item)                     fold_item_tag,

     (fn(&ENV e, &span sp, ident ident,
         &ast._obj ob,
         vec[ast.ty_param] ty_params,
         def_id id, ann a) -> @item)              fold_item_obj,

     // View Item folds.
     (fn(&ENV e, &span sp, ident ident,
         vec[@meta_item] meta_items,
         def_id id) -> @view_item)                fold_view_item_use,

     (fn(&ENV e, &span sp, ident i, vec[ident] idents,
         def_id id, option.t[def]) -> @view_item) fold_view_item_import,

     (fn(&ENV e, &span sp, ident i) -> @view_item) fold_view_item_export,

     // Additional nodes.
     (fn(&ENV e, &span sp,
         &ast.block_) -> block)                   fold_block,

     (fn(&ENV e, &fn_decl decl,
         ast.proto proto,
         &block body) -> ast._fn)                 fold_fn,

     (fn(&ENV e, ast.effect effect,
         vec[arg] inputs,
         @ty output) -> ast.fn_decl)              fold_fn_decl,

     (fn(&ENV e, &ast._mod m) -> ast._mod)        fold_mod,

     (fn(&ENV e, &ast.native_mod m) -> ast.native_mod) fold_native_mod,

     (fn(&ENV e, &span sp,
         vec[@ast.crate_directive] cdirs,
         &ast._mod m) -> @ast.crate)              fold_crate,

     (fn(&ENV e,
         vec[ast.obj_field] fields,
         vec[@ast.method] methods,
         option.t[block] dtor) -> ast._obj)       fold_obj,

     // Env updates.
     (fn(&ENV e, @ast.crate c) -> ENV) update_env_for_crate,
     (fn(&ENV e, @item i) -> ENV) update_env_for_item,
     (fn(&ENV e, @native_item i) -> ENV) update_env_for_native_item,
     (fn(&ENV e, @view_item i) -> ENV) update_env_for_view_item,
     (fn(&ENV e, &block b) -> ENV) update_env_for_block,
     (fn(&ENV e, @stmt s) -> ENV) update_env_for_stmt,
     (fn(&ENV e, @decl i) -> ENV) update_env_for_decl,
     (fn(&ENV e, @pat p) -> ENV) update_env_for_pat,
     (fn(&ENV e, &arm a) -> ENV) update_env_for_arm,
     (fn(&ENV e, @expr x) -> ENV) update_env_for_expr,
     (fn(&ENV e, @ty t) -> ENV) update_env_for_ty,

     // Traversal control.
     (fn(&ENV v) -> bool) keep_going
     );


//// Fold drivers.

fn fold_path[ENV](&ENV env, ast_fold[ENV] fld, &path p) -> path {
    let vec[@ast.ty] tys_ = vec();
    for (@ast.ty t in p.node.types) {
        append[@ast.ty](tys_, fold_ty(env, fld, t));
    }
    let ast.path_ p_ = rec(idents=p.node.idents, types=tys_);
    ret fld.fold_path(env, p.span, p_);
}

fn fold_ty[ENV](&ENV env, ast_fold[ENV] fld, @ty t) -> @ty {
    let ENV env_ = fld.update_env_for_ty(env, t);

    if (!fld.keep_going(env_)) {
        ret t;
    }

    alt (t.node) {
        case (ast.ty_nil) { ret fld.fold_ty_nil(env_, t.span); }
        case (ast.ty_bool) { ret fld.fold_ty_bool(env_, t.span); }
        case (ast.ty_int) { ret fld.fold_ty_int(env_, t.span); }
        case (ast.ty_uint) { ret fld.fold_ty_uint(env_, t.span); }

        case (ast.ty_machine(?m)) {
            ret fld.fold_ty_machine(env_, t.span, m);
        }

        case (ast.ty_char) { ret fld.fold_ty_char(env_, t.span); }
        case (ast.ty_str) { ret fld.fold_ty_str(env_, t.span); }

        case (ast.ty_box(?ty)) {
            auto ty_ = fold_ty(env, fld, ty);
            ret fld.fold_ty_box(env_, t.span, ty_);
        }

        case (ast.ty_vec(?ty)) {
            auto ty_ = fold_ty(env, fld, ty);
            ret fld.fold_ty_vec(env_, t.span, ty_);
        }

        case (ast.ty_tup(?elts)) {
            let vec[@ty] elts_ = vec();
            for (@ty elt in elts) {
                append[@ty](elts_,fold_ty(env, fld, elt));
            }
            ret fld.fold_ty_tup(env_, t.span, elts_);
        }

        case (ast.ty_rec(?flds)) {
            let vec[ast.ty_field] flds_ = vec();
            for (ast.ty_field f in flds) {
                append[ast.ty_field]
                    (flds_, rec(ty=fold_ty(env, fld, f.ty) with f));
            }
            ret fld.fold_ty_rec(env_, t.span, flds_);
        }

        case (ast.ty_obj(?meths)) {
            let vec[ast.ty_method] meths_ = vec();
            for (ast.ty_method m in meths) {
                auto tfn = fold_ty_fn(env_, fld, t.span, m.proto,
                                      m.inputs, m.output);
                alt (tfn.node) {
                    case (ast.ty_fn(?p, ?ins, ?out)) {
                        append[ast.ty_method]
                            (meths_, rec(proto=p, inputs=ins, output=out
                                         with m));
                    }
                }
            }
            ret fld.fold_ty_obj(env_, t.span, meths_);
        }

        case (ast.ty_path(?pth, ?ref_opt)) {
            auto pth_ = fold_path(env, fld, pth);
            ret fld.fold_ty_path(env_, t.span, pth_, ref_opt);
        }

        case (ast.ty_mutable(?ty)) {
            auto ty_ = fold_ty(env, fld, ty);
            ret fld.fold_ty_mutable(env_, t.span, ty_);
        }

        case (ast.ty_fn(?proto, ?inputs, ?output)) {
            ret fold_ty_fn(env_, fld, t.span, proto, inputs, output);
        }
    }
}

fn fold_ty_fn[ENV](&ENV env, ast_fold[ENV] fld, &span sp,
                   ast.proto proto,
                   vec[rec(ast.mode mode, @ty ty)] inputs,
                   @ty output) -> @ty {
    auto output_ = fold_ty(env, fld, output);
    let vec[rec(ast.mode mode, @ty ty)] inputs_ = vec();
    for (rec(ast.mode mode, @ty ty) input in inputs) {
        auto ty_ = fold_ty(env, fld, input.ty);
        auto input_ = rec(ty=ty_ with input);
        inputs_ += vec(input_);
    }
    ret fld.fold_ty_fn(env, sp, proto, inputs_, output_);
}

fn fold_decl[ENV](&ENV env, ast_fold[ENV] fld, @decl d) -> @decl {
    let ENV env_ = fld.update_env_for_decl(env, d);

    if (!fld.keep_going(env_)) {
        ret d;
    }

    alt (d.node) {
        case (ast.decl_local(?local)) {
            auto ty_ = none[@ast.ty];
            auto init_ = none[@ast.expr];
            alt (local.ty) {
                case (some[@ast.ty](?t)) {
                    ty_ = some[@ast.ty](fold_ty(env, fld, t));
                }
                case (_) { /* fall through */  }
            }
            alt (local.init) {
                case (some[@ast.expr](?e)) {
                    init_ = some[@ast.expr](fold_expr(env, fld, e));
                }
                case (_) { /* fall through */  }
            }
            let @ast.local local_ = @rec(ty=ty_, init=init_ with *local);
            ret fld.fold_decl_local(env_, d.span, local_);
        }

        case (ast.decl_item(?item)) {
            auto item_ = fold_item(env_, fld, item);
            ret fld.fold_decl_item(env_, d.span, item_);
        }
    }

    fail;
}

fn fold_pat[ENV](&ENV env, ast_fold[ENV] fld, @ast.pat p) -> @ast.pat {
    let ENV env_ = fld.update_env_for_pat(env, p);

    if (!fld.keep_going(env_)) {
        ret p;
    }

    alt (p.node) {
        case (ast.pat_wild(?t)) { ret fld.fold_pat_wild(env_, p.span, t); }
        case (ast.pat_lit(?lt, ?t)) {
            ret fld.fold_pat_lit(env_, p.span, lt, t);
        }
        case (ast.pat_bind(?id, ?did, ?t)) {
            ret fld.fold_pat_bind(env_, p.span, id, did, t);
        }
        case (ast.pat_tag(?path, ?pats, ?d, ?t)) {
            auto ppath = fold_path(env, fld, path);

            let vec[@ast.pat] ppats = vec();
            for (@ast.pat pat in pats) {
                ppats += vec(fold_pat(env_, fld, pat));
            }

            ret fld.fold_pat_tag(env_, p.span, ppath, ppats, d, t);
        }
    }
}

fn fold_exprs[ENV](&ENV env, ast_fold[ENV] fld, vec[@expr] es) -> vec[@expr] {
    let vec[@expr] exprs = vec();
    for (@expr e in es) {
        append[@expr](exprs, fold_expr(env, fld, e));
    }
    ret exprs;
}

fn fold_tup_elt[ENV](&ENV env, ast_fold[ENV] fld, &ast.elt e) -> ast.elt {
    ret rec(expr=fold_expr(env, fld, e.expr) with e);
}

fn fold_rec_field[ENV](&ENV env, ast_fold[ENV] fld, &ast.field f)
    -> ast.field {
    ret rec(expr=fold_expr(env, fld, f.expr) with f);
}

fn fold_expr[ENV](&ENV env, ast_fold[ENV] fld, &@expr e) -> @expr {

    let ENV env_ = fld.update_env_for_expr(env, e);

    if (!fld.keep_going(env_)) {
        ret e;
    }

    alt (e.node) {
        case (ast.expr_vec(?es, ?t)) {
            auto ees = fold_exprs(env_, fld, es);
            ret fld.fold_expr_vec(env_, e.span, ees, t);
        }

        case (ast.expr_tup(?es, ?t)) {
            let vec[ast.elt] elts = vec();
            for (ast.elt e in es) {
                elts += fold_tup_elt[ENV](env, fld, e);
            }
            ret fld.fold_expr_tup(env_, e.span, elts, t);
        }

        case (ast.expr_rec(?fs, ?base, ?t)) {
            let vec[ast.field] fields = vec();
            let option.t[@expr] b = none[@expr];
            for (ast.field f in fs) {
                fields += fold_rec_field(env, fld, f);
            }
            alt (base) {
                case (none[@ast.expr]) { }
                case (some[@ast.expr](?eb)) {
                    b = some[@expr](fold_expr(env_, fld, eb));
                }
            }
            ret fld.fold_expr_rec(env_, e.span, fields, b, t);
        }

        case (ast.expr_call(?f, ?args, ?t)) {
            auto ff = fold_expr(env_, fld, f);
            auto aargs = fold_exprs(env_, fld, args);
            ret fld.fold_expr_call(env_, e.span, ff, aargs, t);
        }

        case (ast.expr_bind(?f, ?args_opt, ?t)) {
            auto ff = fold_expr(env_, fld, f);
            let vec[option.t[@ast.expr]] aargs_opt = vec();
            for (option.t[@ast.expr] t_opt in args_opt) {
                alt (t_opt) {
                    case (none[@ast.expr]) {
                        aargs_opt += none[@ast.expr];
                    }
                    case (some[@ast.expr](?e)) {
                        aargs_opt += vec(some(fold_expr(env_, fld, e)));
                    }
                    case (none[@ast.expr]) { /* empty */ }
                }
            }
            ret fld.fold_expr_bind(env_, e.span, ff, aargs_opt, t);
        }

        case (ast.expr_binary(?op, ?a, ?b, ?t)) {
            auto aa = fold_expr(env_, fld, a);
            auto bb = fold_expr(env_, fld, b);
            ret fld.fold_expr_binary(env_, e.span, op, aa, bb, t);
        }

        case (ast.expr_unary(?op, ?a, ?t)) {
            auto aa = fold_expr(env_, fld, a);
            ret fld.fold_expr_unary(env_, e.span, op, aa, t);
        }

        case (ast.expr_lit(?lit, ?t)) {
            ret fld.fold_expr_lit(env_, e.span, lit, t);
        }

        case (ast.expr_cast(?e, ?t, ?at)) {
            auto ee = fold_expr(env_, fld, e);
            auto tt = fold_ty(env, fld, t);
            ret fld.fold_expr_cast(env_, e.span, ee, tt, at);
        }

        case (ast.expr_if(?cnd, ?thn, ?elifs, ?els, ?t)) {
            auto ccnd = fold_expr(env_, fld, cnd);
            auto tthn = fold_block(env_, fld, thn);

            let vec[tup(@ast.expr, ast.block)] eelifs = vec();
            for (tup(@expr, block) elif in elifs) {
                auto elifcnd = elif._0;
                auto elifthn = elif._1;
                auto elifccnd = fold_expr(env_, fld, elifcnd);
                auto eliftthn = fold_block(env_, fld, elifthn);
                eelifs += tup(elifccnd, eliftthn);
            }

            auto eels = none[block];
            alt (els) {
                case (some[block](?b)) {
                    eels = some(fold_block(env_, fld, b));
                }
                case (_) { /* fall through */  }
            }
            ret fld.fold_expr_if(env_, e.span, ccnd, tthn, eelifs, eels, t);
        }

        case (ast.expr_for(?decl, ?seq, ?body, ?t)) {
            auto ddecl = fold_decl(env_, fld, decl);
            auto sseq = fold_expr(env_, fld, seq);
            auto bbody = fold_block(env_, fld, body);
            ret fld.fold_expr_for(env_, e.span, ddecl, sseq, bbody, t);
        }

        case (ast.expr_for_each(?decl, ?seq, ?body, ?t)) {
            auto ddecl = fold_decl(env_, fld, decl);
            auto sseq = fold_expr(env_, fld, seq);
            auto bbody = fold_block(env_, fld, body);
            ret fld.fold_expr_for_each(env_, e.span, ddecl, sseq, bbody, t);
        }

        case (ast.expr_while(?cnd, ?body, ?t)) {
            auto ccnd = fold_expr(env_, fld, cnd);
            auto bbody = fold_block(env_, fld, body);
            ret fld.fold_expr_while(env_, e.span, ccnd, bbody, t);
        }

        case (ast.expr_do_while(?body, ?cnd, ?t)) {
            auto bbody = fold_block(env_, fld, body);
            auto ccnd = fold_expr(env_, fld, cnd);
            ret fld.fold_expr_do_while(env_, e.span, bbody, ccnd, t);
        }

        case (ast.expr_alt(?expr, ?arms, ?t)) {
            auto eexpr = fold_expr(env_, fld, expr);
            let vec[ast.arm] aarms = vec();
            for (ast.arm a in arms) {
                aarms += vec(fold_arm(env_, fld, a));
            }
            ret fld.fold_expr_alt(env_, e.span, eexpr, aarms, t);
        }

        case (ast.expr_block(?b, ?t)) {
            auto bb = fold_block(env_, fld, b);
            ret fld.fold_expr_block(env_, e.span, bb, t);
        }

        case (ast.expr_assign(?lhs, ?rhs, ?t)) {
            auto llhs = fold_expr(env_, fld, lhs);
            auto rrhs = fold_expr(env_, fld, rhs);
            ret fld.fold_expr_assign(env_, e.span, llhs, rrhs, t);
        }

        case (ast.expr_assign_op(?op, ?lhs, ?rhs, ?t)) {
            auto llhs = fold_expr(env_, fld, lhs);
            auto rrhs = fold_expr(env_, fld, rhs);
            ret fld.fold_expr_assign_op(env_, e.span, op, llhs, rrhs, t);
        }

        case (ast.expr_field(?e, ?i, ?t)) {
            auto ee = fold_expr(env_, fld, e);
            ret fld.fold_expr_field(env_, e.span, ee, i, t);
        }

        case (ast.expr_index(?e, ?ix, ?t)) {
            auto ee = fold_expr(env_, fld, e);
            auto iix = fold_expr(env_, fld, ix);
            ret fld.fold_expr_index(env_, e.span, ee, iix, t);
        }

        case (ast.expr_path(?p, ?r, ?t)) {
            auto p_ = fold_path(env_, fld, p);
            ret fld.fold_expr_path(env_, e.span, p_, r, t);
        }

        case (ast.expr_ext(?p, ?args, ?body, ?expanded, ?t)) {
            // Only fold the expanded expression, not the
            // expressions involved in syntax extension
            auto exp = fold_expr(env_, fld, expanded);
            ret fld.fold_expr_ext(env_, e.span, p, args, body,
                                  exp, t);
        }

        case (ast.expr_fail) {
            ret fld.fold_expr_fail(env_, e.span);
        }

        case (ast.expr_ret(?oe)) {
            auto oee = none[@expr];
            alt (oe) {
                case (some[@expr](?x)) {
                    oee = some(fold_expr(env_, fld, x));
                }
                case (_) { /* fall through */  }
            }
            ret fld.fold_expr_ret(env_, e.span, oee);
        }

        case (ast.expr_put(?oe)) {
            auto oee = none[@expr];
            alt (oe) {
                case (some[@expr](?x)) {
                    oee = some(fold_expr(env_, fld, x));
                }
                case (_) { /* fall through */  }
            }
            ret fld.fold_expr_put(env_, e.span, oee);
        }

        case (ast.expr_be(?x)) {
            auto ee = fold_expr(env_, fld, x);
            ret fld.fold_expr_be(env_, e.span, ee);
        }

        case (ast.expr_log(?x)) {
            auto ee = fold_expr(env_, fld, x);
            ret fld.fold_expr_log(env_, e.span, ee);
        }

        case (ast.expr_check_expr(?x)) {
            auto ee = fold_expr(env_, fld, x);
            ret fld.fold_expr_check_expr(env_, e.span, ee);
        }

    }

    fail;
}


fn fold_stmt[ENV](&ENV env, ast_fold[ENV] fld, &@stmt s) -> @stmt {

    let ENV env_ = fld.update_env_for_stmt(env, s);

    if (!fld.keep_going(env_)) {
        ret s;
    }

    alt (s.node) {
        case (ast.stmt_decl(?d)) {
            auto dd = fold_decl(env_, fld, d);
            ret fld.fold_stmt_decl(env_, s.span, dd);
        }

        case (ast.stmt_expr(?e)) {
            auto ee = fold_expr(env_, fld, e);
            ret fld.fold_stmt_expr(env_, s.span, ee);
        }
    }
    fail;
}

fn fold_block[ENV](&ENV env, ast_fold[ENV] fld, &block blk) -> block {

    auto index = new_str_hash[ast.block_index_entry]();
    let ENV env_ = fld.update_env_for_block(env, blk);

    if (!fld.keep_going(env_)) {
        ret blk;
    }

    let vec[@ast.stmt] stmts = vec();
    for (@ast.stmt s in blk.node.stmts) {
        auto new_stmt = fold_stmt[ENV](env_, fld, s);
        append[@ast.stmt](stmts, new_stmt);
        ast.index_stmt(index, new_stmt);
    }

    auto expr = none[@ast.expr];
    alt (blk.node.expr) {
        case (some[@ast.expr](?e)) {
            expr = some[@ast.expr](fold_expr[ENV](env_, fld, e));
        }
        case (none[@ast.expr]) {
            // empty
        }
    }

    ret respan(blk.span, rec(stmts=stmts, expr=expr, index=index));
}

fn fold_arm[ENV](&ENV env, ast_fold[ENV] fld, &arm a) -> arm {
    let ENV env_ = fld.update_env_for_arm(env, a);
    auto ppat = fold_pat(env_, fld, a.pat);
    auto bblock = fold_block(env_, fld, a.block);
    ret rec(pat=ppat, block=bblock, index=a.index);
}

fn fold_arg[ENV](&ENV env, ast_fold[ENV] fld, &arg a) -> arg {
    auto ty = fold_ty(env, fld, a.ty);
    ret rec(ty=ty with a);
}

fn fold_fn_decl[ENV](&ENV env, ast_fold[ENV] fld,
                     &ast.fn_decl decl) -> ast.fn_decl {
    let vec[ast.arg] inputs = vec();
    for (ast.arg a in decl.inputs) {
        inputs += fold_arg(env, fld, a);
    }
    auto output = fold_ty[ENV](env, fld, decl.output);
    ret fld.fold_fn_decl(env, decl.effect, inputs, output);
}

fn fold_fn[ENV](&ENV env, ast_fold[ENV] fld, &ast._fn f) -> ast._fn {
    auto decl = fold_fn_decl(env, fld, f.decl);

    auto body = fold_block[ENV](env, fld, f.body);

    ret fld.fold_fn(env, decl, f.proto, body);
}


fn fold_obj_field[ENV](&ENV env, ast_fold[ENV] fld,
                       &ast.obj_field f) -> ast.obj_field {
    auto ty = fold_ty(env, fld, f.ty);
    ret rec(ty=ty with f);
}


fn fold_method[ENV](&ENV env, ast_fold[ENV] fld,
                    @ast.method m) -> @ast.method {
    auto meth = fold_fn(env, fld, m.node.meth);
    ret @rec(node=rec(meth=meth with m.node) with *m);
}


fn fold_obj[ENV](&ENV env, ast_fold[ENV] fld, &ast._obj ob) -> ast._obj {

    let vec[ast.obj_field] fields = vec();
    let vec[@ast.method] meths = vec();
    for (ast.obj_field f in ob.fields) {
        fields += fold_obj_field(env, fld, f);
    }
    let option.t[block] dtor = none[block];
    alt (ob.dtor) {
        case (none[block]) { }
        case (some[block](?b)) {
            dtor = some[block](fold_block[ENV](env, fld, b));
        }
    }
    let vec[ast.ty_param] tp = vec();
    for (@ast.method m in ob.methods) {
        // Fake-up an ast.item for this method.
        // FIXME: this is kinda awful. Maybe we should reformulate
        // the way we store methods in the AST?
        let @ast.item i = @rec(node=ast.item_fn(m.node.ident,
                                                m.node.meth,
                                                tp,
                                                m.node.id,
                                                m.node.ann),
                               span=m.span);
        let ENV _env = fld.update_env_for_item(env, i);
        append[@ast.method](meths, fold_method(_env, fld, m));
    }
    ret fld.fold_obj(env, fields, meths, dtor);
}

fn fold_view_item[ENV](&ENV env, ast_fold[ENV] fld, @view_item vi)
    -> @view_item {

    let ENV env_ = fld.update_env_for_view_item(env, vi);

    if (!fld.keep_going(env_)) {
        ret vi;
    }

    alt (vi.node) {
        case (ast.view_item_use(?ident, ?meta_items, ?def_id)) {
            ret fld.fold_view_item_use(env_, vi.span, ident, meta_items,
                                       def_id);
        }
        case (ast.view_item_import(?def_ident, ?idents, ?def_id,
                                   ?target_def)) {
            ret fld.fold_view_item_import(env_, vi.span, def_ident, idents,
                                          def_id, target_def);
        }

        case (ast.view_item_export(?def_ident)) {
            ret fld.fold_view_item_export(env_, vi.span, def_ident);
        }
    }

    fail;
}

fn fold_item[ENV](&ENV env, ast_fold[ENV] fld, @item i) -> @item {

    let ENV env_ = fld.update_env_for_item(env, i);

    if (!fld.keep_going(env_)) {
        ret i;
    }

    alt (i.node) {

        case (ast.item_const(?ident, ?t, ?e, ?id, ?ann)) {
            let @ast.ty t_ = fold_ty[ENV](env_, fld, t);
            let @ast.expr e_ = fold_expr(env_, fld, e);
            ret fld.fold_item_const(env_, i.span, ident, t_, e_, id, ann);
        }

        case (ast.item_fn(?ident, ?ff, ?tps, ?id, ?ann)) {
            let ast._fn ff_ = fold_fn[ENV](env_, fld, ff);
            ret fld.fold_item_fn(env_, i.span, ident, ff_, tps, id, ann);
        }

        case (ast.item_mod(?ident, ?mm, ?id)) {
            let ast._mod mm_ = fold_mod[ENV](env_, fld, mm);
            ret fld.fold_item_mod(env_, i.span, ident, mm_, id);
        }

        case (ast.item_native_mod(?ident, ?mm, ?id)) {
            let ast.native_mod mm_ = fold_native_mod[ENV](env_, fld, mm);
            ret fld.fold_item_native_mod(env_, i.span, ident, mm_, id);
        }

        case (ast.item_ty(?ident, ?ty, ?params, ?id, ?ann)) {
            let @ast.ty ty_ = fold_ty[ENV](env_, fld, ty);
            ret fld.fold_item_ty(env_, i.span, ident, ty_, params, id, ann);
        }

        case (ast.item_tag(?ident, ?variants, ?ty_params, ?id)) {
            let vec[ast.variant] new_variants = vec();
            for (ast.variant v in variants) {
                let vec[ast.variant_arg] new_args = vec();
                for (ast.variant_arg va in v.args) {
                    auto new_ty = fold_ty[ENV](env_, fld, va.ty);
                    new_args += vec(rec(ty=new_ty, id=va.id));
                }
                new_variants += rec(name=v.name, args=new_args, id=v.id,
                                    ann=v.ann);
            }
            ret fld.fold_item_tag(env_, i.span, ident, new_variants,
                                  ty_params, id);
        }

        case (ast.item_obj(?ident, ?ob, ?tps, ?id, ?ann)) {
            let ast._obj ob_ = fold_obj[ENV](env_, fld, ob);
            ret fld.fold_item_obj(env_, i.span, ident, ob_, tps, id, ann);
        }

    }

    fail;
}

fn fold_mod[ENV](&ENV e, ast_fold[ENV] fld, &ast._mod m) -> ast._mod {

    let vec[@view_item] view_items = vec();
    let vec[@item] items = vec();
    auto index = new_str_hash[ast.mod_index_entry]();

    for (@view_item vi in m.view_items) {
        auto new_vi = fold_view_item[ENV](e, fld, vi);
        append[@view_item](view_items, new_vi);
        ast.index_view_item(index, new_vi);
    }

    for (@item i in m.items) {
        auto new_item = fold_item[ENV](e, fld, i);
        append[@item](items, new_item);
        ast.index_item(index, new_item);
    }

    ret fld.fold_mod(e, rec(view_items=view_items, items=items, index=index));
}

fn fold_native_item[ENV](&ENV env, ast_fold[ENV] fld,
                         @native_item i) -> @native_item {
    let ENV env_ = fld.update_env_for_native_item(env, i);

    if (!fld.keep_going(env_)) {
        ret i;
    }
    alt (i.node) {
        case (ast.native_item_ty(?ident, ?id)) {
            ret fld.fold_native_item_ty(env_, i.span, ident, id);
        }
        case (ast.native_item_fn(?ident, ?fn_decl, ?ty_params, ?id, ?ann)) {
            auto d = fold_fn_decl[ENV](env_, fld, fn_decl);
            ret fld.fold_native_item_fn(env_, i.span, ident, d,
                                        ty_params, id, ann);
        }
    }
}

fn fold_native_mod[ENV](&ENV e, ast_fold[ENV] fld,
                        &ast.native_mod m) -> ast.native_mod {
    let vec[@view_item] view_items = vec();
    let vec[@native_item] items = vec();
    auto index = new_str_hash[ast.native_mod_index_entry]();

    for (@view_item vi in m.view_items) {
        auto new_vi = fold_view_item[ENV](e, fld, vi);
        append[@view_item](view_items, new_vi);
    }

    for (@native_item i in m.items) {
        auto new_item = fold_native_item[ENV](e, fld, i);
        append[@native_item](items, new_item);
        ast.index_native_item(index, new_item);
    }

    ret fld.fold_native_mod(e, rec(native_name=m.native_name,
                                   abi=m.abi,
                                   view_items=view_items,
                                   items=items,
                                   index=index));
}

fn fold_crate[ENV](&ENV env, ast_fold[ENV] fld, @ast.crate c) -> @ast.crate {
    // FIXME: possibly fold the directives so you process any expressions
    // within them? Not clear. After front/eval.rs, nothing else should look
    // at crate directives.
    let ENV env_ = fld.update_env_for_crate(env, c);
    let ast._mod m = fold_mod[ENV](env_, fld, c.node.module);
    ret fld.fold_crate(env_, c.span, c.node.directives, m);
}

//// Identity folds.

fn respan[T](&span sp, &T t) -> spanned[T] {
    ret rec(node=t, span=sp);
}


// Path identity.

fn identity_fold_path[ENV](&ENV env, &span sp, ast.path_ p) -> path {
    ret respan(sp, p);
}

// Type identities.

fn identity_fold_ty_nil[ENV](&ENV env, &span sp) -> @ty {
    ret @respan(sp, ast.ty_nil);
}

fn identity_fold_ty_bool[ENV](&ENV env, &span sp) -> @ty {
    ret @respan(sp, ast.ty_bool);
}

fn identity_fold_ty_int[ENV](&ENV env, &span sp) -> @ty {
    ret @respan(sp, ast.ty_int);
}

fn identity_fold_ty_uint[ENV](&ENV env, &span sp) -> @ty {
    ret @respan(sp, ast.ty_uint);
}

fn identity_fold_ty_machine[ENV](&ENV env, &span sp,
                                 ty_mach tm) -> @ty {
    ret @respan(sp, ast.ty_machine(tm));
}

fn identity_fold_ty_char[ENV](&ENV env, &span sp) -> @ty {
    ret @respan(sp, ast.ty_char);
}

fn identity_fold_ty_str[ENV](&ENV env, &span sp) -> @ty {
    ret @respan(sp, ast.ty_str);
}

fn identity_fold_ty_box[ENV](&ENV env, &span sp, @ty t) -> @ty {
    ret @respan(sp, ast.ty_box(t));
}

fn identity_fold_ty_vec[ENV](&ENV env, &span sp, @ty t) -> @ty {
    ret @respan(sp, ast.ty_vec(t));
}

fn identity_fold_ty_tup[ENV](&ENV env, &span sp,
                             vec[@ty] elts) -> @ty {
    ret @respan(sp, ast.ty_tup(elts));
}

fn identity_fold_ty_rec[ENV](&ENV env, &span sp,
                             vec[ast.ty_field] elts) -> @ty {
    ret @respan(sp, ast.ty_rec(elts));
}

fn identity_fold_ty_obj[ENV](&ENV env, &span sp,
                             vec[ast.ty_method] meths) -> @ty {
    ret @respan(sp, ast.ty_obj(meths));
}

fn identity_fold_ty_fn[ENV](&ENV env, &span sp,
                            ast.proto proto,
                            vec[rec(ast.mode mode, @ty ty)] inputs,
                            @ty output) -> @ty {
    ret @respan(sp, ast.ty_fn(proto, inputs, output));
}

fn identity_fold_ty_path[ENV](&ENV env, &span sp, ast.path p,
                        &option.t[def] d) -> @ty {
    ret @respan(sp, ast.ty_path(p, d));
}

fn identity_fold_ty_mutable[ENV](&ENV env, &span sp, @ty t) -> @ty {
    ret @respan(sp, ast.ty_mutable(t));
}


// Expr identities.

fn identity_fold_expr_vec[ENV](&ENV env, &span sp, vec[@expr] es,
                               ann a) -> @expr {
    ret @respan(sp, ast.expr_vec(es, a));
}

fn identity_fold_expr_tup[ENV](&ENV env, &span sp,
                               vec[ast.elt] es, ann a) -> @expr {
    ret @respan(sp, ast.expr_tup(es, a));
}

fn identity_fold_expr_rec[ENV](&ENV env, &span sp,
                               vec[ast.field] fields,
                               option.t[@expr] base, ann a) -> @expr {
    ret @respan(sp, ast.expr_rec(fields, base, a));
}

fn identity_fold_expr_call[ENV](&ENV env, &span sp, @expr f,
                                vec[@expr] args, ann a) -> @expr {
    ret @respan(sp, ast.expr_call(f, args, a));
}

fn identity_fold_expr_bind[ENV](&ENV env, &span sp, @expr f,
                                vec[option.t[@expr]] args_opt, ann a)
        -> @expr {
    ret @respan(sp, ast.expr_bind(f, args_opt, a));
}

fn identity_fold_expr_binary[ENV](&ENV env, &span sp, ast.binop b,
                                  @expr lhs, @expr rhs,
                                  ann a) -> @expr {
    ret @respan(sp, ast.expr_binary(b, lhs, rhs, a));
}

fn identity_fold_expr_unary[ENV](&ENV env, &span sp,
                                 ast.unop u, @expr e, ann a)
        -> @expr {
    ret @respan(sp, ast.expr_unary(u, e, a));
}

fn identity_fold_expr_lit[ENV](&ENV env, &span sp, @ast.lit lit,
                               ann a) -> @expr {
    ret @respan(sp, ast.expr_lit(lit, a));
}

fn identity_fold_expr_cast[ENV](&ENV env, &span sp, @ast.expr e,
                                @ast.ty t, ann a) -> @expr {
    ret @respan(sp, ast.expr_cast(e, t, a));
}

fn identity_fold_expr_if[ENV](&ENV env, &span sp,
                              @expr cond, &block thn,
                              &vec[tup(@expr, block)] elifs,
                              &option.t[block] els, ann a) -> @expr {
    ret @respan(sp, ast.expr_if(cond, thn, elifs, els, a));
}

fn identity_fold_expr_for[ENV](&ENV env, &span sp,
                               @decl d, @expr seq,
                               &block body, ann a) -> @expr {
    ret @respan(sp, ast.expr_for(d, seq, body, a));
}

fn identity_fold_expr_for_each[ENV](&ENV env, &span sp,
                                    @decl d, @expr seq,
                                    &block body, ann a) -> @expr {
    ret @respan(sp, ast.expr_for_each(d, seq, body, a));
}

fn identity_fold_expr_while[ENV](&ENV env, &span sp,
                                 @expr cond, &block body, ann a) -> @expr {
    ret @respan(sp, ast.expr_while(cond, body, a));
}

fn identity_fold_expr_do_while[ENV](&ENV env, &span sp,
                                    &block body, @expr cond, ann a) -> @expr {
    ret @respan(sp, ast.expr_do_while(body, cond, a));
}

fn identity_fold_expr_alt[ENV](&ENV env, &span sp,
                               @expr e, vec[arm] arms, ann a) -> @expr {
    ret @respan(sp, ast.expr_alt(e, arms, a));
}

fn identity_fold_expr_block[ENV](&ENV env, &span sp, &block blk,
                                 ann a) -> @expr {
    ret @respan(sp, ast.expr_block(blk, a));
}

fn identity_fold_expr_assign[ENV](&ENV env, &span sp,
                                  @expr lhs, @expr rhs, ann a)
        -> @expr {
    ret @respan(sp, ast.expr_assign(lhs, rhs, a));
}

fn identity_fold_expr_assign_op[ENV](&ENV env, &span sp, ast.binop op,
                                     @expr lhs, @expr rhs, ann a)
        -> @expr {
    ret @respan(sp, ast.expr_assign_op(op, lhs, rhs, a));
}

fn identity_fold_expr_field[ENV](&ENV env, &span sp,
                                 @expr e, ident i, ann a) -> @expr {
    ret @respan(sp, ast.expr_field(e, i, a));
}

fn identity_fold_expr_index[ENV](&ENV env, &span sp,
                                 @expr e, @expr ix, ann a) -> @expr {
    ret @respan(sp, ast.expr_index(e, ix, a));
}

fn identity_fold_expr_path[ENV](&ENV env, &span sp,
                                &path p, &option.t[def] d,
                                ann a) -> @expr {
    ret @respan(sp, ast.expr_path(p, d, a));
}

fn identity_fold_expr_ext[ENV](&ENV env, &span sp,
                               &path p, vec[@expr] args,
                               option.t[@expr] body,
                               @expr expanded,
                               ann a) -> @expr {
    ret @respan(sp, ast.expr_ext(p, args, body, expanded, a));
}

fn identity_fold_expr_fail[ENV](&ENV env, &span sp) -> @expr {
    ret @respan(sp, ast.expr_fail);
}

fn identity_fold_expr_ret[ENV](&ENV env, &span sp,
                               &option.t[@expr] rv) -> @expr {
    ret @respan(sp, ast.expr_ret(rv));
}

fn identity_fold_expr_put[ENV](&ENV env, &span sp,
                               &option.t[@expr] rv) -> @expr {
    ret @respan(sp, ast.expr_put(rv));
}

fn identity_fold_expr_be[ENV](&ENV env, &span sp, @expr x) -> @expr {
    ret @respan(sp, ast.expr_be(x));
}

fn identity_fold_expr_log[ENV](&ENV e, &span sp, @expr x) -> @expr {
    ret @respan(sp, ast.expr_log(x));
}

fn identity_fold_expr_check_expr[ENV](&ENV e, &span sp, @expr x) -> @expr {
    ret @respan(sp, ast.expr_check_expr(x));
}


// Decl identities.

fn identity_fold_decl_local[ENV](&ENV e, &span sp,
                                 @ast.local local) -> @decl {
    ret @respan(sp, ast.decl_local(local));
}

fn identity_fold_decl_item[ENV](&ENV e, &span sp, @item i) -> @decl {
    ret @respan(sp, ast.decl_item(i));
}


// Pat identities.

fn identity_fold_pat_wild[ENV](&ENV e, &span sp, ann a) -> @pat {
    ret @respan(sp, ast.pat_wild(a));
}

fn identity_fold_pat_lit[ENV](&ENV e, &span sp, @ast.lit lit, ann a) -> @pat {
    ret @respan(sp, ast.pat_lit(lit, a));
}

fn identity_fold_pat_bind[ENV](&ENV e, &span sp, ident i, def_id did, ann a)
        -> @pat {
    ret @respan(sp, ast.pat_bind(i, did, a));
}

fn identity_fold_pat_tag[ENV](&ENV e, &span sp, path p, vec[@pat] args,
                              option.t[ast.variant_def] d, ann a) -> @pat {
    ret @respan(sp, ast.pat_tag(p, args, d, a));
}


// Stmt identities.

fn identity_fold_stmt_decl[ENV](&ENV env, &span sp, @decl d) -> @stmt {
    ret @respan(sp, ast.stmt_decl(d));
}

fn identity_fold_stmt_expr[ENV](&ENV e, &span sp, @expr x) -> @stmt {
    ret @respan(sp, ast.stmt_expr(x));
}


// Item identities.

fn identity_fold_item_const[ENV](&ENV e, &span sp, ident i,
                                 @ty t, @expr ex,
                                 def_id id, ann a) -> @item {
    ret @respan(sp, ast.item_const(i, t, ex, id, a));
}

fn identity_fold_item_fn[ENV](&ENV e, &span sp, ident i,
                              &ast._fn f, vec[ast.ty_param] ty_params,
                              def_id id, ann a) -> @item {
    ret @respan(sp, ast.item_fn(i, f, ty_params, id, a));
}

fn identity_fold_native_item_fn[ENV](&ENV e, &span sp, ident i,
                                     &ast.fn_decl decl,
                                     vec[ast.ty_param] ty_params,
                                     def_id id, ann a) -> @native_item {
    ret @respan(sp, ast.native_item_fn(i, decl, ty_params, id, a));
}

fn identity_fold_item_mod[ENV](&ENV e, &span sp, ident i,
                               &ast._mod m, def_id id) -> @item {
    ret @respan(sp, ast.item_mod(i, m, id));
}

fn identity_fold_item_native_mod[ENV](&ENV e, &span sp, ident i,
                                      &ast.native_mod m, def_id id) -> @item {
    ret @respan(sp, ast.item_native_mod(i, m, id));
}

fn identity_fold_item_ty[ENV](&ENV e, &span sp, ident i,
                              @ty t, vec[ast.ty_param] ty_params,
                              def_id id, ann a) -> @item {
    ret @respan(sp, ast.item_ty(i, t, ty_params, id, a));
}

fn identity_fold_native_item_ty[ENV](&ENV e, &span sp, ident i,
                                     def_id id) -> @native_item {
    ret @respan(sp, ast.native_item_ty(i, id));
}

fn identity_fold_item_tag[ENV](&ENV e, &span sp, ident i,
                               vec[ast.variant] variants,
                               vec[ast.ty_param] ty_params,
                               def_id id) -> @item {
    ret @respan(sp, ast.item_tag(i, variants, ty_params, id));
}

fn identity_fold_item_obj[ENV](&ENV e, &span sp, ident i,
                               &ast._obj ob, vec[ast.ty_param] ty_params,
                               def_id id, ann a) -> @item {
    ret @respan(sp, ast.item_obj(i, ob, ty_params, id, a));
}

// View Item folds.

fn identity_fold_view_item_use[ENV](&ENV e, &span sp, ident i,
                                    vec[@meta_item] meta_items,
                                    def_id id) -> @view_item {
    ret @respan(sp, ast.view_item_use(i, meta_items, id));
}

fn identity_fold_view_item_import[ENV](&ENV e, &span sp, ident i,
                                       vec[ident] is, def_id id,
                                       option.t[def] target_def)
    -> @view_item {
    ret @respan(sp, ast.view_item_import(i, is, id, target_def));
}

fn identity_fold_view_item_export[ENV](&ENV e, &span sp, ident i)
    -> @view_item {
    ret @respan(sp, ast.view_item_export(i));
}

// Additional identities.

fn identity_fold_block[ENV](&ENV e, &span sp, &ast.block_ blk) -> block {
    ret respan(sp, blk);
}

fn identity_fold_fn_decl[ENV](&ENV e,
                              ast.effect effect,
                              vec[arg] inputs,
                              @ty output) -> ast.fn_decl {
    ret rec(effect=effect, inputs=inputs, output=output);
}

fn identity_fold_fn[ENV](&ENV e,
                         &fn_decl decl,
                         ast.proto proto,
                         &block body) -> ast._fn {
    ret rec(decl=decl, proto=proto, body=body);
}

fn identity_fold_mod[ENV](&ENV e, &ast._mod m) -> ast._mod {
    ret m;
}

fn identity_fold_native_mod[ENV](&ENV e,
                                 &ast.native_mod m) -> ast.native_mod {
    ret m;
}

fn identity_fold_crate[ENV](&ENV e, &span sp,
                            vec[@ast.crate_directive] cdirs,
                            &ast._mod m) -> @ast.crate {
    ret @respan(sp, rec(directives=cdirs, module=m));
}

fn identity_fold_obj[ENV](&ENV e,
                          vec[ast.obj_field] fields,
                          vec[@ast.method] methods,
                          option.t[block] dtor) -> ast._obj {
    ret rec(fields=fields, methods=methods, dtor=dtor);
}


// Env update identities.

fn identity_update_env_for_crate[ENV](&ENV e, @ast.crate c) -> ENV {
    ret e;
}

fn identity_update_env_for_item[ENV](&ENV e, @item i) -> ENV {
    ret e;
}

fn identity_update_env_for_native_item[ENV](&ENV e, @native_item i) -> ENV {
    ret e;
}

fn identity_update_env_for_view_item[ENV](&ENV e, @view_item i) -> ENV {
    ret e;
}

fn identity_update_env_for_block[ENV](&ENV e, &block b) -> ENV {
    ret e;
}

fn identity_update_env_for_stmt[ENV](&ENV e, @stmt s) -> ENV {
    ret e;
}

fn identity_update_env_for_decl[ENV](&ENV e, @decl d) -> ENV {
    ret e;
}

fn identity_update_env_for_arm[ENV](&ENV e, &arm a) -> ENV {
    ret e;
}

fn identity_update_env_for_pat[ENV](&ENV e, @pat p) -> ENV {
    ret e;
}

fn identity_update_env_for_expr[ENV](&ENV e, @expr x) -> ENV {
    ret e;
}

fn identity_update_env_for_ty[ENV](&ENV e, @ty t) -> ENV {
    ret e;
}


// Always-true traversal control fn.

fn always_keep_going[ENV](&ENV e) -> bool {
    ret true;
}


fn new_identity_fold[ENV]() -> ast_fold[ENV] {
    ret @rec
        (
         fold_path       = bind identity_fold_path[ENV](_,_,_),

         fold_ty_nil     = bind identity_fold_ty_nil[ENV](_,_),
         fold_ty_bool    = bind identity_fold_ty_bool[ENV](_,_),
         fold_ty_int     = bind identity_fold_ty_int[ENV](_,_),
         fold_ty_uint    = bind identity_fold_ty_uint[ENV](_,_),
         fold_ty_machine = bind identity_fold_ty_machine[ENV](_,_,_),
         fold_ty_char    = bind identity_fold_ty_char[ENV](_,_),
         fold_ty_str     = bind identity_fold_ty_str[ENV](_,_),
         fold_ty_box     = bind identity_fold_ty_box[ENV](_,_,_),
         fold_ty_vec     = bind identity_fold_ty_vec[ENV](_,_,_),
         fold_ty_tup     = bind identity_fold_ty_tup[ENV](_,_,_),
         fold_ty_rec     = bind identity_fold_ty_rec[ENV](_,_,_),
         fold_ty_obj     = bind identity_fold_ty_obj[ENV](_,_,_),
         fold_ty_fn      = bind identity_fold_ty_fn[ENV](_,_,_,_,_),
         fold_ty_path    = bind identity_fold_ty_path[ENV](_,_,_,_),
         fold_ty_mutable = bind identity_fold_ty_mutable[ENV](_,_,_),

         fold_expr_vec    = bind identity_fold_expr_vec[ENV](_,_,_,_),
         fold_expr_tup    = bind identity_fold_expr_tup[ENV](_,_,_,_),
         fold_expr_rec    = bind identity_fold_expr_rec[ENV](_,_,_,_,_),
         fold_expr_call   = bind identity_fold_expr_call[ENV](_,_,_,_,_),
         fold_expr_bind   = bind identity_fold_expr_bind[ENV](_,_,_,_,_),
         fold_expr_binary = bind identity_fold_expr_binary[ENV](_,_,_,_,_,_),
         fold_expr_unary  = bind identity_fold_expr_unary[ENV](_,_,_,_,_),
         fold_expr_lit    = bind identity_fold_expr_lit[ENV](_,_,_,_),
         fold_expr_cast   = bind identity_fold_expr_cast[ENV](_,_,_,_,_),
         fold_expr_if     = bind identity_fold_expr_if[ENV](_,_,_,_,_,_,_),
         fold_expr_for    = bind identity_fold_expr_for[ENV](_,_,_,_,_,_),
         fold_expr_for_each
             = bind identity_fold_expr_for_each[ENV](_,_,_,_,_,_),
         fold_expr_while  = bind identity_fold_expr_while[ENV](_,_,_,_,_),
         fold_expr_do_while
                          = bind identity_fold_expr_do_while[ENV](_,_,_,_,_),
         fold_expr_alt    = bind identity_fold_expr_alt[ENV](_,_,_,_,_),
         fold_expr_block  = bind identity_fold_expr_block[ENV](_,_,_,_),
         fold_expr_assign = bind identity_fold_expr_assign[ENV](_,_,_,_,_),
         fold_expr_assign_op
                       = bind identity_fold_expr_assign_op[ENV](_,_,_,_,_,_),
         fold_expr_field  = bind identity_fold_expr_field[ENV](_,_,_,_,_),
         fold_expr_index  = bind identity_fold_expr_index[ENV](_,_,_,_,_),
         fold_expr_path   = bind identity_fold_expr_path[ENV](_,_,_,_,_),
         fold_expr_ext    = bind identity_fold_expr_ext[ENV](_,_,_,_,_,_,_),
         fold_expr_fail   = bind identity_fold_expr_fail[ENV](_,_),
         fold_expr_ret    = bind identity_fold_expr_ret[ENV](_,_,_),
         fold_expr_put    = bind identity_fold_expr_put[ENV](_,_,_),
         fold_expr_be     = bind identity_fold_expr_be[ENV](_,_,_),
         fold_expr_log    = bind identity_fold_expr_log[ENV](_,_,_),
         fold_expr_check_expr
                          = bind identity_fold_expr_check_expr[ENV](_,_,_),

         fold_decl_local  = bind identity_fold_decl_local[ENV](_,_,_),
         fold_decl_item   = bind identity_fold_decl_item[ENV](_,_,_),

         fold_pat_wild    = bind identity_fold_pat_wild[ENV](_,_,_),
         fold_pat_lit     = bind identity_fold_pat_lit[ENV](_,_,_,_),
         fold_pat_bind    = bind identity_fold_pat_bind[ENV](_,_,_,_,_),
         fold_pat_tag     = bind identity_fold_pat_tag[ENV](_,_,_,_,_,_),

         fold_stmt_decl   = bind identity_fold_stmt_decl[ENV](_,_,_),
         fold_stmt_expr   = bind identity_fold_stmt_expr[ENV](_,_,_),

         fold_item_const= bind identity_fold_item_const[ENV](_,_,_,_,_,_,_),
         fold_item_fn   = bind identity_fold_item_fn[ENV](_,_,_,_,_,_,_),
         fold_native_item_fn =
             bind identity_fold_native_item_fn[ENV](_,_,_,_,_,_,_),
         fold_item_mod  = bind identity_fold_item_mod[ENV](_,_,_,_,_),
         fold_item_native_mod =
             bind identity_fold_item_native_mod[ENV](_,_,_,_,_),
         fold_item_ty   = bind identity_fold_item_ty[ENV](_,_,_,_,_,_,_),
         fold_native_item_ty =
             bind identity_fold_native_item_ty[ENV](_,_,_,_),
         fold_item_tag  = bind identity_fold_item_tag[ENV](_,_,_,_,_,_),
         fold_item_obj  = bind identity_fold_item_obj[ENV](_,_,_,_,_,_,_),

         fold_view_item_use =
             bind identity_fold_view_item_use[ENV](_,_,_,_,_),
         fold_view_item_import =
             bind identity_fold_view_item_import[ENV](_,_,_,_,_,_),
         fold_view_item_export =
             bind identity_fold_view_item_export[ENV](_,_,_),

         fold_block = bind identity_fold_block[ENV](_,_,_),
         fold_fn = bind identity_fold_fn[ENV](_,_,_,_),
         fold_fn_decl = bind identity_fold_fn_decl[ENV](_,_,_,_),
         fold_mod = bind identity_fold_mod[ENV](_,_),
         fold_native_mod = bind identity_fold_native_mod[ENV](_,_),
         fold_crate = bind identity_fold_crate[ENV](_,_,_,_),
         fold_obj = bind identity_fold_obj[ENV](_,_,_,_),

         update_env_for_crate = bind identity_update_env_for_crate[ENV](_,_),
         update_env_for_item = bind identity_update_env_for_item[ENV](_,_),
         update_env_for_native_item =
             bind identity_update_env_for_native_item[ENV](_,_),
         update_env_for_view_item =
             bind identity_update_env_for_view_item[ENV](_,_),
         update_env_for_block = bind identity_update_env_for_block[ENV](_,_),
         update_env_for_stmt = bind identity_update_env_for_stmt[ENV](_,_),
         update_env_for_decl = bind identity_update_env_for_decl[ENV](_,_),
         update_env_for_pat = bind identity_update_env_for_pat[ENV](_,_),
         update_env_for_arm = bind identity_update_env_for_arm[ENV](_,_),
         update_env_for_expr = bind identity_update_env_for_expr[ENV](_,_),
         update_env_for_ty = bind identity_update_env_for_ty[ENV](_,_),

         keep_going = bind always_keep_going[ENV](_)
         );
}


//
// Local Variables:
// mode: rust
// fill-column: 78;
// indent-tabs-mode: nil
// c-basic-offset: 4
// buffer-file-coding-system: utf-8-unix
// compile-command: "make -k -C ../.. 2>&1 | sed -e 's/\\/x\\//x:\\//g'";
// End:
//
