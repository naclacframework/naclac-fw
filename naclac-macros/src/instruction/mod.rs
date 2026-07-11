//! # Instruction Macro Module
//!
//! Houses the expansion logic for the `#[instruction]` macro. This macro rewrites
//! instruction function signatures to properly enforce Zero-Copy `Hydrated` accounts
//! and manages the generation of security checks, CPI initializers, and account reallocation.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

pub mod close_account;
pub mod init_cpi;
pub mod parser;
pub mod realloc;
pub mod security;

/// This macro rewrites the developer's instruction function signature, automatically converting
/// `Context<T>` to `Context<THydrated>`. This ensures that all accounts within
/// the instruction body are safely type-guarded.
pub fn expand(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Zero-copy vs Borsh mode is auto-detected from the `borsh` feature; any
    // legacy `#[instruction(zero_copy)]`-style argument is silently ignored.
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_vis = &input_fn.vis;
    let fn_block = &input_fn.block;
    let fn_attrs = &input_fn.attrs;

    let fn_sig = &input_fn.sig;

    let expanded = quote! {
        #(#fn_attrs)*
        #fn_vis #fn_sig {
            #fn_block
        }
    };

    TokenStream::from(expanded)
}
