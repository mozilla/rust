use rustc_ast::ptr::P;
use rustc_ast::tokenstream::{DelimSpan, TokenStream};
use rustc_ast::*;
use rustc_expand::base::*;
use rustc_span::symbol::sym;
use rustc_span::Span;

// This expands to either
// - `$crate::panic::panic_2015!(...)` or
// - `$crate::panic::panic_2021!(...)`
// depending on the edition.
//
// This is used for both std::panic!() and core::panic!().
//
// `$crate` will refer to either the `std` or `core` crate depending on which
// one we're expanding from.
pub fn expand_panic<'cx>(
    cx: &'cx mut ExtCtxt<'_>,
    sp: Span,
    tts: TokenStream,
) -> Box<dyn MacResult + 'cx> {
    let panic = if sp.rust_2021() { sym::panic_2021 } else { sym::panic_2015 };

    let sp = cx.with_call_site_ctxt(sp);

    MacEager::expr(
        cx.expr(
            sp,
            ExprKind::MacCall(MacCall {
                path: Path {
                    span: sp,
                    segments: cx
                        .std_path(&[sym::panic, panic])
                        .into_iter()
                        .map(|ident| PathSegment::from_ident(ident))
                        .collect(),
                    tokens: None,
                },
                args: P(MacArgs::Delimited(
                    DelimSpan::from_single(sp),
                    MacDelimiter::Parenthesis,
                    tts,
                )),
                prior_type_ascription: None,
            }),
        ),
    )
}
