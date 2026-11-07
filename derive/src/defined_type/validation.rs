//! Compile-time rejections for `#[defined_type]`: field types that cannot be
//! stored in a fixed-size value, and enum discriminants the Borsh/Wincode tag
//! cannot encode.

use proc_macro2::TokenStream;

/// The first field-type error in `original_data` (the data as the user wrote
/// it, before any padding or bounds rewriting), if any.
pub(super) fn field_error(original_data: &syn::Data) -> Option<syn::Error> {
    let declared_fields: Vec<&syn::Field> = match original_data {
        syn::Data::Struct(data) => data.fields.iter().collect(),
        syn::Data::Enum(data) => data.variants.iter().flat_map(|v| v.fields.iter()).collect(),
        syn::Data::Union(_) => Vec::new(),
    };
    if let Some(error) = crate::type_classify::zc_view_field_error(
        declared_fields.into_iter(),
        "a #[defined_type]",
        "A #[defined_type] is a fixed-size value stored in accounts and events: use a fixed-size \
         array (e.g. '[u8; 64]'). To group instruction arguments that need ZcVec/ZcString, use \
         #[instruction_args].",
    ) {
        return Some(error);
    }

    let syn::Data::Enum(data) = original_data else {
        return None;
    };
    let aliases = crate::common::crate_source::crate_aliases();
    let variant_fields: Vec<syn::Field> = data
        .variants
        .iter()
        .flat_map(|v| v.fields.iter())
        .map(|field| syn::Field {
            ty: aliases.resolve(&field.ty),
            ..field.clone()
        })
        .collect();
    crate::type_classify::fixed_capacity_field_error(
        variant_fields.iter(),
        "a #[defined_type] enum variant",
        "Put the FixedVec/FixedString in a #[defined_type] struct and use that struct as the \
         variant's field.",
    )
}

/// A compile error, gated to the Borsh and Wincode cfgs, when an enum has a
/// discriminant outside `0..=255`; empty otherwise.
pub(super) fn discriminant_range_conflict(
    ident: &syn::Ident,
    original_data: &syn::Data,
    borsh_cfg: &TokenStream,
    wincode_cfg: &TokenStream,
) -> TokenStream {
    let syn::Data::Enum(data_enum) = original_data else {
        return TokenStream::new();
    };
    let discriminants = naclac_syn::discriminator::variant_discriminants(data_enum.variants.iter());
    if discriminants.iter().all(|d| (0..=255).contains(d)) {
        return TokenStream::new();
    }
    let message = format!(
        "Naclac Error: `#[defined_type]` enum `{ident}` has a discriminant outside \
         0..=255. Borsh and Wincode encode an enum tag as a single unsigned byte; \
         use discriminants in that range, or the zero-copy representation."
    );
    quote::quote! {
        #borsh_cfg
        ::core::compile_error!(#message);
        #wincode_cfg
        ::core::compile_error!(#message);
    }
}
