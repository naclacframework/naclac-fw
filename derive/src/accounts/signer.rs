//! Whether an Accounts field's account must be a signer, as a `bool`
//! expression the generated code evaluates.

use {crate::accounts::field::parser, quote::quote};

/// True if `ty` names a lifetime, type or const parameter of `generics`. Such a
/// type cannot be written inside a `const` item nested in a function.
fn type_mentions_generic(ty: &syn::Type, generics: &syn::Generics) -> bool {
    let names: Vec<String> = generics
        .params
        .iter()
        .map(|param| match param {
            syn::GenericParam::Lifetime(lifetime) => lifetime.lifetime.ident.to_string(),
            syn::GenericParam::Type(type_param) => type_param.ident.to_string(),
            syn::GenericParam::Const(const_param) => const_param.ident.to_string(),
        })
        .collect();
    if names.is_empty() {
        return false;
    }
    fn walk(stream: proc_macro2::TokenStream, names: &[String]) -> bool {
        stream.into_iter().any(|tree| match tree {
            proc_macro2::TokenTree::Ident(ident) => names.contains(&ident.to_string()),
            proc_macro2::TokenTree::Group(group) => walk(group.stream(), names),
            _ => false,
        })
    }
    walk(quote!(#ty), &names)
}

/// The `bool` expression for whether `field`'s account must be a signer: an
/// explicit `#[account(signer)]`, or its type's `NaclacAccount::IS_SIGNER`
/// (`field.ty` is already the inner type of an `Option<T>` field).
/// `in_function_const` is true where the expression sits in a `const` item
/// inside a function, which cannot name the struct's generic parameters; a
/// field type that mentions one then contributes `false`.
pub(super) fn signer_flag(
    field: &parser::ParsedField,
    generics: &syn::Generics,
    in_function_const: bool,
) -> proc_macro2::TokenStream {
    if field.is_signer {
        return quote! { true };
    }
    if in_function_const && type_mentions_generic(&field.ty, generics) {
        return quote! { false };
    }
    let ty = &field.ty;
    quote! { <#ty as naclac_lang::prelude::NaclacAccount>::IS_SIGNER }
}
