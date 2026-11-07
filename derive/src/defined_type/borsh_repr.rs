//! The Borsh representation of a `#[defined_type]`: the item unchanged apart
//! from `Clone` and the Borsh derives.

use proc_macro2::TokenStream;

use crate::common::derive_attrs::derive_attr_listing;

/// `input` with `Clone` (unless already derived), the Borsh derives and, for
/// an enum, `use_discriminant = true`, gated by `cfg`.
///
/// `use_discriminant` makes Borsh encode the enum's real compiler-assigned
/// discriminant (explicit `= N`, else previous + 1) instead of the variant's
/// position.
pub(super) fn variant(input: &syn::DeriveInput, is_enum: bool, cfg: &TokenStream) -> TokenStream {
    let mut input = input.clone();
    if derive_attr_listing(&input.attrs, "Clone").is_none() {
        input.attrs.push(syn::parse_quote!(#[derive(Clone)]));
    }
    if is_enum {
        input
            .attrs
            .push(syn::parse_quote!(#[borsh(use_discriminant = true)]));
    }
    quote::quote! {
        #cfg
        #[derive(naclac_lang::prelude::BorshSerialize, naclac_lang::prelude::BorshDeserialize)]
        #[borsh(crate = "naclac_lang::prelude::borsh")]
        #input
    }
}
