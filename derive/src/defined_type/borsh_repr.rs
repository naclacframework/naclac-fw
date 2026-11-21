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
///
/// All Borsh options go in one `#[borsh(..)]` attribute: `borsh-derive` before
/// 1.8.0 rejects an item that carries several.
pub(super) fn variant(input: &syn::DeriveInput, is_enum: bool, cfg: &TokenStream) -> TokenStream {
    let mut input = input.clone();
    if derive_attr_listing(&input.attrs, "Clone").is_none() {
        input.attrs.push(syn::parse_quote!(#[derive(Clone)]));
    }
    let borsh_attr = if is_enum {
        quote::quote! {
            #[borsh(crate = "naclac_lang::prelude::borsh", use_discriminant = true)]
        }
    } else {
        quote::quote! { #[borsh(crate = "naclac_lang::prelude::borsh")] }
    };
    quote::quote! {
        #cfg
        #[derive(naclac_lang::prelude::BorshSerialize, naclac_lang::prelude::BorshDeserialize)]
        #borsh_attr
        #input
    }
}
