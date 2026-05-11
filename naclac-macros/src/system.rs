//! # System Macro Logic
//!
//! Handles the expansion of the `#[system]` macro. Currently used as an optional
//! wrapper for isolated logic blocks, injecting automatic compute unit profiling
//! when the `debug-mode` feature is enabled.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

/// Expands the `#[system]` macro.
///
/// Wraps a function to automatically log compute unit usage before and after execution
/// when compiling with the `debug-mode` feature. Supports both standard Solana and Pinocchio.
pub fn expand(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = &input_fn.sig.ident;
    let fn_vis = &input_fn.vis;
    let fn_block = &input_fn.block;
    let fn_inputs = &input_fn.sig.inputs;
    let fn_output = &input_fn.sig.output;

    let expanded = quote! {
        #fn_vis fn #fn_name(#fn_inputs) #fn_output {
            #[cfg(all(feature = "debug-mode", not(feature = "pinocchio")))]
            naclac_lang::solana_program::log::sol_log_compute_units();
            #[cfg(all(feature = "debug-mode", feature = "pinocchio"))]
            naclac_lang::prelude::pinocchio_log::sol_log_compute_units();

            let __result = { #fn_block };

            #[cfg(all(feature = "debug-mode", not(feature = "pinocchio")))]
            naclac_lang::solana_program::log::sol_log_compute_units();
            #[cfg(all(feature = "debug-mode", feature = "pinocchio"))]
            naclac_lang::prelude::pinocchio_log::sol_log_compute_units();

            __result
        }
    };

    TokenStream::from(expanded)
}
