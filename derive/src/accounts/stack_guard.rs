//! Compile-time stack-frame guards for the heap layouts: `const` assertions on
//! the real `size_of` of each data-carrying account field and on their sum.

use {
    super::layout::Layout,
    crate::{accounts::field::parser, type_classify},
    proc_macro2::TokenStream,
    quote::quote,
};

/// The size assertions for `layout`, gated by `cfg`. Empty for the zero-copy
/// layout, whose accounts do not heap-allocate.
///
/// Any field other than a thin marker (`Signer`, `Program`, plain `AccountInfo`)
/// is data-carrying. A `Box<Account<T>>` passes trivially, as `size_of::<Box<_>>()`
/// is a pointer, so boxing a field is how a caller clears either check.
pub(super) fn size_guard_consts(
    fields: &[parser::ParsedField],
    layout: Layout,
    cfg: &TokenStream,
) -> TokenStream {
    if layout == Layout::ZeroCopy {
        return quote! {};
    }
    let data_carrying_fields: Vec<(&syn::Ident, &syn::Type)> = fields
        .iter()
        .filter(|f| type_classify::is_account_wrapper(&f.ty))
        .map(|f| (&f.ident, &f.ty))
        .collect();

    let per_field_asserts = data_carrying_fields.iter().map(|(ident, ty)| {
        let msg = format!(
            "naclac: field `{}` is large for a single stack frame — wrap it as `Box<{}>` to keep `load_and_validate` under the SBF 4KB stack limit",
            ident,
            quote! { #ty }.to_string().replace(" ", "")
        );
        quote! {
            #cfg
            const _: () = assert!(core::mem::size_of::<#ty>() <= 300, #msg);
        }
    });

    let aggregate_assert = if data_carrying_fields.is_empty() {
        quote! {}
    } else {
        let field_list = data_carrying_fields
            .iter()
            .map(|(ident, _)| ident.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let sizes = data_carrying_fields
            .iter()
            .map(|(_, ty)| quote! { core::mem::size_of::<#ty>() });
        let msg = format!(
            "naclac: this Accounts struct's data-carrying fields ({}) sum to more than the safe stack budget for `load_and_validate` — box one or more of them as `Box<Account<T>>`",
            field_list
        );
        quote! {
            #cfg
            const _: () = assert!(0usize #( + #sizes )* <= 1700, #msg);
        }
    };

    quote! {
        #(#per_field_asserts)*
        #aggregate_assert
    }
}
