//! Compile-time layout checks for a zero-copy struct, used by `defined_type`,
//! `#[account]`, `#[instruction_args]` and plain `#[event]` in the macros and
//! by the generated Rust clients. Handles named-field and tuple structs alike.
//!
//! A zero-copy struct is emitted `#[repr(C, packed)]`: fields in declaration
//! order, no padding, alignment 1, so it can be cast from account or
//! instruction bytes at any address. A generator cannot compute field sizes
//! itself (it runs on syntax, before type resolution), so this emits `const`
//! assertions that the compiler resolves after expansion, once every field's
//! real layout is known:
//!
//! - the struct's size equals the sum of its fields' sizes, so nothing, not
//!   even a future compiler change to `repr(packed)`, adds a hidden gap;
//! - the struct's alignment is 1;
//! - every field is a `ZcField` (naclac-pod's name for `Pod`), which gives a
//!   precise error for a field that is not plain data.

use {
    proc_macro2::{Ident, TokenStream},
    quote::quote,
    syn::Fields,
};

/// The `const` assertions for the struct `ident` with `fields`, to be emitted
/// alongside the struct, not inside it. `cfg` is prepended to every item
/// (empty for unconditional), since one attribute cannot gate several sibling
/// items at once. `core` locates `ZcField`.
pub fn generate(
    ident: &Ident,
    fields: &Fields,
    cfg: &TokenStream,
    core: &syn::Path,
) -> TokenStream {
    let field_tys: Vec<&syn::Type> = fields.iter().map(|f| &f.ty).collect();
    let mut size_terms = field_tys
        .iter()
        .map(|ty| quote! { ::core::mem::size_of::<#ty>() });
    let size_sum = match size_terms.next() {
        None => quote! { 0usize },
        Some(first) => size_terms.fold(first, |sum, term| quote! { #sum + #term }),
    };
    let size_message = format!(
        "defined_type/#[account]: `{ident}` has a different size than its fields add up to; a \
         zero-copy struct is `#[repr(C, packed)]` and must have no padding"
    );
    let align_message = format!(
        "defined_type/#[account]: `{ident}` must have alignment 1; a zero-copy struct is \
         `#[repr(C, packed)]`"
    );

    quote! {
        #cfg
        const _: () = {
            assert!(::core::mem::size_of::<#ident>() == (#size_sum), #size_message);
            assert!(::core::mem::align_of::<#ident>() == 1, #align_message);
        };

        #cfg
        const _: fn() = || {
            fn assert_impl<T: #core::ZcField>() {}
            #(assert_impl::<#field_tys>();)*
        };
    }
}

/// An error spanned at the first `#[repr(..)]` attribute in `attrs`, or `None`
/// when there is none. A zero-copy struct's layout is fixed at
/// `#[repr(C, packed)]`, so another `repr` would either be redundant or
/// conflict with it.
pub fn repr_error(attrs: &[syn::Attribute], owner: &str) -> Option<syn::Error> {
    attrs
        .iter()
        .find(|attr| attr.path().is_ident("repr"))
        .map(|attr| {
            syn::Error::new_spanned(
                attr,
                format!(
                    "Naclac Error: {owner} is stored as `#[repr(C, packed)]` in the zero-copy \
                     representation, which this macro adds itself: remove this `#[repr(..)]`."
                ),
            )
        })
}
