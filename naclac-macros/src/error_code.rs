//! # Error Code Macro Logic
//!
//! Handles the expansion of the `#[error_code]` macro. It transforms an enum of custom errors
//! into native Solana `ProgramError`s, automatically offsetting the discriminants by 6000
//! to conform with standard SBF framework conventions.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemEnum};

/// Builds `impl #name { pub fn __naclac_idl_errors() -> Vec<IdlError> }` —
/// each variant's `code` is its *real* Rust discriminant (via the same
/// shared `variant_discriminants` the compiled `From<#name> for
/// ProgramError` impl's own `e as u32 + 6000` relies on) plus 6000, not a
/// sequential-by-position guess, so this can never silently disagree with
/// what actually lands on-chain the way the AST-walker's default path
/// currently can (see docs/plan/idl-build-compilation-migration.md). Each
/// variant's doc comment (if any) becomes its `message`.
#[cfg(feature = "idl-build")]
fn idl_build_errors_impl(
    name: &syn::Ident,
    variants: &syn::punctuated::Punctuated<syn::Variant, syn::Token![,]>,
) -> proc_macro2::TokenStream {
    let discriminants = naclac_syn::discriminator::variant_discriminants(variants.iter());
    let mut error_ts_list = Vec::new();
    for (variant, discriminant) in variants.iter().zip(discriminants.iter()) {
        let variant_name = variant.ident.to_string();
        let code = (*discriminant + 6000) as u32;
        let docs = naclac_syn::parser::extract_docs(&variant.attrs);
        let message_tokens = if docs.is_empty() {
            quote! { None }
        } else {
            let joined = docs.join(" ");
            quote! { Some(#joined.to_string()) }
        };
        error_ts_list.push(quote! {
            naclac_lang::naclac_idl::IdlError {
                code: #code,
                name: #variant_name.into(),
                message: #message_tokens,
            }
        });
    }

    quote! {
        #[cfg(feature = "idl-build")]
        impl #name {
            pub fn __naclac_idl_errors() -> naclac_lang::prelude::Vec<naclac_lang::naclac_idl::IdlError> {
                vec![#(#error_ts_list),*]
            }
        }
    }
}

#[cfg(not(feature = "idl-build"))]
fn idl_build_errors_impl(
    _name: &syn::Ident,
    _variants: &syn::punctuated::Punctuated<syn::Variant, syn::Token![,]>,
) -> proc_macro2::TokenStream {
    quote! {}
}

/// Expands the `#[error_code]` macro.
///
/// Implements `From<YourError> for ProgramError` and `Display`.
pub fn expand(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemEnum);
    let name = &input.ident;
    let idl_build_impl = idl_build_errors_impl(name, &input.variants);

    let expanded = quote! {
        #[cfg_attr(feature = "debug-mode", derive(Debug))]
        #[derive(Copy, Clone, Eq, PartialEq)]
        #input

        // Automatically converts custom errors into native SBF ProgramErrors.
        // Offsets by 6000 (standard framework convention) to avoid collision with native errors.
        impl From<#name> for naclac_lang::prelude::ProgramError {
            fn from(e: #name) -> Self {
                naclac_lang::prelude::ProgramError::Custom(e as u32 + 6000)
            }
        }

        #[cfg(feature = "debug-mode")]
        impl naclac_lang::prelude::fmt::Display for #name {
            fn fmt(&self, f: &mut naclac_lang::prelude::fmt::Formatter<'_>) -> naclac_lang::prelude::fmt::Result {
                write!(f, "Naclac Error: {:?}", self)
            }
        }

        #idl_build_impl
    };

    TokenStream::from(expanded)
}
