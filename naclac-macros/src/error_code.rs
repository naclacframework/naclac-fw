//! # Error Code Macro Logic
//!
//! Handles the expansion of the `#[error_code]` macro. It transforms an enum of custom errors
//! into native Solana `ProgramError`s whose code is the enum's `offset` (6000 unless set with
//! `#[error_code(offset = N)]`) plus the variant's discriminant.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemEnum};

/// Builds `impl #name { pub fn __naclac_idl_errors() -> Vec<IdlError> }` —
/// each variant's `code` is `offset` plus its *real* Rust discriminant (via
/// the same shared `variant_discriminants` the AST IDL generator uses), so
/// it matches what the `From<#name> for ProgramError` impl puts on-chain.
/// Each variant's doc comment (if any) becomes its `message`.
#[cfg(feature = "idl-build")]
fn idl_build_errors_impl(
    name: &syn::Ident,
    variants: &syn::punctuated::Punctuated<syn::Variant, syn::Token![,]>,
    offset: u32,
) -> proc_macro2::TokenStream {
    let discriminants = naclac_syn::discriminator::variant_discriminants(variants.iter());
    let mut error_ts_list = Vec::new();
    for (variant, discriminant) in variants.iter().zip(discriminants.iter()) {
        let variant_name = variant.ident.to_string();
        let code = (i128::from(offset) + *discriminant) as u32;
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
    _offset: u32,
) -> proc_macro2::TokenStream {
    quote! {}
}

/// Compile-time assertions that every variant's on-chain code
/// (`offset + discriminant`) lies in the user range: at or above
/// `USER_ERROR_BASE` and below `FRAMEWORK_ERROR_BASE`, where naclac's own
/// framework errors begin. Also pins `naclac_syn`'s default offset to
/// `USER_ERROR_BASE`, which the AST IDL generator relies on.
fn user_code_range_asserts(
    name: &syn::Ident,
    variants: &syn::punctuated::Punctuated<syn::Variant, syn::Token![,]>,
    offset: u32,
) -> proc_macro2::TokenStream {
    let default_offset = naclac_syn::idl_items::DEFAULT_ERROR_OFFSET;
    let offset_message = format!(
        "#[error_code] on `{name}`: `offset = {offset}` is below naclac's user error range, which \
         starts at USER_ERROR_BASE (6000)"
    );
    let asserts = variants.iter().map(|variant| {
        let variant_ident = &variant.ident;
        let message = format!(
            "#[error_code] variant `{}::{}` has a negative or too-large discriminant: \
             user error codes ({offset} + discriminant) must stay below 10000, where \
             naclac's framework error codes begin",
            name, variant_ident
        );
        quote! {
            const _: () = ::core::assert!(
                (#name::#variant_ident as u32)
                    < naclac_lang::error::FRAMEWORK_ERROR_BASE - #offset,
                #message
            );
        }
    });
    quote! {
        const _: () = ::core::assert!(
            #default_offset == naclac_lang::error::USER_ERROR_BASE,
            "naclac-syn's default #[error_code] offset differs from USER_ERROR_BASE"
        );
        const _: () = ::core::assert!(
            #offset >= naclac_lang::error::USER_ERROR_BASE
                && #offset < naclac_lang::error::FRAMEWORK_ERROR_BASE,
            #offset_message
        );
        #(#asserts)*
    }
}

/// Expands the `#[error_code]` / `#[error_code(offset = N)]` macro.
///
/// Implements `From<YourError> for ProgramError` (code `offset + discriminant`)
/// and `Display`.
pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    let offset = match naclac_syn::idl_items::error_code_offset_tokens(attr.into()) {
        Ok(offset) => offset,
        Err(err) => return err.to_compile_error().into(),
    };
    let input = parse_macro_input!(item as ItemEnum);
    let name = &input.ident;
    let idl_build_impl = idl_build_errors_impl(name, &input.variants, offset);
    let range_asserts = user_code_range_asserts(name, &input.variants, offset);

    let expanded = quote! {
        #[cfg_attr(feature = "debug-mode", derive(Debug))]
        #[derive(Copy, Clone, Eq, PartialEq)]
        #input

        #range_asserts

        impl From<#name> for naclac_lang::prelude::ProgramError {
            fn from(e: #name) -> Self {
                naclac_lang::prelude::ProgramError::Custom(e as u32 + #offset)
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
