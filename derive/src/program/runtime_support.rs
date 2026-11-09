//! Program-level support code: the panic support every program carries.

use quote::quote;

/// The program's panic support for the Solana target. A program crate is
/// `#![no_std]`, so `naclac_lang::__naclac_nostd_panic!` supplies it: the
/// program's `#[panic_handler]`, or only the `custom_panic` hook when a
/// dependency (the SPL crates behind the `token` feature) already links `std`
/// and with it a handler. Omitted under `no-entrypoint`, where the program that
/// links this one supplies it.
pub(super) fn panic_support_tokens() -> proc_macro2::TokenStream {
    quote! {
        #[cfg(not(feature = "no-entrypoint"))]
        naclac_lang::__naclac_nostd_panic!();
    }
}
