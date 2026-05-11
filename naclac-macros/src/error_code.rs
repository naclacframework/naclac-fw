//! # Error Code Macro Logic
//!
//! Handles the expansion of the `#[error_code]` macro. It transforms an enum of custom errors
//! into native Solana `ProgramError`s, automatically offsetting the discriminants by 6000
//! to conform with standard SBF framework conventions.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemEnum};

/// Expands the `#[error_code]` macro.
///
/// Implements `From<YourError> for ProgramError` and `Display`.
pub fn expand(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemEnum);
    let name = &input.ident;

    let expanded = quote! {
        #[derive(Copy, Clone, Debug, Eq, PartialEq)]
        #input

        // Automatically converts custom errors into native SBF ProgramErrors.
        // Offsets by 6000 (standard framework convention) to avoid collision with native errors.
        impl From<#name> for naclac_lang::prelude::ProgramError {
            fn from(e: #name) -> Self {
                naclac_lang::prelude::ProgramError::Custom(e as u32 + 6000)
            }
        }

        impl naclac_lang::prelude::fmt::Display for #name {
            fn fmt(&self, f: &mut naclac_lang::prelude::fmt::Formatter<'_>) -> naclac_lang::prelude::fmt::Result {
                write!(f, "Naclac Error: {:?}", self)
            }
        }
    };

    TokenStream::from(expanded)
}
