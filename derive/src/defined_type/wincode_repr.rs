//! The Wincode representation of a `#[defined_type]`: the item unchanged apart
//! from `Clone`, the Wincode derives and an explicit tag on every enum variant.

use proc_macro2::TokenStream;

use crate::common::derive_attrs::derive_attr_listing;

/// `input` with `Clone` (unless already derived), the Wincode derives and, for
/// an enum, `#[wincode(tag = N)]` on each variant, gated by `cfg`.
///
/// `N` is the value from `naclac_syn::discriminator::variant_discriminants`,
/// the same one the IDL and the zero-copy representation use; without the
/// attribute wincode-derive would tag by declaration index.
pub(super) fn variant(input: &syn::DeriveInput, cfg: &TokenStream) -> TokenStream {
    let mut input = input.clone();
    if derive_attr_listing(&input.attrs, "Clone").is_none() {
        input.attrs.push(syn::parse_quote!(#[derive(Clone)]));
    }
    if let syn::Data::Enum(data_enum) = &mut input.data {
        let discriminants =
            naclac_syn::discriminator::variant_discriminants(data_enum.variants.iter());
        for (variant, discriminant) in data_enum.variants.iter_mut().zip(discriminants.iter()) {
            let tag_lit = syn::LitInt::new(&discriminant.to_string(), variant.ident.span());
            variant
                .attrs
                .push(syn::parse_quote!(#[wincode(tag = #tag_lit)]));
        }
    }
    quote::quote! {
        #cfg
        #[derive(naclac_lang::prelude::SchemaWrite, naclac_lang::prelude::SchemaRead)]
        #[wincode(crate = "naclac_lang::prelude::wincode")]
        #input
    }
}
