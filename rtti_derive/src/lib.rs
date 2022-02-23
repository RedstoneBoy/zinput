extern crate proc_macro;

use proc_macro::TokenStream;
use syn::{Data, DeriveInput, Fields, punctuated::Pair, parse_macro_input};

#[proc_macro_derive(TypeInfo)]
pub fn derive_type_info(input: TokenStream) -> TokenStream {
    let derive_input = parse_macro_input!(input as DeriveInput);

    let struct_type = match derive_input.data {
        Data::Struct(struct_type) => struct_type,
        _ => return quote! { compile_error!("TypeInfo can only be derived on a struct (currently)"); }
    };

    let fields = match struct_type.fields  {
        Fields::Named(fields) => fields,
        _ => return quote! { compile_error!("TypeInfo can only be derived on a struct with named fields (currently)"); }
    };

    let fields = fields
        .into_iter()
        .map(|(Pair::Punctuated(f, _) | Pair::End(f))| f)
        .collect::<Vec<_>>();
}