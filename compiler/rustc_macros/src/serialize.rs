use proc_macro2::TokenStream;
use quote::quote;
use syn::parse_quote;

pub fn type_decodable_derive(mut s: synstructure::Structure<'_>) -> proc_macro2::TokenStream {
    let decoder_ty = quote! { __D };
    if !s.ast().generics.lifetimes().any(|lt| lt.lifetime.ident == "tcx") {
        s.add_impl_generic(parse_quote! { 'tcx });
    }
    s.add_impl_generic(parse_quote! {#decoder_ty: ::rustc_middle::ty::codec::TyDecoder<'tcx>});
    s.add_bounds(synstructure::AddBounds::Generics);

    decodable_body(s, decoder_ty)
}

pub fn meta_decodable_derive(mut s: synstructure::Structure<'_>) -> proc_macro2::TokenStream {
    if !s.ast().generics.lifetimes().any(|lt| lt.lifetime.ident == "tcx") {
        s.add_impl_generic(parse_quote! { 'tcx });
    }
    s.add_impl_generic(parse_quote! { '__a });
    let decoder_ty = quote! { DecodeContext<'__a, 'tcx> };
    s.add_bounds(synstructure::AddBounds::Generics);

    decodable_body(s, decoder_ty)
}

pub fn decodable_derive(mut s: synstructure::Structure<'_>) -> proc_macro2::TokenStream {
    let decoder_ty = quote! { __D };
    s.add_impl_generic(parse_quote! {#decoder_ty: ::rustc_serialize::Decoder});
    s.add_bounds(synstructure::AddBounds::Generics);

    decodable_body(s, decoder_ty)
}

fn decodable_body(
    s: synstructure::Structure<'_>,
    decoder_ty: TokenStream,
) -> proc_macro2::TokenStream {
    if let syn::Data::Union(_) = s.ast().data {
        panic!("cannot derive on union")
    }
    let ty_name = s.ast().ident.to_string();
    let decode_body = match s.variants() {
        [vi] => {
            let construct = vi.construct(|field, _index| decode_field(field, true));
            quote! {
                ::rustc_serialize::Decoder::read_struct(
                    __decoder,
                    |__decoder| { ::std::result::Result::Ok(#construct) },
                )
            }
        }
        variants => {
            let match_inner: TokenStream = variants
                .iter()
                .enumerate()
                .map(|(idx, vi)| {
                    let construct = vi.construct(|field, _index| decode_field(field, false));
                    quote! { #idx => { ::std::result::Result::Ok(#construct) } }
                })
                .collect();
            let message = format!(
                "invalid enum variant tag while decoding `{}`, expected 0..{}",
                ty_name,
                variants.len()
            );
            quote! {
                ::rustc_serialize::Decoder::read_enum(
                    __decoder,
                    |__decoder| {
                        ::rustc_serialize::Decoder::read_enum_variant(
                            __decoder,
                            |__decoder, __variant_idx| {
                                match __variant_idx {
                                    #match_inner
                                    _ => return ::std::result::Result::Err(
                                        ::rustc_serialize::Decoder::error(__decoder, #message)),
                                }
                            })
                    }
                )
            }
        }
    };

    s.bound_impl(
        quote!(::rustc_serialize::Decodable<#decoder_ty>),
        quote! {
            fn decode(
                __decoder: &mut #decoder_ty,
            ) -> ::std::result::Result<Self, <#decoder_ty as ::rustc_serialize::Decoder>::Error> {
                #decode_body
            }
        },
    )
}

fn decode_field(field: &syn::Field, is_struct: bool) -> proc_macro2::TokenStream {
    let decode_inner_method = if let syn::Type::Reference(_) = field.ty {
        quote! { ::rustc_middle::ty::codec::RefDecodable::decode }
    } else {
        quote! { ::rustc_serialize::Decodable::decode }
    };
    let decode_method = if is_struct {
        proc_macro2::Ident::new("read_struct_field", proc_macro2::Span::call_site())
    } else {
        proc_macro2::Ident::new("read_enum_variant_arg", proc_macro2::Span::call_site())
    };

    quote! {
        match ::rustc_serialize::Decoder::#decode_method(
            __decoder, #decode_inner_method) {
            ::std::result::Result::Ok(__res) => __res,
            ::std::result::Result::Err(__err) => return ::std::result::Result::Err(__err),
        }
    }
}

pub fn type_encodable_derive(mut s: synstructure::Structure<'_>) -> proc_macro2::TokenStream {
    if !s.ast().generics.lifetimes().any(|lt| lt.lifetime.ident == "tcx") {
        s.add_impl_generic(parse_quote! {'tcx});
    }
    let encoder_ty = quote! { __E };
    s.add_impl_generic(parse_quote! {#encoder_ty: ::rustc_middle::ty::codec::TyEncoder<'tcx>});
    s.add_bounds(synstructure::AddBounds::Generics);

    encodable_body(s, encoder_ty, false)
}

pub fn meta_encodable_derive(mut s: synstructure::Structure<'_>) -> proc_macro2::TokenStream {
    if !s.ast().generics.lifetimes().any(|lt| lt.lifetime.ident == "tcx") {
        s.add_impl_generic(parse_quote! {'tcx});
    }
    s.add_impl_generic(parse_quote! { '__a });
    let encoder_ty = quote! { EncodeContext<'__a, 'tcx> };
    s.add_bounds(synstructure::AddBounds::Generics);

    encodable_body(s, encoder_ty, true)
}

pub fn encodable_derive(mut s: synstructure::Structure<'_>) -> proc_macro2::TokenStream {
    let encoder_ty = quote! { __E };
    s.add_impl_generic(parse_quote! { #encoder_ty: ::rustc_serialize::Encoder});
    s.add_bounds(synstructure::AddBounds::Generics);

    encodable_body(s, encoder_ty, false)
}

fn encodable_body(
    mut s: synstructure::Structure<'_>,
    encoder_ty: TokenStream,
    allow_unreachable_code: bool,
) -> proc_macro2::TokenStream {
    if let syn::Data::Union(_) = s.ast().data {
        panic!("cannot derive on union")
    }

    s.bind_with(|binding| {
        // Handle the lack of a blanket reference impl.
        if let syn::Type::Reference(_) = binding.ast().ty {
            synstructure::BindStyle::Move
        } else {
            synstructure::BindStyle::Ref
        }
    });

    let encode_body = match s.variants() {
        [_] => {
            let encode_inner = s.each_variant(|vi| {
                vi.bindings()
                    .iter()
                    .map(|binding| {
                        let bind_ident = &binding.binding;
                        let result = quote! {
                            match ::rustc_serialize::Encoder::emit_struct_field(
                                __encoder,
                                |__encoder|
                                ::rustc_serialize::Encodable::<#encoder_ty>::encode(#bind_ident, __encoder),
                            ) {
                                ::std::result::Result::Ok(()) => (),
                                ::std::result::Result::Err(__err)
                                    => return ::std::result::Result::Err(__err),
                            }
                        };
                        result
                    })
                    .collect::<TokenStream>()
            });
            quote! {
                ::rustc_serialize::Encoder::emit_struct(__encoder, |__encoder| {
                    ::std::result::Result::Ok(match *self { #encode_inner })
                })
            }
        }
        _ => {
            let mut variant_idx = 0usize;
            let encode_inner = s.each_variant(|vi| {
                let encode_fields: TokenStream = vi
                    .bindings()
                    .iter()
                    .map(|binding| {
                        let bind_ident = &binding.binding;
                        let result = quote! {
                            match ::rustc_serialize::Encoder::emit_enum_variant_arg(
                                __encoder,
                                |__encoder|
                                ::rustc_serialize::Encodable::<#encoder_ty>::encode(#bind_ident, __encoder),
                            ) {
                                ::std::result::Result::Ok(()) => (),
                                ::std::result::Result::Err(__err)
                                    => return ::std::result::Result::Err(__err),
                            }
                        };
                        result
                    })
                    .collect();

                let result = quote! { ::rustc_serialize::Encoder::emit_enum_variant(
                    __encoder,
                   #variant_idx,
                   |__encoder| { ::std::result::Result::Ok({ #encode_fields }) }
                ) };
                variant_idx += 1;
                result
            });
            quote! {
                ::rustc_serialize::Encoder::emit_enum(__encoder, |__encoder| {
                    match *self {
                        #encode_inner
                    }
                })
            }
        }
    };

    let lints = if allow_unreachable_code {
        quote! { #![allow(unreachable_code)] }
    } else {
        quote! {}
    };

    s.bound_impl(
        quote!(::rustc_serialize::Encodable<#encoder_ty>),
        quote! {
            fn encode(
                &self,
                __encoder: &mut #encoder_ty,
            ) -> ::std::result::Result<(), <#encoder_ty as ::rustc_serialize::Encoder>::Error> {
                #lints
                #encode_body
            }
        },
    )
}
