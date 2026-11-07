//! The `#[cfg(feature = ...)]` attributes that gate each account-data layout's
//! generated items. A proc macro cannot see the calling crate's features, so
//! every layout is generated and the calling crate's compiler keeps one.

use {proc_macro2::TokenStream, quote::quote};

/// Gates an item to the `bytemuck` (zero-copy) layout.
pub(crate) fn zero_copy() -> TokenStream {
    quote! { #[cfg(feature = "bytemuck")] }
}

/// Gates an item to the `borsh` layout.
pub(crate) fn borsh() -> TokenStream {
    quote! { #[cfg(feature = "borsh")] }
}

/// Gates an item to the `wincode` layout.
pub(crate) fn wincode() -> TokenStream {
    quote! { #[cfg(feature = "wincode")] }
}
