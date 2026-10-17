//! The zero-copy struct layout checks for on-chain code, from
//! `naclac_syn::zc_codegen::struct_checks`, which the generated Rust clients
//! share, with `naclac_lang::prelude` as the path to `ZcField`.

use {
    proc_macro2::{Ident, TokenStream},
    syn::Fields,
};

pub use naclac_syn::zc_codegen::struct_checks::repr_error;

/// The `const` assertions for the struct `ident` with `fields`, to be emitted
/// alongside the struct. `cfg` is prepended to every item.
pub fn generate(ident: &Ident, fields: &Fields, cfg: &TokenStream) -> TokenStream {
    naclac_syn::zc_codegen::struct_checks::generate(
        ident,
        fields,
        cfg,
        &syn::parse_quote!(naclac_lang::prelude),
    )
}
