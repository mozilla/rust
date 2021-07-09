use rustc_ast::ast::DUMMY_NODE_ID;
use rustc_ast::attr::mk_list_item;
use rustc_ast::ptr::P;
use rustc_ast::{
    AngleBracketedArg, AngleBracketedArgs, Arm, AssocItem, AssocItemKind, Attribute, BinOpKind,
    BindingMode, Block, Const, Defaultness, EnumDef, Expr, FnHeader, FnKind, FnRetTy, FnSig,
    GenericArg, GenericArgs, GenericBounds, Generics, ImplKind, ImplPolarity, Item, ItemKind, Lit,
    LitIntType, LitKind, MetaItem, MetaItemKind, Mutability, NestedMetaItem, PatKind, Path,
    PathSegment, Stmt, StmtKind, StrStyle, Ty, TyAliasKind, Unsafe, Variant, Visibility,
    VisibilityKind,
};
use rustc_attr::find_repr_attrs;
use rustc_data_structures::thin_vec::ThinVec;
use rustc_errors::struct_span_err;
use rustc_expand::base::{Annotatable, ExtCtxt};
use rustc_span::symbol::{sym, Ident};
use rustc_span::{Span, Symbol, DUMMY_SP};

macro_rules! invalid_derive {
    ($trait_ident:expr, $cx:ident, $span:ident, $reason:expr) => {
        struct_span_err!(
            &$cx.sess.parse_sess.span_diagnostic,
            $span,
            FIXME,
            "`{}` can only be derived for {}",
            $trait_ident,
            $reason
        )
        .emit()
    };
}

pub fn expand_as_repr(
    cx: &mut ExtCtxt<'_>,
    span: Span,
    _mitem: &MetaItem,
    item: &Annotatable,
    push: &mut dyn FnMut(Annotatable),
) {
    if let Some(features) = cx.ecfg.features {
        if !features.enabled(sym::enum_as_repr) {
            return;
        }
    }

    let ctx = Ctx::extract(cx, item);

    match ctx {
        Ok(ctx) => push(make_as_repr(cx, ctx.ty, ctx.repr_ty)),
        Err(reason) => invalid_derive!("AsRepr", cx, span, reason),
    };
}

pub fn expand_from_repr(
    cx: &mut ExtCtxt<'_>,
    span: Span,
    _mitem: &MetaItem,
    item: &Annotatable,
    push: &mut dyn FnMut(Annotatable),
) {
    let ctx = Ctx::extract(cx, item);

    match ctx {
        Ok(ctx) => push(make_from_repr(cx, ctx.enum_def, ctx.ty, ctx.repr_ty)),
        Err(reason) => invalid_derive!("FromRepr", cx, span, reason),
    };
}

struct Ctx<'enumdef> {
    enum_def: &'enumdef EnumDef,
    ty: P<Ty>,
    repr_ty: P<Ty>,
}

impl<'enumdef> Ctx<'enumdef> {
    fn extract(
        cx: &mut ExtCtxt<'_>,
        item: &'enumdef Annotatable,
    ) -> Result<Ctx<'enumdef>, &'static str> {
        match *item {
            Annotatable::Item(ref annitem) => match annitem.kind {
                ItemKind::Enum(ref enum_def, _) => {
                    let repr = extract_repr(cx, annitem, enum_def)?;

                    let ty = cx.ty_ident(DUMMY_SP, annitem.ident);
                    let repr_ty = cx.ty_ident(DUMMY_SP, Ident { name: repr, span: DUMMY_SP });

                    Ok(Self { enum_def, ty, repr_ty })
                }
                _ => Err("enums"),
            },
            _ => Err("enums"),
        }
    }
}

fn extract_repr(
    cx: &mut ExtCtxt<'_>,
    annitem: &P<Item>,
    def: &EnumDef,
) -> Result<Symbol, &'static str> {
    let reprs: Vec<_> = find_repr_attrs(&cx.sess.parse_sess, &annitem.attrs, true)
        .into_iter()
        .filter_map(|r| {
            use rustc_attr::*;
            match r {
                ReprInt(rustc_attr::IntType::UnsignedInt(int_type)) => Some(int_type.name()),
                ReprInt(rustc_attr::IntType::SignedInt(int_type)) => Some(int_type.name()),
                ReprC | ReprPacked(..) | ReprSimd | ReprTransparent | ReprAlign(..)
                | ReprNoNiche => None,
            }
        })
        .collect();
    if reprs.len() != 1 {
        return Err("enums with an explicit integer representation");
    }

    if !def.is_fieldless() {
        return Err("data-free enums");
    }
    return Ok(reprs[0]);
}

fn make_as_repr(cx: &mut ExtCtxt<'_>, ty: P<Ty>, repr_ty: P<Ty>) -> Annotatable {
    let assoc_type = assoc_type("Repr", repr_ty.clone());

    Annotatable::Item(cx.item(
        DUMMY_SP,
        Ident::invalid(),
        make_stability_attributes(cx),
        ItemKind::Impl(Box::new(ImplKind {
            unsafety: Unsafe::Yes(DUMMY_SP),
            polarity: ImplPolarity::Positive,
            defaultness: Defaultness::Final,
            constness: Const::No,
            generics: Generics::default(),
            of_trait: Some(cx.trait_ref(std_path_from_ident_symbols(
                cx,
                &[Symbol::intern("enums"), sym::AsRepr],
            ))),
            self_ty: ty.clone(),
            items: vec![assoc_type],
        })),
    ))
}

fn make_stability_attributes(cx: &ExtCtxt<'_>) -> Vec<Attribute> {
    if cx.ecfg.crate_name != "core" && cx.ecfg.crate_name != "std" {
        return Vec::new();
    }

    let attr = cx.attribute(MetaItem {
        path: cx.path(DUMMY_SP, vec![Ident::from_str("unstable")]),
        kind: MetaItemKind::List(vec![
            NestedMetaItem::MetaItem(MetaItem {
                path: cx.path(DUMMY_SP, vec![Ident::from_str("feature")]),
                kind: MetaItemKind::NameValue(Lit::from_lit_kind(
                    LitKind::Str(sym::enum_as_repr, StrStyle::Cooked),
                    DUMMY_SP,
                )),
                span: DUMMY_SP,
            }),
            NestedMetaItem::MetaItem(MetaItem {
                path: cx.path(DUMMY_SP, vec![Ident::from_str("issue")]),
                kind: MetaItemKind::NameValue(Lit::from_lit_kind(
                    LitKind::Str(Symbol::intern("none"), StrStyle::Cooked),
                    DUMMY_SP,
                )),
                span: DUMMY_SP,
            }),
        ]),
        span: Default::default(),
    });

    vec![attr]
}

fn make_from_repr(cx: &mut ExtCtxt<'_>, def: &EnumDef, ty: P<Ty>, repr_ty: P<Ty>) -> Annotatable {
    let param_ident = Ident::from_str("value");
    let result_type = make_result_type(cx, ty.clone(), repr_ty.clone());

    let try_from_repr = {
        let decl = cx.fn_decl(
            vec![cx.param(DUMMY_SP, param_ident.clone(), repr_ty.clone())],
            FnRetTy::Ty(result_type),
        );

        assoc_item(
            "try_from_repr",
            AssocItemKind::Fn(Box::new(FnKind(
                Defaultness::Final,
                FnSig { header: FnHeader::default(), decl, span: DUMMY_SP },
                Generics::default(),
                Some(make_match_block(cx, def, repr_ty.clone(), param_ident)),
            ))),
        )
    };

    Annotatable::Item(cx.item(
        DUMMY_SP,
        Ident::invalid(),
        make_stability_attributes(cx),
        ItemKind::Impl(Box::new(ImplKind {
            unsafety: Unsafe::Yes(DUMMY_SP),
            polarity: ImplPolarity::Positive,
            defaultness: Defaultness::Final,
            constness: Const::No,
            generics: Generics::default(),
            of_trait: Some(cx.trait_ref(std_path_from_ident_symbols(
                cx,
                &[Symbol::intern("enums"), sym::FromRepr],
            ))),
            self_ty: ty.clone(),
            items: vec![try_from_repr],
        })),
    ))
}

fn make_match_block(
    cx: &mut ExtCtxt<'_>,
    def: &EnumDef,
    repr_ty: P<Ty>,
    value_ident: Ident,
) -> P<Block> {
    let ok = std_path_from_ident_symbols(cx, &[sym::result, sym::Result, sym::Ok]);
    let ok = cx.expr_path(ok);

    let mut prev_explicit_disr: Option<P<Expr>> = None;
    let mut count_since_prev_explicit_disr = 0;

    let mut stmts = Vec::with_capacity(def.variants.len() + 1);
    let mut arms = Vec::with_capacity(def.variants.len() + 1);

    for Variant { ident, disr_expr, .. } in &def.variants {
        let expr = match (disr_expr, &prev_explicit_disr) {
            (Some(disr), _) => {
                prev_explicit_disr = Some(disr.value.clone());
                count_since_prev_explicit_disr = 0;
                disr.value.clone()
            }
            (None, None) => {
                let expr = cx.expr_lit(
                    DUMMY_SP,
                    LitKind::Int(count_since_prev_explicit_disr, LitIntType::Unsuffixed),
                );
                count_since_prev_explicit_disr += 1;
                expr
            }
            (None, Some(prev_expr)) => {
                count_since_prev_explicit_disr += 1;
                cx.expr_binary(
                    DUMMY_SP,
                    BinOpKind::Add,
                    prev_expr.clone(),
                    cx.expr_lit(
                        DUMMY_SP,
                        LitKind::Int(count_since_prev_explicit_disr, LitIntType::Unsuffixed),
                    ),
                )
            }
        };

        let const_ident = Ident::from_str(&format!("DISCIMINANT_FOR_{}", arms.len()));
        stmts.push(
            cx.stmt_item(DUMMY_SP, cx.item_const(DUMMY_SP, const_ident, repr_ty.clone(), expr)),
        );

        arms.push(cx.arm(
            DUMMY_SP,
            cx.pat_ident(DUMMY_SP, const_ident),
            cx.expr_call(
                DUMMY_SP,
                ok.clone(),
                vec![cx.expr_path(cx.path(DUMMY_SP, vec![Ident::from_str("Self"), ident.clone()]))],
            ),
        ));
    }

    let err = std_path_from_ident_symbols(cx, &[sym::result, sym::Result, sym::Err]);
    let try_from_int_error = std_path_from_ident_symbols(
        cx,
        &[Symbol::intern("enums"), Symbol::intern("TryFromReprError")],
    );

    let other_match = Ident::from_str("other_value");

    // Rather than having to know how many variants could fit into the repr,
    // just allow the catch-all to be superfluous.
    arms.push(Arm {
        attrs: ThinVec::from(vec![cx.attribute(mk_list_item(
            Ident::new(sym::allow, DUMMY_SP),
            vec![NestedMetaItem::MetaItem(
                cx.meta_word(DUMMY_SP, Symbol::intern("unreachable_patterns")),
            )],
        ))]),
        pat: cx.pat(
            DUMMY_SP,
            PatKind::Ident(BindingMode::ByValue(Mutability::Not), other_match.clone(), None),
        ),
        guard: Option::None,
        body: cx.expr_call(
            DUMMY_SP,
            cx.expr_path(err),
            vec![cx.expr_call(
                DUMMY_SP,
                cx.expr_path(try_from_int_error),
                vec![cx.expr_ident(DUMMY_SP, other_match.clone())],
            )],
        ),
        span: DUMMY_SP,
        id: DUMMY_NODE_ID,
        is_placeholder: false,
    });

    stmts.push(Stmt {
        id: DUMMY_NODE_ID,
        span: DUMMY_SP,
        kind: StmtKind::Expr(cx.expr_match(DUMMY_SP, cx.expr_ident(DUMMY_SP, value_ident), arms)),
    });

    cx.block(DUMMY_SP, stmts)
}

fn assoc_type(name: &str, ty: P<Ty>) -> P<AssocItem> {
    assoc_item(
        name,
        AssocItemKind::TyAlias(Box::new(TyAliasKind(
            Defaultness::Final,
            Generics::default(),
            GenericBounds::default(),
            Some(ty),
        ))),
    )
}

fn assoc_item(name: &str, kind: AssocItemKind) -> P<AssocItem> {
    P(AssocItem {
        attrs: vec![],
        id: DUMMY_NODE_ID,
        span: DUMMY_SP,
        vis: Visibility { kind: VisibilityKind::Inherited, span: DUMMY_SP, tokens: None },
        ident: Ident::from_str(name),
        kind,
        tokens: None,
    })
}

fn make_result_type(cx: &mut ExtCtxt<'_>, ty: P<Ty>, repr_ty: P<Ty>) -> P<Ty> {
    std_path_with_generics(cx, &[sym::result], sym::Result, vec![ty, error_type(cx, repr_ty)])
}

fn std_path_with_generics(
    cx: &ExtCtxt<'_>,
    symbols: &[Symbol],
    ty: Symbol,
    generics: Vec<P<Ty>>,
) -> P<Ty> {
    let mut path = std_path_from_ident_symbols(cx, symbols);
    path.segments.push(path_segment_with_generics(ty, generics));
    cx.ty_path(path)
}

fn std_path_from_ident_symbols(cx: &ExtCtxt<'_>, symbols: &[Symbol]) -> Path {
    Path {
        span: DUMMY_SP,
        segments: cx.std_path(symbols).into_iter().map(PathSegment::from_ident).collect(),
        tokens: None,
    }
}

fn error_type(cx: &ExtCtxt<'_>, repr_ty: P<Ty>) -> P<Ty> {
    let mut error_type = std_path_from_ident_symbols(cx, &[Symbol::intern("enums")]);

    error_type
        .segments
        .push(path_segment_with_generics(Symbol::intern("TryFromReprError"), vec![repr_ty]));

    cx.ty_path(error_type)
}

fn path_segment_with_generics(symbol: Symbol, generic_types: Vec<P<Ty>>) -> PathSegment {
    PathSegment {
        ident: Ident { name: symbol, span: DUMMY_SP },
        id: DUMMY_NODE_ID,
        args: Some(P(GenericArgs::AngleBracketed(AngleBracketedArgs {
            span: DUMMY_SP,
            args: generic_types
                .into_iter()
                .map(|ty| AngleBracketedArg::Arg(GenericArg::Type(ty)))
                .collect(),
        }))),
    }
}
