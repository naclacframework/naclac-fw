//! Compile-time stack-frame guards for the heap layouts: `const` assertions on
//! the real `size_of` of each data-carrying account field and on their sum.

use {
    super::layout::Layout, crate::accounts::field::parser, proc_macro2::TokenStream, quote::quote,
};

/// The size assertions for `layout`, gated by `cfg`. Empty for the zero-copy
/// layout, whose accounts do not heap-allocate.
///
/// A field counts when its type's `NaclacAccount::CARRIES_DATA` is true, so the
/// compiler decides it for any spelling of the type, an alias included. A
/// `Box<Account<T>>` passes trivially, as `size_of::<Box<_>>()` is a pointer,
/// so boxing a field is how a caller clears either check.
pub(super) fn size_guard_consts(
    fields: &[parser::ParsedField],
    layout: Layout,
    cfg: &TokenStream,
) -> TokenStream {
    if layout == Layout::ZeroCopy || fields.is_empty() {
        return quote! {};
    }

    let per_field_asserts = fields.iter().map(|field| {
        let ident = &field.ident;
        let ty = &field.ty;
        let msg = format!(
            "Naclac Error: field `{}` is too large for one stack frame; wrap it as `Box<{}>` to \
             stay under the SBF 4KB stack limit.",
            ident,
            quote! { #ty }.to_string().replace(" ", "")
        );
        quote! {
            #cfg
            const _: () = assert!(
                !<#ty as naclac_lang::prelude::NaclacAccount>::CARRIES_DATA
                    || core::mem::size_of::<#ty>() <= 300,
                #msg
            );
        }
    });

    let sizes = fields.iter().map(|field| {
        let ty = &field.ty;
        quote! {
            (if <#ty as naclac_lang::prelude::NaclacAccount>::CARRIES_DATA {
                core::mem::size_of::<#ty>()
            } else {
                0
            })
        }
    });
    let aggregate_msg = "Naclac Error: this Accounts struct's account fields together exceed the \
                         stack budget for `load_and_validate`; box some as `Box<Account<T>>`.";

    quote! {
        #(#per_field_asserts)*
        #cfg
        const _: () = assert!(0usize #( + #sizes )* <= 1700, #aggregate_msg);
    }
}
