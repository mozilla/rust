import result::result;
import either::{either, left, right};
import std::map::{hashmap, str_hash};
import token::{can_begin_expr, is_ident, is_plain_ident};
import codemap::{span,fss_none};
import util::interner;
import ast_util::{spanned, mk_sp, ident_to_path};
import lexer::reader;
import prec::{op_spec, as_prec};
import attr::{parse_outer_attrs_or_ext,
              parse_inner_attrs_and_next,
              parse_outer_attributes,
              parse_optional_meta};
import common::*;

export expect;
export file_type;
export mk_item;
export restriction;
export parser;
export parse_crate_directives;
export parse_crate_mod;
export parse_expr;
export parse_item;
export parse_mod_items;
export parse_pat;
export parse_seq;
export parse_stmt;
export parse_ty;
export parse_lit;
export parse_syntax_ext_naked;

// FIXME: #ast expects to find this here but it's actually defined in `parse`
// Fixing this will be easier when we have export decls on individual items --
// then parse can export this publicly, and everything else crate-visibly.
// (See #1893)
import parse_from_source_str;
export parse_from_source_str;

enum restriction {
    UNRESTRICTED,
    RESTRICT_STMT_EXPR,
    RESTRICT_NO_CALL_EXPRS,
    RESTRICT_NO_BAR_OP,
}

enum file_type { CRATE_FILE, SOURCE_FILE, }

type parser = @{
    sess: parse_sess,
    cfg: ast::crate_cfg,
    file_type: file_type,
    mut token: token::token,
    mut span: span,
    mut last_span: span,
    mut buffer: [{tok: token::token, span: span}],
    mut restriction: restriction,
    reader: reader,
    binop_precs: @[op_spec],
    keywords: hashmap<str, ()>,
    bad_expr_words: hashmap<str, ()>
};

impl parser for parser {
    fn bump() {
        self.last_span = self.span;
        if vec::len(self.buffer) == 0u {
            let next = lexer::next_token(self.reader);
            self.token = next.tok;
            self.span = ast_util::mk_sp(next.chpos, self.reader.chpos);
        } else {
            let next = vec::pop(self.buffer);
            self.token = next.tok;
            self.span = next.span;
        }
    }
    fn swap(next: token::token, lo: uint, hi: uint) {
        self.token = next;
        self.span = ast_util::mk_sp(lo, hi);
    }
    fn look_ahead(distance: uint) -> token::token {
        while vec::len(self.buffer) < distance {
            let next = lexer::next_token(self.reader);
            let sp = ast_util::mk_sp(next.chpos, self.reader.chpos);
            self.buffer = [{tok: next.tok, span: sp}] + self.buffer;
        }
        ret self.buffer[distance - 1u].tok;
    }
    fn fatal(m: str) -> ! {
        self.sess.span_diagnostic.span_fatal(self.span, m)
    }
    fn span_fatal(sp: span, m: str) -> ! {
        self.sess.span_diagnostic.span_fatal(sp, m)
    }
    fn bug(m: str) -> ! {
        self.sess.span_diagnostic.span_bug(self.span, m)
    }
    fn warn(m: str) {
        self.sess.span_diagnostic.span_warn(self.span, m)
    }
    fn get_str(i: token::str_num) -> str {
        interner::get(*self.reader.interner, i)
    }
    fn get_id() -> node_id { next_node_id(self.sess) }
}

fn parse_ty_fn(p: parser) -> ast::fn_decl {
    fn parse_fn_input_ty(p: parser) -> ast::arg {
        let mode = parse_arg_mode(p);
        let name = if is_plain_ident(p.token)
            && p.look_ahead(1u) == token::COLON {

            let name = parse_value_ident(p);
            p.bump();
            name
        } else { "" };
        ret {mode: mode, ty: parse_ty(p, false), ident: name, id: p.get_id()};
    }
    let inputs =
        parse_seq(token::LPAREN, token::RPAREN, seq_sep(token::COMMA),
                  parse_fn_input_ty, p);
    // FIXME: constrs is empty because right now, higher-order functions
    // can't have constrained types.
    // Not sure whether that would be desirable anyway. See #34 for the
    // story on constrained types.
    let constrs: [@ast::constr] = [];
    let (ret_style, ret_ty) = parse_ret_ty(p);
    ret {inputs: inputs.node, output: ret_ty,
         purity: ast::impure_fn, cf: ret_style,
         constraints: constrs};
}

fn parse_ty_methods(p: parser) -> [ast::ty_method] {
    parse_seq(token::LBRACE, token::RBRACE, seq_sep_none(), {|p|
        let attrs = parse_outer_attributes(p);
        let flo = p.span.lo;
        let pur = parse_fn_purity(p);
        let ident = parse_method_name(p);
        let tps = parse_ty_params(p);
        let d = parse_ty_fn(p), fhi = p.last_span.hi;
        expect(p, token::SEMI);
        {ident: ident, attrs: attrs, decl: {purity: pur with d}, tps: tps,
         span: ast_util::mk_sp(flo, fhi)}
    }, p).node
}

fn parse_mt(p: parser) -> ast::mt {
    let mutbl = parse_mutability(p);
    let t = parse_ty(p, false);
    ret {ty: t, mutbl: mutbl};
}

fn parse_ty_field(p: parser) -> ast::ty_field {
    let lo = p.span.lo;
    let mutbl = parse_mutability(p);
    let id = parse_ident(p);
    expect(p, token::COLON);
    let ty = parse_ty(p, false);
    ret spanned(lo, ty.span.hi, {ident: id, mt: {ty: ty, mutbl: mutbl}});
}

// if i is the jth ident in args, return j
// otherwise, fail
fn ident_index(p: parser, args: [ast::arg], i: ast::ident) -> uint {
    let mut j = 0u;
    for args.each {|a| if a.ident == i { ret j; } j += 1u; }
    p.fatal("unbound variable `" + i + "` in constraint arg");
}

fn parse_type_constr_arg(p: parser) -> @ast::ty_constr_arg {
    let sp = p.span;
    let mut carg = ast::carg_base;
    expect(p, token::BINOP(token::STAR));
    if p.token == token::DOT {
        // "*..." notation for record fields
        p.bump();
        let pth = parse_path(p);
        carg = ast::carg_ident(pth);
    }
    // No literals yet, I guess?
    ret @{node: carg, span: sp};
}

fn parse_constr_arg(args: [ast::arg], p: parser) -> @ast::constr_arg {
    let sp = p.span;
    let mut carg = ast::carg_base;
    if p.token == token::BINOP(token::STAR) {
        p.bump();
    } else {
        let i: ast::ident = parse_value_ident(p);
        carg = ast::carg_ident(ident_index(p, args, i));
    }
    ret @{node: carg, span: sp};
}

fn parse_ty_constr(fn_args: [ast::arg], p: parser) -> @ast::constr {
    let lo = p.span.lo;
    let path = parse_path(p);
    let args: {node: [@ast::constr_arg], span: span} =
        parse_seq(token::LPAREN, token::RPAREN, seq_sep(token::COMMA),
                  {|p| parse_constr_arg(fn_args, p)}, p);
    ret @spanned(lo, args.span.hi,
                 {path: path, args: args.node, id: p.get_id()});
}

fn parse_constr_in_type(p: parser) -> @ast::ty_constr {
    let lo = p.span.lo;
    let path = parse_path(p);
    let args: [@ast::ty_constr_arg] =
        parse_seq(token::LPAREN, token::RPAREN, seq_sep(token::COMMA),
                  parse_type_constr_arg, p).node;
    let hi = p.span.lo;
    let tc: ast::ty_constr_ = {path: path, args: args, id: p.get_id()};
    ret @spanned(lo, hi, tc);
}


fn parse_constrs<T: copy>(pser: fn(parser) -> @ast::constr_general<T>,
                         p: parser) ->
   [@ast::constr_general<T>] {
    let mut constrs: [@ast::constr_general<T>] = [];
    loop {
        let constr = pser(p);
        constrs += [constr];
        if p.token == token::COMMA { p.bump(); } else { ret constrs; }
    };
}

fn parse_type_constraints(p: parser) -> [@ast::ty_constr] {
    ret parse_constrs(parse_constr_in_type, p);
}

fn parse_ty_postfix(orig_t: ast::ty_, p: parser, colons_before_params: bool,
                    lo: uint) -> @ast::ty {


    fn mk_ty(p: parser, t: ast::ty_, lo: uint, hi: uint) -> @ast::ty {
        @{id: p.get_id(),
          node: t,
          span: ast_util::mk_sp(lo, hi)}
    }

    if p.token == token::BINOP(token::SLASH) {
        let orig_hi = p.last_span.hi;
        alt maybe_parse_vstore(p) {
          none { }
          some(v) {
            let t = ast::ty_vstore(mk_ty(p, orig_t, lo, orig_hi), v);
            ret mk_ty(p, t, lo, p.last_span.hi);
          }
        }
    }

    if colons_before_params && p.token == token::MOD_SEP {
        p.bump();
        expect(p, token::LT);
    } else if !colons_before_params && p.token == token::LT {
        p.bump();
    } else {
        ret mk_ty(p, orig_t, lo, p.last_span.hi);
    }

    // If we're here, we have explicit type parameter instantiation.
    let seq = parse_seq_to_gt(some(token::COMMA), {|p| parse_ty(p, false)},
                              p);

    alt orig_t {
      ast::ty_path(pth, ann) {
        ret mk_ty(p, ast::ty_path(@spanned(lo, p.last_span.hi,
                                           {global: pth.node.global,
                                            idents: pth.node.idents,
                                            types: seq}), ann),
                  lo, p.last_span.hi);
      }
      _ { p.fatal("type parameter instantiation only allowed for paths"); }
    }
}

fn parse_ret_ty(p: parser) -> (ast::ret_style, @ast::ty) {
    ret if eat(p, token::RARROW) {
        let lo = p.span.lo;
        if eat(p, token::NOT) {
            (ast::noreturn, @{id: p.get_id(),
                              node: ast::ty_bot,
                              span: ast_util::mk_sp(lo, p.last_span.hi)})
        } else {
            (ast::return_val, parse_ty(p, false))
        }
    } else {
        let pos = p.span.lo;
        (ast::return_val, @{id: p.get_id(),
                            node: ast::ty_nil,
                            span: ast_util::mk_sp(pos, pos)})
    }
}

fn region_from_name(p: parser, s: option<str>) -> ast::region {
    let r = alt s {
      some (string) {
        // FIXME: To be consistent with our type resolution, the
        // static region should probably be resolved during type
        // checking, not in the parser. (Issue #2256)
        if string == "static" {
            ast::re_static
        } else {
            ast::re_named(string)
        }
      }
      none { ast::re_anon }
    };

    {id: p.get_id(), node: r}
}

fn parse_region(p: parser) -> ast::region {
    let name =
        alt p.token {
          token::IDENT(sid, _) if p.look_ahead(1u) == token::DOT {
            p.bump(); p.bump();
            some(p.get_str(sid))
          }
          _ { none }
        };
    region_from_name(p, name)
}

fn parse_ty(p: parser, colons_before_params: bool) -> @ast::ty {
    let lo = p.span.lo;

    alt have_dollar(p) {
      some(e) {
        ret @{id: p.get_id(),
              node: ast::ty_mac(spanned(lo, p.span.hi, e)),
              span: ast_util::mk_sp(lo, p.span.hi)};
      }
      none {}
    }

    let t = if p.token == token::LPAREN {
        p.bump();
        if p.token == token::RPAREN {
            p.bump();
            ast::ty_nil
        } else {
            let mut ts = [parse_ty(p, false)];
            while p.token == token::COMMA {
                p.bump();
                ts += [parse_ty(p, false)];
            }
            let t = if vec::len(ts) == 1u { ts[0].node }
                    else { ast::ty_tup(ts) };
            expect(p, token::RPAREN);
            t
        }
    } else if p.token == token::AT {
        p.bump();
        ast::ty_box(parse_mt(p))
    } else if p.token == token::TILDE {
        p.bump();
        ast::ty_uniq(parse_mt(p))
    } else if p.token == token::BINOP(token::STAR) {
        p.bump();
        ast::ty_ptr(parse_mt(p))
    } else if p.token == token::LBRACE {
        let elems =
            parse_seq(token::LBRACE, token::RBRACE, seq_sep_opt(token::COMMA),
                      parse_ty_field, p);
        if vec::len(elems.node) == 0u { unexpected_last(p, token::RBRACE); }
        let hi = elems.span.hi;

        let t = ast::ty_rec(elems.node);
        if p.token == token::COLON {
            p.bump();
            ast::ty_constr(@{id: p.get_id(),
                             node: t,
                             span: ast_util::mk_sp(lo, hi)},
                           parse_type_constraints(p))
        } else { t }
    } else if p.token == token::LBRACKET {
        expect(p, token::LBRACKET);
        let t = ast::ty_vec(parse_mt(p));
        expect(p, token::RBRACKET);
        t
    } else if p.token == token::BINOP(token::AND) {
        p.bump();
        let region = parse_region(p);
        let mt = parse_mt(p);
        ast::ty_rptr(region, mt)
    } else if eat_word(p, "fn") {
        let proto = parse_fn_ty_proto(p);
        alt proto {
          ast::proto_bare { p.warn("fn is deprecated, use native fn"); }
          _ { /* fallthrough */ }
        }
        ast::ty_fn(proto, parse_ty_fn(p))
    } else if eat_word(p, "native") {
        expect_word(p, "fn");
        ast::ty_fn(ast::proto_bare, parse_ty_fn(p))
    } else if p.token == token::MOD_SEP || is_ident(p.token) {
        let path = parse_path(p);
        ast::ty_path(path, p.get_id())
    } else { p.fatal("expecting type"); };
    ret parse_ty_postfix(t, p, colons_before_params, lo);
}

fn parse_arg_mode(p: parser) -> ast::mode {
    if eat(p, token::BINOP(token::AND)) {
        ast::expl(ast::by_mutbl_ref)
    } else if eat(p, token::BINOP(token::MINUS)) {
        ast::expl(ast::by_move)
    } else if eat(p, token::ANDAND) {
        ast::expl(ast::by_ref)
    } else if eat(p, token::BINOP(token::PLUS)) {
        if eat(p, token::BINOP(token::PLUS)) {
            ast::expl(ast::by_val)
        } else {
            ast::expl(ast::by_copy)
        }
    } else { ast::infer(p.get_id()) }
}

fn parse_arg(p: parser) -> ast::arg {
    let m = parse_arg_mode(p);
    let i = parse_value_ident(p);
    expect(p, token::COLON);
    let t = parse_ty(p, false);
    ret {mode: m, ty: t, ident: i, id: p.get_id()};
}

fn parse_fn_block_arg(p: parser) -> ast::arg {
    let m = parse_arg_mode(p);
    let i = parse_value_ident(p);
    let t = if eat(p, token::COLON) {
                parse_ty(p, false)
            } else {
                @{id: p.get_id(),
                  node: ast::ty_infer,
                  span: ast_util::mk_sp(p.span.lo, p.span.hi)}
            };
    ret {mode: m, ty: t, ident: i, id: p.get_id()};
}

fn have_dollar(p: parser) -> option<ast::mac_> {
    alt p.token {
      token::DOLLAR_NUM(num) {
        p.bump();
        some(ast::mac_var(num))
      }
      token::DOLLAR_LPAREN {
        let lo = p.span.lo;
        p.bump();
        let e = parse_expr(p);
        expect(p, token::RPAREN);
        let hi = p.last_span.hi;
        some(ast::mac_aq(ast_util::mk_sp(lo,hi), e))
      }
      _ {none}
    }
}

fn maybe_parse_vstore(p: parser) -> option<ast::vstore> {
    if p.token == token::BINOP(token::SLASH) {
        p.bump();
        alt p.token {
          token::AT {
            p.bump(); some(ast::vstore_box)
          }
          token::TILDE {
            p.bump(); some(ast::vstore_uniq)
          }
          token::UNDERSCORE {
            p.bump(); some(ast::vstore_fixed(none))
          }
          token::LIT_INT(i, ast::ty_i) if i >= 0i64 {
            p.bump(); some(ast::vstore_fixed(some(i as uint)))
          }
          token::BINOP(token::AND) {
            p.bump();
            alt p.token {
              token::IDENT(sid, _) {
                p.bump();
                let n = p.get_str(sid);
                some(ast::vstore_slice(region_from_name(p, some(n))))
              }
              _ {
                some(ast::vstore_slice(region_from_name(p, none)))
              }
            }
          }
          _ {
            none
          }
        }
    } else {
        none
    }
}

fn lit_from_token(p: parser, tok: token::token) -> ast::lit_ {
    alt tok {
      token::LIT_INT(i, it) { ast::lit_int(i, it) }
      token::LIT_UINT(u, ut) { ast::lit_uint(u, ut) }
      token::LIT_FLOAT(s, ft) { ast::lit_float(p.get_str(s), ft) }
      token::LIT_STR(s) { ast::lit_str(p.get_str(s)) }
      token::LPAREN { expect(p, token::RPAREN); ast::lit_nil }
      _ { unexpected_last(p, tok); }
    }
}

fn parse_lit(p: parser) -> ast::lit {
    let lo = p.span.lo;
    let lit = if eat_word(p, "true") {
        ast::lit_bool(true)
    } else if eat_word(p, "false") {
        ast::lit_bool(false)
    } else {
        let tok = p.token;
        p.bump();
        lit_from_token(p, tok)
    };
    ret {node: lit, span: ast_util::mk_sp(lo, p.last_span.hi)};
}

fn parse_path(p: parser) -> @ast::path {
    let lo = p.span.lo;
    let global = eat(p, token::MOD_SEP);
    let mut ids = [parse_ident(p)];
    while p.look_ahead(1u) != token::LT && eat(p, token::MOD_SEP) {
        ids += [parse_ident(p)];
    }
    ret @spanned(lo, p.last_span.hi,
                 {global: global, idents: ids, types: []});
}

fn parse_value_path(p: parser) -> @ast::path {
    let pt = parse_path(p);
    let last_word = pt.node.idents[vec::len(pt.node.idents)-1u];
    if p.bad_expr_words.contains_key(last_word) {
        p.fatal("found " + last_word + " in expression position");
    }
    pt
}

fn parse_path_and_ty_param_substs(p: parser, colons: bool) -> @ast::path {
    let lo = p.span.lo;
    let path = parse_path(p);
    let b = if colons {
                eat(p, token::MOD_SEP)
            } else {
                p.token == token::LT
            };
    if b {
        let seq = parse_seq_lt_gt(some(token::COMMA),
                                  {|p| parse_ty(p, false)}, p);
        @spanned(lo, seq.span.hi, {types: seq.node with path.node})
    } else { path }
}

fn parse_mutability(p: parser) -> ast::mutability {
    if eat_word(p, "mut") {
        ast::m_mutbl
    } else if eat_word(p, "mut") {
        ast::m_mutbl
    } else if eat_word(p, "const") {
        ast::m_const
    } else {
        ast::m_imm
    }
}

fn parse_field(p: parser, sep: token::token) -> ast::field {
    let lo = p.span.lo;
    let m = parse_mutability(p);
    let i = parse_ident(p);
    expect(p, sep);
    let e = parse_expr(p);
    ret spanned(lo, e.span.hi, {mutbl: m, ident: i, expr: e});
}

fn mk_expr(p: parser, lo: uint, hi: uint, node: ast::expr_) -> @ast::expr {
    ret @{id: p.get_id(), node: node, span: ast_util::mk_sp(lo, hi)};
}

fn mk_mac_expr(p: parser, lo: uint, hi: uint, m: ast::mac_) -> @ast::expr {
    ret @{id: p.get_id(),
          node: ast::expr_mac({node: m, span: ast_util::mk_sp(lo, hi)}),
          span: ast_util::mk_sp(lo, hi)};
}

fn mk_lit_u32(p: parser, i: u32) -> @ast::expr {
    let span = p.span;
    let lv_lit = @{node: ast::lit_uint(i as u64, ast::ty_u32),
                   span: span};

    ret @{id: p.get_id(), node: ast::expr_lit(lv_lit), span: span};
}

// We don't allow single-entry tuples in the true AST; that indicates a
// parenthesized expression.  However, we preserve them temporarily while
// parsing because `(while{...})+3` parses differently from `while{...}+3`.
//
// To reflect the fact that the @ast::expr is not a true expr that should be
// part of the AST, we wrap such expressions in the pexpr enum.  They
// can then be converted to true expressions by a call to `to_expr()`.
enum pexpr {
    pexpr(@ast::expr),
}

fn mk_pexpr(p: parser, lo: uint, hi: uint, node: ast::expr_) -> pexpr {
    ret pexpr(mk_expr(p, lo, hi, node));
}

fn to_expr(e: pexpr) -> @ast::expr {
    alt e.node {
      ast::expr_tup(es) if vec::len(es) == 1u { es[0u] }
      _ { *e }
    }
}

fn parse_bottom_expr(p: parser) -> pexpr {
    let lo = p.span.lo;
    let mut hi = p.span.hi;

    let mut ex: ast::expr_;

    alt have_dollar(p) {
      some(x) {ret pexpr(mk_mac_expr(p, lo, p.span.hi, x));}
      _ {}
    }

    if p.token == token::LPAREN {
        p.bump();
        if p.token == token::RPAREN {
            hi = p.span.hi;
            p.bump();
            let lit = @spanned(lo, hi, ast::lit_nil);
            ret mk_pexpr(p, lo, hi, ast::expr_lit(lit));
        }
        let mut es = [parse_expr(p)];
        while p.token == token::COMMA { p.bump(); es += [parse_expr(p)]; }
        hi = p.span.hi;
        expect(p, token::RPAREN);

        // Note: we retain the expr_tup() even for simple
        // parenthesized expressions, but only for a "little while".
        // This is so that wrappers around parse_bottom_expr()
        // can tell whether the expression was parenthesized or not,
        // which affects expr_is_complete().
        ret mk_pexpr(p, lo, hi, ast::expr_tup(es));
    } else if p.token == token::LBRACE {
        p.bump();
        if is_word(p, "mut") ||
               is_plain_ident(p.token) && p.look_ahead(1u) == token::COLON {
            let mut fields = [parse_field(p, token::COLON)];
            let mut base = none;
            while p.token != token::RBRACE {
                if eat_word(p, "with") { base = some(parse_expr(p)); break; }
                expect(p, token::COMMA);
                if p.token == token::RBRACE {
                    // record ends by an optional trailing comma
                    break;
                }
                fields += [parse_field(p, token::COLON)];
            }
            hi = p.span.hi;
            expect(p, token::RBRACE);
            ex = ast::expr_rec(fields, base);
        } else if token::is_bar(p.token) {
            ret pexpr(parse_fn_block_expr(p));
        } else {
            let blk = parse_block_tail(p, lo, ast::default_blk);
            ret mk_pexpr(p, blk.span.lo, blk.span.hi, ast::expr_block(blk));
        }
    } else if eat_word(p, "new") {
        expect(p, token::LPAREN);
        let r = parse_expr(p);
        expect(p, token::RPAREN);
        let v = parse_expr(p);
        ret mk_pexpr(p, lo, p.span.hi,
                     ast::expr_new(r, p.get_id(), v));
    } else if eat_word(p, "if") {
        ret pexpr(parse_if_expr(p));
    } else if eat_word(p, "for") {
        ret pexpr(parse_for_expr(p));
    } else if eat_word(p, "while") {
        ret pexpr(parse_while_expr(p));
    } else if eat_word(p, "do") {
        ret pexpr(parse_do_while_expr(p));
    } else if eat_word(p, "loop") {
        ret pexpr(parse_loop_expr(p));
    } else if eat_word(p, "alt") {
        ret pexpr(parse_alt_expr(p));
    } else if eat_word(p, "fn") {
        let proto = parse_fn_ty_proto(p);
        alt proto {
          ast::proto_bare { p.fatal("fn expr are deprecated, use fn@"); }
          ast::proto_any { p.fatal("fn* cannot be used in an expression"); }
          _ { /* fallthrough */ }
        }
        ret pexpr(parse_fn_expr(p, proto));
    } else if eat_word(p, "unchecked") {
        ret pexpr(parse_block_expr(p, lo, ast::unchecked_blk));
    } else if eat_word(p, "unsafe") {
        ret pexpr(parse_block_expr(p, lo, ast::unsafe_blk));
    } else if p.token == token::LBRACKET {
        p.bump();
        let mutbl = parse_mutability(p);
        let es =
            parse_seq_to_end(token::RBRACKET, seq_sep(token::COMMA),
                             parse_expr, p);
        hi = p.span.hi;
        ex = ast::expr_vec(es, mutbl);
    } else if p.token == token::POUND_LT {
        p.bump();
        let ty = parse_ty(p, false);
        expect(p, token::GT);

        /* hack: early return to take advantage of specialized function */
        ret pexpr(mk_mac_expr(p, lo, p.span.hi,
                              ast::mac_embed_type(ty)));
    } else if p.token == token::POUND_LBRACE {
        p.bump();
        let blk = ast::mac_embed_block(
            parse_block_tail(p, lo, ast::default_blk));
        ret pexpr(mk_mac_expr(p, lo, p.span.hi, blk));
    } else if p.token == token::ELLIPSIS {
        p.bump();
        ret pexpr(mk_mac_expr(p, lo, p.span.hi, ast::mac_ellipsis));
    } else if eat_word(p, "bind") {
        let e = parse_expr_res(p, RESTRICT_NO_CALL_EXPRS);
        let es =
            parse_seq(token::LPAREN, token::RPAREN, seq_sep(token::COMMA),
                      parse_expr_or_hole, p);
        hi = es.span.hi;
        ex = ast::expr_bind(e, es.node);
    } else if p.token == token::POUND {
        let ex_ext = parse_syntax_ext(p);
        hi = ex_ext.span.hi;
        ex = ex_ext.node;
    } else if eat_word(p, "fail") {
        if can_begin_expr(p.token) {
            let e = parse_expr(p);
            hi = e.span.hi;
            ex = ast::expr_fail(some(e));
        } else { ex = ast::expr_fail(none); }
    } else if eat_word(p, "log") {
        expect(p, token::LPAREN);
        let lvl = parse_expr(p);
        expect(p, token::COMMA);
        let e = parse_expr(p);
        ex = ast::expr_log(2, lvl, e);
        hi = p.span.hi;
        expect(p, token::RPAREN);
    } else if eat_word(p, "assert") {
        let e = parse_expr(p);
        ex = ast::expr_assert(e);
        hi = e.span.hi;
    } else if eat_word(p, "check") {
        /* Should be a predicate (pure boolean function) applied to
           arguments that are all either slot variables or literals.
           but the typechecker enforces that. */
        let e = parse_expr(p);
        hi = e.span.hi;
        ex = ast::expr_check(ast::checked_expr, e);
    } else if eat_word(p, "claim") {
        /* Same rules as check, except that if check-claims
         is enabled (a command-line flag), then the parser turns
        claims into check */

        let e = parse_expr(p);
        hi = e.span.hi;
        ex = ast::expr_check(ast::claimed_expr, e);
    } else if eat_word(p, "ret") {
        if can_begin_expr(p.token) {
            let e = parse_expr(p);
            hi = e.span.hi;
            ex = ast::expr_ret(some(e));
        } else { ex = ast::expr_ret(none); }
    } else if eat_word(p, "break") {
        ex = ast::expr_break;
        hi = p.span.hi;
    } else if eat_word(p, "cont") {
        ex = ast::expr_cont;
        hi = p.span.hi;
    } else if eat_word(p, "be") {
        let e = parse_expr(p);
        hi = e.span.hi;
        ex = ast::expr_be(e);
    } else if eat_word(p, "copy") {
        let e = parse_expr(p);
        ex = ast::expr_copy(e);
        hi = e.span.hi;
    } else if p.token == token::MOD_SEP ||
                  is_ident(p.token) && !is_word(p, "true") &&
                      !is_word(p, "false") {
        check_bad_word(p);
        let pth = parse_path_and_ty_param_substs(p, true);
        hi = pth.span.hi;
        ex = ast::expr_path(pth);
    } else {
        let lit = parse_lit(p);
        hi = lit.span.hi;
        ex = ast::expr_lit(@lit);
    }

    // Vstore is legal following expr_lit(lit_str(...)) and expr_vec(...)
    // only.
    alt ex {
      ast::expr_lit(@{node: ast::lit_str(_), span: _}) |
      ast::expr_vec(_, _)  {
        alt maybe_parse_vstore(p) {
          none { }
          some(v) {
            hi = p.span.hi;
            ex = ast::expr_vstore(mk_expr(p, lo, hi, ex), v);
          }
        }
      }
      _ { }
    }

    ret mk_pexpr(p, lo, hi, ex);
}

fn parse_block_expr(p: parser,
                    lo: uint,
                    blk_mode: ast::blk_check_mode) -> @ast::expr {
    expect(p, token::LBRACE);
    let blk = parse_block_tail(p, lo, blk_mode);
    ret mk_expr(p, blk.span.lo, blk.span.hi, ast::expr_block(blk));
}

fn parse_syntax_ext(p: parser) -> @ast::expr {
    let lo = p.span.lo;
    expect(p, token::POUND);
    ret parse_syntax_ext_naked(p, lo);
}

fn parse_syntax_ext_naked(p: parser, lo: uint) -> @ast::expr {
    alt p.token {
      token::IDENT(_, _) {}
      _ { p.fatal("expected a syntax expander name"); }
    }
    let pth = parse_path(p);
    //temporary for a backwards-compatible cycle:
    let sep = seq_sep(token::COMMA);
    let mut e = none;
    if (p.token == token::LPAREN || p.token == token::LBRACKET) {
        let es =
            if p.token == token::LPAREN {
                parse_seq(token::LPAREN, token::RPAREN,
                          sep, parse_expr, p)
            } else {
                parse_seq(token::LBRACKET, token::RBRACKET,
                          sep, parse_expr, p)
            };
        let hi = es.span.hi;
        e = some(mk_expr(p, es.span.lo, hi,
                         ast::expr_vec(es.node, ast::m_imm)));
    }
    let mut b = none;
    if p.token == token::LBRACE {
        p.bump();
        let lo = p.span.lo;
        let mut depth = 1u;
        while (depth > 0u) {
            alt (p.token) {
              token::LBRACE {depth += 1u;}
              token::RBRACE {depth -= 1u;}
              token::EOF {p.fatal("unexpected EOF in macro body");}
              _ {}
            }
            p.bump();
        }
        let hi = p.last_span.lo;
        b = some({span: mk_sp(lo,hi)});
    }
    ret mk_mac_expr(p, lo, p.span.hi, ast::mac_invoc(pth, e, b));
}

fn parse_dot_or_call_expr(p: parser) -> pexpr {
    let b = parse_bottom_expr(p);
    parse_dot_or_call_expr_with(p, b)
}

fn permits_call(p: parser) -> bool {
    ret p.restriction != RESTRICT_NO_CALL_EXPRS;
}

fn parse_dot_or_call_expr_with(p: parser, e0: pexpr) -> pexpr {
    let mut e = e0;
    let lo = e.span.lo;
    let mut hi = e.span.hi;
    loop {
        // expr.f
        if eat(p, token::DOT) {
            alt p.token {
              token::IDENT(i, _) {
                hi = p.span.hi;
                p.bump();
                let tys = if eat(p, token::MOD_SEP) {
                    expect(p, token::LT);
                    parse_seq_to_gt(some(token::COMMA),
                                    {|p| parse_ty(p, false)}, p)
                } else { [] };
                e = mk_pexpr(p, lo, hi,
                             ast::expr_field(to_expr(e),
                                             p.get_str(i),
                                             tys));
              }
              _ { unexpected(p); }
            }
            cont;
        }
        if expr_is_complete(p, e) { break; }
        alt p.token {
          // expr(...)
          token::LPAREN if permits_call(p) {
            let es_opt =
                parse_seq(token::LPAREN, token::RPAREN,
                          seq_sep(token::COMMA), parse_expr_or_hole, p);
            hi = es_opt.span.hi;

            let nd =
                if vec::any(es_opt.node, {|e| option::is_none(e) }) {
                    ast::expr_bind(to_expr(e), es_opt.node)
                } else {
                    let es = vec::map(es_opt.node) {|e| option::get(e) };
                    ast::expr_call(to_expr(e), es, false)
                };
            e = mk_pexpr(p, lo, hi, nd);
          }

          // expr {|| ... }
          token::LBRACE if (token::is_bar(p.look_ahead(1u))
                            && permits_call(p)) {
            p.bump();
            let blk = parse_fn_block_expr(p);
            alt e.node {
              ast::expr_call(f, args, false) {
                e = pexpr(@{node: ast::expr_call(f, args + [blk], true)
                            with *to_expr(e)});
              }
              _ {
                e = mk_pexpr(p, lo, p.last_span.hi,
                            ast::expr_call(to_expr(e), [blk], true));
              }
            }
          }

          // expr[...]
          token::LBRACKET {
            p.bump();
            let ix = parse_expr(p);
            hi = ix.span.hi;
            expect(p, token::RBRACKET);
            p.get_id(); // see ast_util::op_expr_callee_id
            e = mk_pexpr(p, lo, hi, ast::expr_index(to_expr(e), ix));
          }

          _ { ret e; }
        }
    }
    ret e;
}

fn parse_prefix_expr(p: parser) -> pexpr {
    let lo = p.span.lo;
    let mut hi = p.span.hi;

    let mut ex;
    alt p.token {
      token::NOT {
        p.bump();
        let e = to_expr(parse_prefix_expr(p));
        hi = e.span.hi;
        p.get_id(); // see ast_util::op_expr_callee_id
        ex = ast::expr_unary(ast::not, e);
      }
      token::BINOP(b) {
        alt b {
          token::MINUS {
            p.bump();
            let e = to_expr(parse_prefix_expr(p));
            hi = e.span.hi;
            p.get_id(); // see ast_util::op_expr_callee_id
            ex = ast::expr_unary(ast::neg, e);
          }
          token::STAR {
            p.bump();
            let e = to_expr(parse_prefix_expr(p));
            hi = e.span.hi;
            ex = ast::expr_unary(ast::deref, e);
          }
          token::AND {
            p.bump();
            let m = parse_mutability(p);
            let e = to_expr(parse_prefix_expr(p));
            hi = e.span.hi;
            ex = ast::expr_addr_of(m, e);
          }
          _ { ret parse_dot_or_call_expr(p); }
        }
      }
      token::AT {
        p.bump();
        let m = parse_mutability(p);
        let e = to_expr(parse_prefix_expr(p));
        hi = e.span.hi;
        ex = ast::expr_unary(ast::box(m), e);
      }
      token::TILDE {
        p.bump();
        let m = parse_mutability(p);
        let e = to_expr(parse_prefix_expr(p));
        hi = e.span.hi;
        ex = ast::expr_unary(ast::uniq(m), e);
      }
      _ { ret parse_dot_or_call_expr(p); }
    }
    ret mk_pexpr(p, lo, hi, ex);
}


fn parse_binops(p: parser) -> @ast::expr {
    ret parse_more_binops(p, parse_prefix_expr(p), 0);
}

fn parse_more_binops(p: parser, plhs: pexpr, min_prec: int) ->
   @ast::expr {
    let lhs = to_expr(plhs);
    if expr_is_complete(p, plhs) { ret lhs; }
    let peeked = p.token;
    if peeked == token::BINOP(token::OR) &&
       p.restriction == RESTRICT_NO_BAR_OP { ret lhs; }
    for vec::each(*p.binop_precs) {|cur|
        if cur.prec > min_prec && cur.tok == peeked {
            p.bump();
            let expr = parse_prefix_expr(p);
            let rhs = parse_more_binops(p, expr, cur.prec);
            p.get_id(); // see ast_util::op_expr_callee_id
            let bin = mk_pexpr(p, lhs.span.lo, rhs.span.hi,
                              ast::expr_binary(cur.op, lhs, rhs));
            ret parse_more_binops(p, bin, min_prec);
        }
    }
    if as_prec > min_prec && eat_word(p, "as") {
        let rhs = parse_ty(p, true);
        let _as =
            mk_pexpr(p, lhs.span.lo, rhs.span.hi, ast::expr_cast(lhs, rhs));
        ret parse_more_binops(p, _as, min_prec);
    }
    ret lhs;
}

fn parse_assign_expr(p: parser) -> @ast::expr {
    let lo = p.span.lo;
    let lhs = parse_binops(p);
    alt p.token {
      token::EQ {
        p.bump();
        let rhs = parse_expr(p);
        ret mk_expr(p, lo, rhs.span.hi, ast::expr_assign(lhs, rhs));
      }
      token::BINOPEQ(op) {
        p.bump();
        let rhs = parse_expr(p);
        let mut aop;
        alt op {
          token::PLUS { aop = ast::add; }
          token::MINUS { aop = ast::subtract; }
          token::STAR { aop = ast::mul; }
          token::SLASH { aop = ast::div; }
          token::PERCENT { aop = ast::rem; }
          token::CARET { aop = ast::bitxor; }
          token::AND { aop = ast::bitand; }
          token::OR { aop = ast::bitor; }
          token::LSL { aop = ast::lsl; }
          token::LSR { aop = ast::lsr; }
          token::ASR { aop = ast::asr; }
        }
        p.get_id(); // see ast_util::op_expr_callee_id
        ret mk_expr(p, lo, rhs.span.hi, ast::expr_assign_op(aop, lhs, rhs));
      }
      token::LARROW {
        p.bump();
        let rhs = parse_expr(p);
        ret mk_expr(p, lo, rhs.span.hi, ast::expr_move(lhs, rhs));
      }
      token::DARROW {
        p.bump();
        let rhs = parse_expr(p);
        ret mk_expr(p, lo, rhs.span.hi, ast::expr_swap(lhs, rhs));
      }
      _ {/* fall through */ }
    }
    ret lhs;
}

fn parse_if_expr_1(p: parser) ->
   {cond: @ast::expr,
    then: ast::blk,
    els: option<@ast::expr>,
    lo: uint,
    hi: uint} {
    let lo = p.last_span.lo;
    let cond = parse_expr(p);
    let thn = parse_block(p);
    let mut els: option<@ast::expr> = none;
    let mut hi = thn.span.hi;
    if eat_word(p, "else") {
        let elexpr = parse_else_expr(p);
        els = some(elexpr);
        hi = elexpr.span.hi;
    }
    ret {cond: cond, then: thn, els: els, lo: lo, hi: hi};
}

fn parse_if_expr(p: parser) -> @ast::expr {
    if eat_word(p, "check") {
        let q = parse_if_expr_1(p);
        ret mk_expr(p, q.lo, q.hi, ast::expr_if_check(q.cond, q.then, q.els));
    } else {
        let q = parse_if_expr_1(p);
        ret mk_expr(p, q.lo, q.hi, ast::expr_if(q.cond, q.then, q.els));
    }
}

// Parses:
//
//   CC := [copy ID*; move ID*]
//
// where any part is optional and trailing ; is permitted.
fn parse_capture_clause(p: parser) -> @ast::capture_clause {
    fn expect_opt_trailing_semi(p: parser) {
        if !eat(p, token::SEMI) {
            if p.token != token::RBRACKET {
                p.fatal("expecting ; or ]");
            }
        }
    }

    fn eat_ident_list(p: parser) -> [@ast::capture_item] {
        let mut res = [];
        loop {
            alt p.token {
              token::IDENT(_, _) {
                let id = p.get_id();
                let sp = ast_util::mk_sp(p.span.lo, p.span.hi);
                let ident = parse_ident(p);
                res += [@{id:id, name:ident, span:sp}];
                if !eat(p, token::COMMA) {
                    ret res;
                }
              }

              _ { ret res; }
            }
        };
    }

    let mut copies = [];
    let mut moves = [];

    if eat(p, token::LBRACKET) {
        while !eat(p, token::RBRACKET) {
            if eat_word(p, "copy") {
                copies += eat_ident_list(p);
                expect_opt_trailing_semi(p);
            } else if eat_word(p, "move") {
                moves += eat_ident_list(p);
                expect_opt_trailing_semi(p);
            } else {
                let s: str = "expecting send, copy, or move clause";
                p.fatal(s);
            }
        }
    }

    ret @{copies: copies, moves: moves};
}

fn parse_fn_expr(p: parser, proto: ast::proto) -> @ast::expr {
    let lo = p.last_span.lo;
    let capture_clause = parse_capture_clause(p);
    let decl = parse_fn_decl(p, ast::impure_fn);
    let body = parse_block(p);
    ret mk_expr(p, lo, body.span.hi,
                ast::expr_fn(proto, decl, body, capture_clause));
}

fn parse_fn_block_expr(p: parser) -> @ast::expr {
    let lo = p.last_span.lo;
    let decl = parse_fn_block_decl(p);
    let body = parse_block_tail(p, lo, ast::default_blk);
    ret mk_expr(p, lo, body.span.hi, ast::expr_fn_block(decl, body));
}

fn parse_else_expr(p: parser) -> @ast::expr {
    if eat_word(p, "if") {
        ret parse_if_expr(p);
    } else {
        let blk = parse_block(p);
        ret mk_expr(p, blk.span.lo, blk.span.hi, ast::expr_block(blk));
    }
}

fn parse_for_expr(p: parser) -> @ast::expr {
    let lo = p.last_span;
    let call = parse_expr_res(p, RESTRICT_STMT_EXPR);
    alt call.node {
      ast::expr_call(f, args, true) {
        let b_arg = vec::last(args);
        let last = mk_expr(p, b_arg.span.lo, b_arg.span.hi,
                           ast::expr_loop_body(b_arg));
        @{node: ast::expr_call(f, vec::init(args) + [last], true)
          with *call}
      }
      _ {
        p.span_fatal(lo, "`for` must be followed by a block call");
      }
    }
}

fn parse_while_expr(p: parser) -> @ast::expr {
    let lo = p.last_span.lo;
    let cond = parse_expr(p);
    let body = parse_block_no_value(p);
    let mut hi = body.span.hi;
    ret mk_expr(p, lo, hi, ast::expr_while(cond, body));
}

fn parse_do_while_expr(p: parser) -> @ast::expr {
    let lo = p.last_span.lo;
    let body = parse_block_no_value(p);
    expect_word(p, "while");
    let cond = parse_expr(p);
    let mut hi = cond.span.hi;
    ret mk_expr(p, lo, hi, ast::expr_do_while(body, cond));
}

fn parse_loop_expr(p: parser) -> @ast::expr {
    let lo = p.last_span.lo;
    let body = parse_block_no_value(p);
    let mut hi = body.span.hi;
    ret mk_expr(p, lo, hi, ast::expr_loop(body));
}

fn parse_alt_expr(p: parser) -> @ast::expr {
    let lo = p.last_span.lo;
    let mode = if eat_word(p, "check") { ast::alt_check }
               else { ast::alt_exhaustive };
    let discriminant = parse_expr(p);
    expect(p, token::LBRACE);
    let mut arms: [ast::arm] = [];
    while p.token != token::RBRACE {
        let pats = parse_pats(p);
        let mut guard = none;
        if eat_word(p, "if") { guard = some(parse_expr(p)); }
        let blk = parse_block(p);
        arms += [{pats: pats, guard: guard, body: blk}];
    }
    let mut hi = p.span.hi;
    p.bump();
    ret mk_expr(p, lo, hi, ast::expr_alt(discriminant, arms, mode));
}

fn parse_expr(p: parser) -> @ast::expr {
    ret parse_expr_res(p, UNRESTRICTED);
}

fn parse_expr_or_hole(p: parser) -> option<@ast::expr> {
    alt p.token {
      token::UNDERSCORE { p.bump(); ret none; }
      _ { ret some(parse_expr(p)); }
    }
}

fn parse_expr_res(p: parser, r: restriction) -> @ast::expr {
    let old = p.restriction;
    p.restriction = r;
    let e = parse_assign_expr(p);
    p.restriction = old;
    ret e;
}

fn parse_initializer(p: parser) -> option<ast::initializer> {
    alt p.token {
      token::EQ {
        p.bump();
        ret some({op: ast::init_assign, expr: parse_expr(p)});
      }
      token::LARROW {
        p.bump();
        ret some({op: ast::init_move, expr: parse_expr(p)});
      }
      // Now that the the channel is the first argument to receive,
      // combining it with an initializer doesn't really make sense.
      // case (token::RECV) {
      //     p.bump();
      //     ret some(rec(op = ast::init_recv,
      //                  expr = parse_expr(p)));
      // }
      _ {
        ret none;
      }
    }
}

fn parse_pats(p: parser) -> [@ast::pat] {
    let mut pats = [];
    loop {
        pats += [parse_pat(p)];
        if p.token == token::BINOP(token::OR) { p.bump(); } else { ret pats; }
    };
}

fn parse_pat(p: parser) -> @ast::pat {
    let lo = p.span.lo;
    let mut hi = p.span.hi;
    let mut pat;
    alt p.token {
      token::UNDERSCORE { p.bump(); pat = ast::pat_wild; }
      token::AT {
        p.bump();
        let sub = parse_pat(p);
        pat = ast::pat_box(sub);
        hi = sub.span.hi;
      }
      token::TILDE {
        p.bump();
        let sub = parse_pat(p);
        pat = ast::pat_uniq(sub);
        hi = sub.span.hi;
      }
      token::LBRACE {
        p.bump();
        let mut fields = [];
        let mut etc = false;
        let mut first = true;
        while p.token != token::RBRACE {
            if first { first = false; } else { expect(p, token::COMMA); }

            if p.token == token::UNDERSCORE {
                p.bump();
                if p.token != token::RBRACE {
                    p.fatal("expecting }, found " +
                                token_to_str(p.reader, p.token));
                }
                etc = true;
                break;
            }

            let lo1 = p.last_span.lo;
            let fieldname = parse_ident(p);
            let hi1 = p.last_span.lo;
            let fieldpath = ast_util::ident_to_path(ast_util::mk_sp(lo1, hi1),
                                          fieldname);
            let mut subpat;
            if p.token == token::COLON {
                p.bump();
                subpat = parse_pat(p);
            } else {
                if p.bad_expr_words.contains_key(fieldname) {
                    p.fatal("found " + fieldname + " in binding position");
                }
                subpat = @{id: p.get_id(),
                           node: ast::pat_ident(fieldpath, none),
                           span: ast_util::mk_sp(lo, hi)};
            }
            fields += [{ident: fieldname, pat: subpat}];
        }
        hi = p.span.hi;
        p.bump();
        pat = ast::pat_rec(fields, etc);
      }
      token::LPAREN {
        p.bump();
        if p.token == token::RPAREN {
            hi = p.span.hi;
            p.bump();
            let lit = @{node: ast::lit_nil, span: ast_util::mk_sp(lo, hi)};
            let expr = mk_expr(p, lo, hi, ast::expr_lit(lit));
            pat = ast::pat_lit(expr);
        } else {
            let mut fields = [parse_pat(p)];
            while p.token == token::COMMA {
                p.bump();
                fields += [parse_pat(p)];
            }
            if vec::len(fields) == 1u { expect(p, token::COMMA); }
            hi = p.span.hi;
            expect(p, token::RPAREN);
            pat = ast::pat_tup(fields);
        }
      }
      tok {
        if !is_ident(tok) || is_word(p, "true") || is_word(p, "false") {
            let val = parse_expr_res(p, RESTRICT_NO_BAR_OP);
            if eat_word(p, "to") {
                let end = parse_expr_res(p, RESTRICT_NO_BAR_OP);
                hi = end.span.hi;
                pat = ast::pat_range(val, end);
            } else {
                hi = val.span.hi;
                pat = ast::pat_lit(val);
            }
        } else if is_plain_ident(p.token) &&
            alt p.look_ahead(1u) {
              token::LPAREN | token::LBRACKET | token::LT { false }
              _ { true }
            } {
            let name = parse_value_path(p);
            let sub = if eat(p, token::AT) { some(parse_pat(p)) }
                      else { none };
            pat = ast::pat_ident(name, sub);
        } else {
            let enum_path = parse_path_and_ty_param_substs(p, true);
            hi = enum_path.span.hi;
            let mut args: [@ast::pat] = [];
            let mut star_pat = false;
            alt p.token {
              token::LPAREN {
                alt p.look_ahead(1u) {
                  token::BINOP(token::STAR) {
                    // This is a "top constructor only" pat
                    p.bump(); p.bump();
                    star_pat = true;
                    expect(p, token::RPAREN);
                  }
                  _ {
                   let a =
                       parse_seq(token::LPAREN, token::RPAREN,
                                seq_sep(token::COMMA), parse_pat, p);
                    args = a.node;
                    hi = a.span.hi;
                  }
                }
              }
              _ { }
            }
            // at this point, we're not sure whether it's a enum or a bind
            if star_pat {
                 pat = ast::pat_enum(enum_path, none);
            }
            else if vec::is_empty(args) &&
               vec::len(enum_path.node.idents) == 1u {
                pat = ast::pat_ident(enum_path, none);
            }
            else {
                pat = ast::pat_enum(enum_path, some(args));
            }
        }
      }
    }
    ret @{id: p.get_id(), node: pat, span: ast_util::mk_sp(lo, hi)};
}

fn parse_local(p: parser, is_mutbl: bool,
               allow_init: bool) -> @ast::local {
    let lo = p.span.lo;
    let pat = parse_pat(p);
    let mut ty = @{id: p.get_id(),
                   node: ast::ty_infer,
                   span: ast_util::mk_sp(lo, lo)};
    if eat(p, token::COLON) { ty = parse_ty(p, false); }
    let init = if allow_init { parse_initializer(p) } else { none };
    ret @spanned(lo, p.last_span.hi,
                 {is_mutbl: is_mutbl, ty: ty, pat: pat,
                  init: init, id: p.get_id()});
}

fn parse_let(p: parser) -> @ast::decl {
    let is_mutbl = eat_word(p, "mut");
    let lo = p.span.lo;
    let mut locals = [parse_local(p, is_mutbl, true)];
    while eat(p, token::COMMA) {
        locals += [parse_local(p, is_mutbl, true)];
    }
    ret @spanned(lo, p.last_span.hi, ast::decl_local(locals));
}

/* assumes "let" token has already been consumed */
fn parse_instance_var(p:parser, pr: ast::privacy) -> @ast::class_member {
    let mut is_mutbl = ast::class_immutable;
    let lo = p.span.lo;
    if eat_word(p, "mut") || eat_word(p, "mutable") {
        is_mutbl = ast::class_mutable;
    }
    if !is_plain_ident(p.token) {
        p.fatal("expecting ident");
    }
    let name = parse_ident(p);
    expect(p, token::COLON);
    let ty = parse_ty(p, false);
    ret @{node: ast::instance_var(name, ty, is_mutbl, p.get_id(), pr),
          span: ast_util::mk_sp(lo, p.last_span.hi)};
}

fn parse_stmt(p: parser, first_item_attrs: [ast::attribute]) -> @ast::stmt {
    fn check_expected_item(p: parser, current_attrs: [ast::attribute]) {
        // If we have attributes then we should have an item
        if vec::is_not_empty(current_attrs) {
            p.fatal("expected item");
        }
    }

    let lo = p.span.lo;
    if is_word(p, "let") {
        check_expected_item(p, first_item_attrs);
        expect_word(p, "let");
        let decl = parse_let(p);
        ret @spanned(lo, decl.span.hi, ast::stmt_decl(decl, p.get_id()));
    } else {
        let mut item_attrs;
        alt parse_outer_attrs_or_ext(p, first_item_attrs) {
          none { item_attrs = []; }
          some(left(attrs)) { item_attrs = attrs; }
          some(right(ext)) {
            ret @spanned(lo, ext.span.hi, ast::stmt_expr(ext, p.get_id()));
          }
        }

        let item_attrs = first_item_attrs + item_attrs;

        alt parse_item(p, item_attrs) {
          some(i) {
            let mut hi = i.span.hi;
            let decl = @spanned(lo, hi, ast::decl_item(i));
            ret @spanned(lo, hi, ast::stmt_decl(decl, p.get_id()));
          }
          none() { /* fallthrough */ }
        }

        check_expected_item(p, item_attrs);

        // Remainder are line-expr stmts.
        let e = parse_expr_res(p, RESTRICT_STMT_EXPR);
        ret @spanned(lo, e.span.hi, ast::stmt_expr(e, p.get_id()));
    }
}

fn expr_is_complete(p: parser, e: pexpr) -> bool {
    log(debug, ("expr_is_complete", p.restriction,
                print::pprust::expr_to_str(*e),
                classify::expr_requires_semi_to_be_stmt(*e)));
    ret p.restriction == RESTRICT_STMT_EXPR &&
        !classify::expr_requires_semi_to_be_stmt(*e);
}

fn parse_block(p: parser) -> ast::blk {
    let (attrs, blk) = parse_inner_attrs_and_block(p, false);
    assert vec::is_empty(attrs);
    ret blk;
}

fn parse_inner_attrs_and_block(
    p: parser, parse_attrs: bool) -> ([ast::attribute], ast::blk) {

    fn maybe_parse_inner_attrs_and_next(
        p: parser, parse_attrs: bool) ->
        {inner: [ast::attribute], next: [ast::attribute]} {
        if parse_attrs {
            parse_inner_attrs_and_next(p)
        } else {
            {inner: [], next: []}
        }
    }

    let lo = p.span.lo;
    if eat_word(p, "unchecked") {
        expect(p, token::LBRACE);
        let {inner, next} = maybe_parse_inner_attrs_and_next(p, parse_attrs);
        ret (inner, parse_block_tail_(p, lo, ast::unchecked_blk, next));
    } else if eat_word(p, "unsafe") {
        expect(p, token::LBRACE);
        let {inner, next} = maybe_parse_inner_attrs_and_next(p, parse_attrs);
        ret (inner, parse_block_tail_(p, lo, ast::unsafe_blk, next));
    } else {
        expect(p, token::LBRACE);
        let {inner, next} = maybe_parse_inner_attrs_and_next(p, parse_attrs);
        ret (inner, parse_block_tail_(p, lo, ast::default_blk, next));
    }
}

fn parse_block_no_value(p: parser) -> ast::blk {
    // We parse blocks that cannot have a value the same as any other block;
    // the type checker will make sure that the tail expression (if any) has
    // unit type.
    ret parse_block(p);
}

// Precondition: already parsed the '{' or '#{'
// I guess that also means "already parsed the 'impure'" if
// necessary, and this should take a qualifier.
// some blocks start with "#{"...
fn parse_block_tail(p: parser, lo: uint, s: ast::blk_check_mode) -> ast::blk {
    parse_block_tail_(p, lo, s, [])
}

fn parse_block_tail_(p: parser, lo: uint, s: ast::blk_check_mode,
                     first_item_attrs: [ast::attribute]) -> ast::blk {
    let mut stmts = [];
    let mut expr = none;
    let view_items = maybe_parse_view_import_only(p, first_item_attrs);
    let mut initial_attrs = first_item_attrs;

    if p.token == token::RBRACE && !vec::is_empty(initial_attrs) {
        p.fatal("expected item");
    }

    while p.token != token::RBRACE {
        alt p.token {
          token::SEMI {
            p.bump(); // empty
          }
          _ {
            let stmt = parse_stmt(p, initial_attrs);
            initial_attrs = [];
            alt stmt.node {
              ast::stmt_expr(e, stmt_id) { // Expression without semicolon:
                alt p.token {
                  token::SEMI {
                    p.bump();
                    stmts += [@{node: ast::stmt_semi(e, stmt_id) with *stmt}];
                  }
                  token::RBRACE {
                    expr = some(e);
                  }
                  t {
                    if classify::stmt_ends_with_semi(*stmt) {
                        p.fatal("expected ';' or '}' after expression but \
                                 found '" + token_to_str(p.reader, t) +
                                "'");
                    }
                    stmts += [stmt];
                  }
                }
              }

              _ { // All other kinds of statements:
                stmts += [stmt];

                if classify::stmt_ends_with_semi(*stmt) {
                    expect(p, token::SEMI);
                }
              }
            }
          }
        }
    }
    let mut hi = p.span.hi;
    p.bump();
    let bloc = {view_items: view_items, stmts: stmts, expr: expr,
                id: p.get_id(), rules: s};
    ret spanned(lo, hi, bloc);
}

fn parse_ty_param(p: parser) -> ast::ty_param {
    let mut bounds = [];
    let ident = parse_ident(p);
    if eat(p, token::COLON) {
        while p.token != token::COMMA && p.token != token::GT {
            if eat_word(p, "send") { bounds += [ast::bound_send]; }
            else if eat_word(p, "copy") { bounds += [ast::bound_copy]; }
            else { bounds += [ast::bound_iface(parse_ty(p, false))]; }
        }
    }
    ret {ident: ident, id: p.get_id(), bounds: @bounds};
}

fn parse_ty_params(p: parser) -> [ast::ty_param] {
    if eat(p, token::LT) {
        parse_seq_to_gt(some(token::COMMA), parse_ty_param, p)
    } else { [] }
}

fn parse_fn_decl(p: parser, purity: ast::purity)
    -> ast::fn_decl {
    let inputs: ast::spanned<[ast::arg]> =
        parse_seq(token::LPAREN, token::RPAREN, seq_sep(token::COMMA),
                  parse_arg, p);
    // Use the args list to translate each bound variable
    // mentioned in a constraint to an arg index.
    // Seems weird to do this in the parser, but I'm not sure how else to.
    let mut constrs = [];
    if p.token == token::COLON {
        p.bump();
        constrs = parse_constrs({|x| parse_ty_constr(inputs.node, x) }, p);
    }
    let (ret_style, ret_ty) = parse_ret_ty(p);
    ret {inputs: inputs.node,
         output: ret_ty,
         purity: purity,
         cf: ret_style,
         constraints: constrs};
}

fn parse_fn_block_decl(p: parser) -> ast::fn_decl {
    let inputs = if eat(p, token::OROR) {
                     []
                 } else {
                     parse_seq(token::BINOP(token::OR),
                               token::BINOP(token::OR),
                               seq_sep(token::COMMA),
                               parse_fn_block_arg, p).node
                 };
    let output = if eat(p, token::RARROW) {
                     parse_ty(p, false)
                 } else {
                     @{id: p.get_id(), node: ast::ty_infer, span: p.span}
                 };
    ret {inputs: inputs,
         output: output,
         purity: ast::impure_fn,
         cf: ast::return_val,
         constraints: []};
}

fn parse_fn_header(p: parser) -> {ident: ast::ident, tps: [ast::ty_param]} {
    let id = parse_value_ident(p);
    let ty_params = parse_ty_params(p);
    ret {ident: id, tps: ty_params};
}

fn mk_item(p: parser, lo: uint, hi: uint, ident: ast::ident, node: ast::item_,
           attrs: [ast::attribute]) -> @ast::item {
    ret @{ident: ident,
          attrs: attrs,
          id: p.get_id(),
          node: node,
          span: ast_util::mk_sp(lo, hi)};
}

fn parse_item_fn(p: parser, purity: ast::purity,
                 attrs: [ast::attribute]) -> @ast::item {
    let lo = p.last_span.lo;
    let t = parse_fn_header(p);
    let decl = parse_fn_decl(p, purity);
    let (inner_attrs, body) = parse_inner_attrs_and_block(p, true);
    let attrs = attrs + inner_attrs;
    ret mk_item(p, lo, body.span.hi, t.ident,
                ast::item_fn(decl, t.tps, body), attrs);
}

fn parse_method_name(p: parser) -> ast::ident {
    alt p.token {
      token::BINOP(op) { p.bump(); token::binop_to_str(op) }
      token::NOT { p.bump(); "!" }
      token::LBRACKET { p.bump(); expect(p, token::RBRACKET); "[]" }
      _ {
          let id = parse_value_ident(p);
          if id == "unary" && eat(p, token::BINOP(token::MINUS)) { "unary-" }
          else { id }
      }
    }
}

fn parse_method(p: parser, pr: ast::privacy) -> @ast::method {
    let attrs = parse_outer_attributes(p);
    let lo = p.span.lo, pur = parse_fn_purity(p);
    let ident = parse_method_name(p);
    let tps = parse_ty_params(p);
    let decl = parse_fn_decl(p, pur);
    let (inner_attrs, body) = parse_inner_attrs_and_block(p, true);
    let attrs = attrs + inner_attrs;
    @{ident: ident, attrs: attrs, tps: tps, decl: decl, body: body,
      id: p.get_id(), span: ast_util::mk_sp(lo, body.span.hi),
      self_id: p.get_id(), privacy: pr}
}

fn parse_item_iface(p: parser, attrs: [ast::attribute]) -> @ast::item {
    let lo = p.last_span.lo, ident = parse_ident(p),
        tps = parse_ty_params(p), meths = parse_ty_methods(p);
    ret mk_item(p, lo, p.last_span.hi, ident,
                ast::item_iface(tps, meths), attrs);
}

// Parses three variants (with the initial params always optional):
//    impl <T: copy> of to_str for [T] { ... }
//    impl name<T> of to_str for [T] { ... }
//    impl name<T> for [T] { ... }
fn parse_item_impl(p: parser, attrs: [ast::attribute]) -> @ast::item {
    let lo = p.last_span.lo;
    fn wrap_path(p: parser, pt: @ast::path) -> @ast::ty {
        @{id: p.get_id(), node: ast::ty_path(pt, p.get_id()), span: pt.span}
    }
    let mut (ident, tps) = if !is_word(p, "of") {
        if p.token == token::LT { (none, parse_ty_params(p)) }
        else { (some(parse_ident(p)), parse_ty_params(p)) }
    } else { (none, []) };
    let ifce = if eat_word(p, "of") {
        let path = parse_path_and_ty_param_substs(p, false);
        if option::is_none(ident) {
            ident = some(path.node.idents[vec::len(path.node.idents) - 1u]);
        }
        some(wrap_path(p, path))
    } else { none };
    let ident = alt ident {
        some(name) { name }
        none { expect_word(p, "of"); fail; }
    };
    expect_word(p, "for");
    let ty = parse_ty(p, false);
    let mut meths = [];
    expect(p, token::LBRACE);
    while !eat(p, token::RBRACE) { meths += [parse_method(p, ast::pub)]; }
    ret mk_item(p, lo, p.last_span.hi, ident,
                ast::item_impl(tps, ifce, ty, meths), attrs);
}

fn parse_item_res(p: parser, attrs: [ast::attribute]) -> @ast::item {
    let lo = p.last_span.lo;
    let ident = parse_value_ident(p);
    let rp = parse_region_param(p);
    let ty_params = parse_ty_params(p);
    expect(p, token::LPAREN);
    let arg_ident = parse_value_ident(p);
    expect(p, token::COLON);
    let t = parse_ty(p, false);
    expect(p, token::RPAREN);
    let dtor = parse_block_no_value(p);
    let decl =
        {inputs:
             [{mode: ast::expl(ast::by_ref), ty: t,
               ident: arg_ident, id: p.get_id()}],
         output: @{id: p.get_id(),
                   node: ast::ty_nil,
                   span: ast_util::mk_sp(lo, lo)},
         purity: ast::impure_fn,
         cf: ast::return_val,
         constraints: []};
    ret mk_item(p, lo, dtor.span.hi, ident,
                ast::item_res(decl, ty_params, dtor,
                              p.get_id(), p.get_id(), rp),
                attrs);
}

// Instantiates ident <i> with references to <typarams> as arguments
fn ident_to_path_tys(p: parser, i: ast::ident,
                     typarams: [ast::ty_param]) -> @ast::path {
    let s = p.last_span;
    let p_: ast::path_ = {global: false, idents: [i],
          types: vec::map(typarams,
            {|tp| @{id: p.get_id(),
                   node: ast::ty_path(ident_to_path(s, tp.ident),
                                      p.get_id()),
                        span: s}})};
    @spanned(s.lo, s.hi, p_)
}

fn parse_iface_ref_list(p:parser) -> [ast::iface_ref] {
    parse_seq_to_before_end(token::LBRACE, seq_sep(token::COMMA),
                   {|p| {path: parse_path(p), id: p.get_id()}}, p)
}

fn parse_item_class(p: parser, attrs: [ast::attribute]) -> @ast::item {
    let lo = p.last_span.lo;
    let class_name = parse_value_ident(p);
    let rp = parse_region_param(p);
    let ty_params = parse_ty_params(p);
    let class_path = ident_to_path_tys(p, class_name, ty_params);
    let ifaces : [ast::iface_ref] = if eat_word(p, "implements")
                                       { parse_iface_ref_list(p) }
                                    else { [] };
    expect(p, token::LBRACE);
    let mut ms: [@ast::class_member] = [];
    let ctor_id = p.get_id();
    let mut the_ctor : option<(ast::fn_decl, ast::blk, codemap::span)> = none;
    while p.token != token::RBRACE {
        alt parse_class_item(p, class_path) {
            ctor_decl(a_fn_decl, blk, s) {
               the_ctor = some((a_fn_decl, blk, s));
            }
            members(mms) { ms += mms; }
       }
    }
    p.bump();
    alt the_ctor {
      some((ct_d, ct_b, ct_s)) {
          ret mk_item(p, lo, p.last_span.hi, class_name,
                      ast::item_class(ty_params, ifaces, ms,
                                      {node: {id: ctor_id,
                                              self_id: p.get_id(),
                                              dec: ct_d,
                                              body: ct_b},
                                       span: ct_s}, rp), attrs); }
       /*
         Is it strange for the parser to check this?
       */
       none {
         p.fatal("class with no ctor");
       }
    }
}

fn parse_single_class_item(p: parser, privcy: ast::privacy)
    -> @ast::class_member {
   if eat_word(p, "let") {
      let a_var = parse_instance_var(p, privcy);
      expect(p, token::SEMI);
      ret a_var;
   }
   else {
       let m = parse_method(p, privcy);
       ret @{node: ast::class_method(m), span: m.span};
   }
}

// lets us identify the constructor declaration at
// parse time
enum class_contents { ctor_decl(ast::fn_decl, ast::blk, codemap::span),
                      members([@ast::class_member]) }

fn parse_class_item(p:parser, class_name_with_tps:@ast::path)
    -> class_contents {
    if eat_word(p, "new") {
        let lo = p.last_span.lo;
        // Can ctors have attrs?
            // result type is always the type of the class
        let decl_ = parse_fn_decl(p, ast::impure_fn);
        let decl = {output: @{id: p.get_id(),
                      node: ast::ty_path(class_name_with_tps, p.get_id()),
                      span: decl_.output.span}
                    with decl_};
        let body = parse_block(p);
        ret ctor_decl(decl, body, ast_util::mk_sp(lo, p.last_span.hi));
    }
    else if eat_word(p, "priv") {
            expect(p, token::LBRACE);
            let mut results = [];
            while p.token != token::RBRACE {
                    results += [parse_single_class_item(p, ast::priv)];
            }
            p.bump();
            ret members(results);
    }
    else {
        // Probably need to parse attrs
        ret members([parse_single_class_item(p, ast::pub)]);
    }
}

fn parse_mod_items(p: parser, term: token::token,
                   first_item_attrs: [ast::attribute]) -> ast::_mod {
    // Shouldn't be any view items since we've already parsed an item attr
    let view_items = maybe_parse_view(p, first_item_attrs);
    let mut items: [@ast::item] = [];
    let mut initial_attrs = first_item_attrs;
    while p.token != term {
        let attrs = initial_attrs + parse_outer_attributes(p);
        #debug["parse_mod_items: parse_item(attrs=%?)", attrs];
        alt parse_item(p, attrs) {
          some(i) { items += [i]; }
          _ {
            p.fatal("expected item but found '" +
                    token_to_str(p.reader, p.token) + "'");
          }
        }
        #debug["parse_mod_items: attrs=%?", attrs];
        initial_attrs = [];
    }

    if vec::is_not_empty(initial_attrs) {
        // We parsed attributes for the first item but didn't find the item
        p.fatal("expected item");
    }

    ret {view_items: view_items, items: items};
}

fn parse_item_const(p: parser, attrs: [ast::attribute]) -> @ast::item {
    let lo = p.last_span.lo;
    let id = parse_value_ident(p);
    expect(p, token::COLON);
    let ty = parse_ty(p, false);
    expect(p, token::EQ);
    let e = parse_expr(p);
    let mut hi = p.span.hi;
    expect(p, token::SEMI);
    ret mk_item(p, lo, hi, id, ast::item_const(ty, e), attrs);
}

fn parse_item_mod(p: parser, attrs: [ast::attribute]) -> @ast::item {
    let lo = p.last_span.lo;
    let id = parse_ident(p);
    expect(p, token::LBRACE);
    let inner_attrs = parse_inner_attrs_and_next(p);
    let first_item_outer_attrs = inner_attrs.next;
    let m = parse_mod_items(p, token::RBRACE, first_item_outer_attrs);
    let mut hi = p.span.hi;
    expect(p, token::RBRACE);
    ret mk_item(p, lo, hi, id, ast::item_mod(m), attrs + inner_attrs.inner);
}

fn parse_item_native_fn(p: parser, attrs: [ast::attribute],
                        purity: ast::purity) -> @ast::native_item {
    let lo = p.last_span.lo;
    let t = parse_fn_header(p);
    let decl = parse_fn_decl(p, purity);
    let mut hi = p.span.hi;
    expect(p, token::SEMI);
    ret @{ident: t.ident,
          attrs: attrs,
          node: ast::native_item_fn(decl, t.tps),
          id: p.get_id(),
          span: ast_util::mk_sp(lo, hi)};
}

fn parse_fn_purity(p: parser) -> ast::purity {
    if eat_word(p, "fn") { ast::impure_fn }
    else if eat_word(p, "pure") { expect_word(p, "fn"); ast::pure_fn }
    else if eat_word(p, "unsafe") { expect_word(p, "fn"); ast::unsafe_fn }
    else { unexpected(p); }
}

fn parse_native_item(p: parser, attrs: [ast::attribute]) ->
   @ast::native_item {
    parse_item_native_fn(p, attrs, parse_fn_purity(p))
}

fn parse_native_mod_items(p: parser, first_item_attrs: [ast::attribute]) ->
   ast::native_mod {
    // Shouldn't be any view items since we've already parsed an item attr
    let view_items =
        if vec::len(first_item_attrs) == 0u {
            parse_native_view(p)
        } else { [] };
    let mut items: [@ast::native_item] = [];
    let mut initial_attrs = first_item_attrs;
    while p.token != token::RBRACE {
        let attrs = initial_attrs + parse_outer_attributes(p);
        initial_attrs = [];
        items += [parse_native_item(p, attrs)];
    }
    ret {view_items: view_items,
         items: items};
}

fn parse_item_native_mod(p: parser, attrs: [ast::attribute]) -> @ast::item {
    let lo = p.last_span.lo;
    expect_word(p, "mod");
    let id = parse_ident(p);
    expect(p, token::LBRACE);
    let more_attrs = parse_inner_attrs_and_next(p);
    let inner_attrs = more_attrs.inner;
    let first_item_outer_attrs = more_attrs.next;
    let m = parse_native_mod_items(p, first_item_outer_attrs);
    let mut hi = p.span.hi;
    expect(p, token::RBRACE);
    ret mk_item(p, lo, hi, id, ast::item_native_mod(m), attrs + inner_attrs);
}

fn parse_type_decl(p: parser) -> {lo: uint, ident: ast::ident} {
    let lo = p.last_span.lo;
    let id = parse_ident(p);
    ret {lo: lo, ident: id};
}

fn parse_item_type(p: parser, attrs: [ast::attribute]) -> @ast::item {
    let t = parse_type_decl(p);
    let rp = parse_region_param(p);
    let tps = parse_ty_params(p);
    expect(p, token::EQ);
    let ty = parse_ty(p, false);
    let mut hi = p.span.hi;
    expect(p, token::SEMI);
    ret mk_item(p, t.lo, hi, t.ident, ast::item_ty(ty, tps, rp), attrs);
}

fn parse_region_param(p: parser) -> ast::region_param {
    if eat(p, token::BINOP(token::SLASH)) {
        expect(p, token::BINOP(token::AND));
        ast::rp_self
    } else {
        ast::rp_none
    }
}

fn parse_item_enum(p: parser, attrs: [ast::attribute]) -> @ast::item {
    let lo = p.last_span.lo;
    let id = parse_ident(p);
    let rp = parse_region_param(p);
    let ty_params = parse_ty_params(p);
    let mut variants: [ast::variant] = [];
    // Newtype syntax
    if p.token == token::EQ {
        if p.bad_expr_words.contains_key(id) {
            p.fatal("found " + id + " in enum constructor position");
        }
        p.bump();
        let ty = parse_ty(p, false);
        expect(p, token::SEMI);
        let variant =
            spanned(ty.span.lo, ty.span.hi,
                    {name: id,
                     attrs: [],
                     args: [{ty: ty, id: p.get_id()}],
                     id: p.get_id(),
                     disr_expr: none});
        ret mk_item(p, lo, ty.span.hi, id,
                    ast::item_enum([variant], ty_params, rp), attrs);
    }
    expect(p, token::LBRACE);

    let mut all_nullary = true, have_disr = false;

    while p.token != token::RBRACE {
        let variant_attrs = parse_outer_attributes(p);
        let vlo = p.span.lo;
        let ident = parse_value_ident(p);
        let mut args = [], disr_expr = none;
        if p.token == token::LPAREN {
            all_nullary = false;
            let arg_tys = parse_seq(token::LPAREN, token::RPAREN,
                                    seq_sep(token::COMMA),
                                    {|p| parse_ty(p, false)}, p);
            for arg_tys.node.each {|ty|
                args += [{ty: ty, id: p.get_id()}];
            }
        } else if eat(p, token::EQ) {
            have_disr = true;
            disr_expr = some(parse_expr(p));
        }

        let vr = {name: ident, attrs: variant_attrs,
                  args: args, id: p.get_id(),
                  disr_expr: disr_expr};
        variants += [spanned(vlo, p.last_span.hi, vr)];

        if !eat(p, token::COMMA) { break; }
    }
    expect(p, token::RBRACE);
    if (have_disr && !all_nullary) {
        p.fatal("discriminator values can only be used with a c-like enum");
    }
    ret mk_item(p, lo, p.last_span.hi, id,
                ast::item_enum(variants, ty_params, rp), attrs);
}

fn parse_fn_ty_proto(p: parser) -> ast::proto {
    alt p.token {
      token::AT {
        p.bump();
        ast::proto_box
      }
      token::TILDE {
        p.bump();
        ast::proto_uniq
      }
      token::BINOP(token::AND) {
        p.bump();
        ast::proto_block
      }
      _ {
        ast::proto_any
      }
    }
}

fn fn_expr_lookahead(tok: token::token) -> bool {
    alt tok {
      token::LPAREN | token::AT | token::TILDE | token::BINOP(_) {
        true
      }
      _ {
        false
      }
    }
}

fn parse_item(p: parser, attrs: [ast::attribute]) -> option<@ast::item> {
    if eat_word(p, "const") {
        ret some(parse_item_const(p, attrs));
    } else if is_word(p, "fn") && !fn_expr_lookahead(p.look_ahead(1u)) {
        p.bump();
        ret some(parse_item_fn(p, ast::impure_fn, attrs));
    } else if eat_word(p, "pure") {
        expect_word(p, "fn");
        ret some(parse_item_fn(p, ast::pure_fn, attrs));
    } else if is_word(p, "unsafe") && p.look_ahead(1u) != token::LBRACE {
        p.bump();
        expect_word(p, "fn");
        ret some(parse_item_fn(p, ast::unsafe_fn, attrs));
    } else if eat_word(p, "crust") {
        expect_word(p, "fn");
        ret some(parse_item_fn(p, ast::crust_fn, attrs));
    } else if eat_word(p, "mod") {
        ret some(parse_item_mod(p, attrs));
    } else if eat_word(p, "native") {
        ret some(parse_item_native_mod(p, attrs));
    } if eat_word(p, "type") {
        ret some(parse_item_type(p, attrs));
    } else if eat_word(p, "enum") {
        ret some(parse_item_enum(p, attrs));
    } else if eat_word(p, "iface") {
        ret some(parse_item_iface(p, attrs));
    } else if eat_word(p, "impl") {
        ret some(parse_item_impl(p, attrs));
    } else if eat_word(p, "resource") {
        ret some(parse_item_res(p, attrs));
    } else if eat_word(p, "class") {
        ret some(parse_item_class(p, attrs));
    }
else { ret none; }
}

fn parse_use(p: parser) -> ast::view_item_ {
    let ident = parse_ident(p);
    let metadata = parse_optional_meta(p);
    ret ast::view_item_use(ident, metadata, p.get_id());
}

fn parse_view_path(p: parser) -> @ast::view_path {
    let lo = p.span.lo;
    let first_ident = parse_ident(p);
    let mut path = [first_ident];
    #debug("parsed view_path: %s", first_ident);
    alt p.token {
      token::EQ {
        // x = foo::bar
        p.bump();
        path = [parse_ident(p)];
        while p.token == token::MOD_SEP {
            p.bump();
            let id = parse_ident(p);
            path += [id];
        }
        let mut hi = p.span.hi;
        ret @spanned(lo, hi,
                     ast::view_path_simple(first_ident,
                        @spanned(lo, hi,
                                 {global: false, idents: path,
                                         types: []}),
                        p.get_id()));
      }

      token::MOD_SEP {
        // foo::bar or foo::{a,b,c} or foo::*
        while p.token == token::MOD_SEP {
            p.bump();

            alt p.token {

              token::IDENT(i, _) {
                p.bump();
                path += [p.get_str(i)];
              }

              // foo::bar::{a,b,c}
              token::LBRACE {
                let idents =
                    parse_seq(token::LBRACE, token::RBRACE,
                              seq_sep(token::COMMA),
                              parse_path_list_ident, p).node;
                let mut hi = p.span.hi;
                ret @spanned(lo, hi,
                             ast::view_path_list(@spanned(lo, hi,
                                {global: false,
                                 idents: path,
                                        types: []}), idents,
                             p.get_id()));
              }

              // foo::bar::*
              token::BINOP(token::STAR) {
                p.bump();
                let mut hi = p.span.hi;
                ret @spanned(lo, hi,
                             ast::view_path_glob(@spanned(lo, hi,
                               {global: false,
                                idents: path,
                                types: []}),
                               p.get_id()));
              }

              _ { break; }
            }
        }
      }
      _ { }
    }
    let mut hi = p.span.hi;
    let last = path[vec::len(path) - 1u];
    ret @spanned(lo, hi,
                 ast::view_path_simple(last, @spanned(lo, hi,
                                                      {global: false,
                                                              idents: path,
                                                              types: []}),
                                       p.get_id()));
}

fn parse_view_paths(p: parser) -> [@ast::view_path] {
    let mut vp = [parse_view_path(p)];
    while p.token == token::COMMA {
        p.bump();
        vp += [parse_view_path(p)];
    }
    ret vp;
}

fn parse_view_item(p: parser) -> @ast::view_item {
    let lo = p.span.lo;
    let the_item =
        if eat_word(p, "use") {
            parse_use(p)
        } else if eat_word(p, "import") {
            ast::view_item_import(parse_view_paths(p))
        } else if eat_word(p, "export") {
            ast::view_item_export(parse_view_paths(p))
        } else {
            fail
    };
    let mut hi = p.span.lo;
    expect(p, token::SEMI);
    ret @spanned(lo, hi, the_item);
}

fn is_view_item(p: parser) -> bool {
    is_word(p, "use") || is_word(p, "import") || is_word(p, "export")
}

fn maybe_parse_view(
    p: parser,
    first_item_attrs: [ast::attribute]) -> [@ast::view_item] {

    maybe_parse_view_while(p, first_item_attrs, is_view_item)
}

fn maybe_parse_view_import_only(
    p: parser,
    first_item_attrs: [ast::attribute]) -> [@ast::view_item] {

    maybe_parse_view_while(p, first_item_attrs, bind is_word(_, "import"))
}

fn maybe_parse_view_while(
    p: parser,
    first_item_attrs: [ast::attribute],
    f: fn@(parser) -> bool) -> [@ast::view_item] {

    if vec::len(first_item_attrs) == 0u {
        let mut items = [];
        while f(p) { items += [parse_view_item(p)]; }
        ret items;
    } else {
        // Shouldn't be any view items since we've already parsed an item attr
        ret [];
    }
}

fn parse_native_view(p: parser) -> [@ast::view_item] {
    maybe_parse_view_while(p, [], is_view_item)
}

// Parses a source module as a crate
fn parse_crate_mod(p: parser, _cfg: ast::crate_cfg) -> @ast::crate {
    let lo = p.span.lo;
    let crate_attrs = parse_inner_attrs_and_next(p);
    let first_item_outer_attrs = crate_attrs.next;
    let m = parse_mod_items(p, token::EOF, first_item_outer_attrs);
    ret @spanned(lo, p.span.lo,
                 {directives: [],
                  module: m,
                  attrs: crate_attrs.inner,
                  config: p.cfg});
}

fn parse_str(p: parser) -> str {
    alt p.token {
      token::LIT_STR(s) { p.bump(); p.get_str(s) }
      _ {
        p.fatal("expected string literal")
      }
    }
}

// Logic for parsing crate files (.rc)
//
// Each crate file is a sequence of directives.
//
// Each directive imperatively extends its environment with 0 or more items.
fn parse_crate_directive(p: parser, first_outer_attr: [ast::attribute]) ->
   ast::crate_directive {

    // Collect the next attributes
    let outer_attrs = first_outer_attr + parse_outer_attributes(p);
    // In a crate file outer attributes are only going to apply to mods
    let expect_mod = vec::len(outer_attrs) > 0u;

    let lo = p.span.lo;
    if expect_mod || is_word(p, "mod") {
        expect_word(p, "mod");
        let id = parse_ident(p);
        alt p.token {
          // mod x = "foo.rs";
          token::SEMI {
            let mut hi = p.span.hi;
            p.bump();
            ret spanned(lo, hi, ast::cdir_src_mod(id, outer_attrs));
          }
          // mod x = "foo_dir" { ...directives... }
          token::LBRACE {
            p.bump();
            let inner_attrs = parse_inner_attrs_and_next(p);
            let mod_attrs = outer_attrs + inner_attrs.inner;
            let next_outer_attr = inner_attrs.next;
            let cdirs =
                parse_crate_directives(p, token::RBRACE, next_outer_attr);
            let mut hi = p.span.hi;
            expect(p, token::RBRACE);
            ret spanned(lo, hi,
                        ast::cdir_dir_mod(id, cdirs, mod_attrs));
          }
          _ { unexpected(p); }
        }
    } else if is_view_item(p) {
        let vi = parse_view_item(p);
        ret spanned(lo, vi.span.hi, ast::cdir_view_item(vi));
    } else { ret p.fatal("expected crate directive"); }
}

fn parse_crate_directives(p: parser, term: token::token,
                          first_outer_attr: [ast::attribute]) ->
   [@ast::crate_directive] {

    // This is pretty ugly. If we have an outer attribute then we can't accept
    // seeing the terminator next, so if we do see it then fail the same way
    // parse_crate_directive would
    if vec::len(first_outer_attr) > 0u && p.token == term {
        expect_word(p, "mod");
    }

    let mut cdirs: [@ast::crate_directive] = [];
    let mut first_outer_attr = first_outer_attr;
    while p.token != term {
        let cdir = @parse_crate_directive(p, first_outer_attr);
        cdirs += [cdir];
        first_outer_attr = [];
    }
    ret cdirs;
}

//
// Local Variables:
// mode: rust
// fill-column: 78;
// indent-tabs-mode: nil
// c-basic-offset: 4
// buffer-file-coding-system: utf-8-unix
// End:
//
