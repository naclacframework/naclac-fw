//! # Instruction Macro Module
//!
//! Houses the expansion logic for the `#[instruction]` macro, and the submodules that
//! generate an instruction's security checks, CPI initializers, and account reallocation.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

pub mod close_account;
pub mod init_cpi;
pub mod parser;
pub mod realloc;
pub mod security;

/// Re-emits the annotated function unchanged. Type-guarding of accounts within
/// the instruction body is already enforced by `#[derive(Accounts)]`'s generated
/// `load_and_validate`, not by any rewrite this macro performs.
pub fn expand(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Zero-copy vs Borsh mode is auto-detected from the `borsh` feature.
    // `#[instruction]` takes no argument at all — enforced by `lib.rs`'s
    // `reject_nonempty_attr` before this function is ever called.
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
