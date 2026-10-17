//! The packed stored form of a zero-copy `#[defined_type]` enum for on-chain
//! code, from `naclac_syn::zc_codegen::enum_codec`, which the generated Rust
//! clients share, with `naclac_lang::prelude` as the path to `ZcCodec`,
//! `ZcEnumKind` and `ZcStorage`.

use {
    proc_macro2::{Ident, TokenStream},
    syn::{Type, Variant},
};

/// The `ZcCodec` and `ZcEnumKind` impls for `ident`, whose tag type is
/// `tag_ty`. `cfg` is prepended to each item.
pub fn generate(
    ident: &Ident,
    variants: &syn::punctuated::Punctuated<Variant, syn::Token![,]>,
    tag_ty: &Type,
    cfg: &TokenStream,
) -> TokenStream {
    naclac_syn::zc_codegen::enum_codec::generate(
        ident,
        variants,
        tag_ty,
        cfg,
        &syn::parse_quote!(naclac_lang::prelude),
    )
}
