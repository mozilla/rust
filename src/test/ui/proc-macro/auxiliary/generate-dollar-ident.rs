// force-host
// no-prefer-dynamic

#![feature(proc_macro_hygiene)]
#![feature(proc_macro_quote)]
#![crate_type = "proc-macro"]

extern crate proc_macro;
use proc_macro::*;

#[proc_macro]
pub fn dollar_ident(input: TokenStream) -> TokenStream {
    let black_hole = input.into_iter().next().unwrap();
    quote! {
        $black_hole!($$var);
    }
}
