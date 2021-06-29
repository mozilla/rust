use rustc_ast::ast::DUMMY_NODE_ID;
use rustc_ast::ptr::P;
use rustc_ast::{
    AssocItem, AssocItemKind, Attribute, Const, Defaultness, EnumDef, GenericBounds, Generics,
    ImplKind, ImplPolarity, Item, ItemKind, Lit, LitKind, MetaItem, MetaItemKind, NestedMetaItem,
    Path, PathSegment, StrStyle, Ty, TyAliasKind, Unsafe, Visibility, VisibilityKind,
};
use rustc_attr::find_repr_attrs;
use rustc_errors::struct_span_err;
use rustc_expand::base::{Annotatable, ExtCtxt};
use rustc_span::symbol::{sym, Ident};
use rustc_span::{Span, Symbol, DUMMY_SP};

macro_rules! invalid_derive {
    ($cx:ident, $span:ident, $reason:expr) => {
        struct_span_err!(
            &$cx.sess.parse_sess.span_diagnostic,
            $span,
            FIXME,
            "`AsRepr` can only be derived for {}",
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
    match *item {
        Annotatable::Item(ref annitem) => match annitem.kind {
            ItemKind::Enum(ref def, _) => {
                let repr = match extract_repr(cx, annitem, def) {
                    Ok(repr) => repr,
                    Err(reason) => {
                        invalid_derive!(cx, span, reason);
                        return;
                    }
                };

                let ty = cx.ty_ident(DUMMY_SP, annitem.ident);
                let repr_ty = cx.ty_ident(DUMMY_SP, Ident { name: repr, span: DUMMY_SP });

                push(make_as_repr(cx, ty.clone(), repr_ty.clone()));
            }
            _ => invalid_derive!(cx, span, "enums"),
        },
        _ => invalid_derive!(cx, span, "enums"),
    }
}

fn extract_repr(
    cx: &mut ExtCtxt<'_>,
    annitem: &P<Item>,
    def: &EnumDef,
) -> Result<Symbol, &'static str> {
    let reprs: Vec<_> = find_repr_attrs(&cx.sess.parse_sess, &annitem.attrs)
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

fn std_path_from_ident_symbols(cx: &ExtCtxt<'_>, symbols: &[Symbol]) -> Path {
    Path {
        span: DUMMY_SP,
        segments: cx.std_path(symbols).into_iter().map(PathSegment::from_ident).collect(),
        tokens: None,
    }
}
