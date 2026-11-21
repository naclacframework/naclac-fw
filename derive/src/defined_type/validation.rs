//! Compile-time rejections for `#[defined_type]`: field types that cannot be
//! stored in a fixed-size value, and enums whose repr or discriminants the
//! one-byte Borsh/Wincode tag cannot encode.

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
        "Use a fixed-size array like [u8; 64], or #[instruction_args] for arguments.",
    ) {
        return Some(error);
    }

    let syn::Data::Enum(data) = original_data else {
        return None;
    };
    crate::type_classify::fixed_capacity_field_error(
        data.variants.iter().flat_map(|v| v.fields.iter()),
        "a #[defined_type] enum variant",
        "Wrap it in a #[defined_type] struct.",
    )
}

/// A compile error, gated to the Borsh and Wincode cfgs, when an enum declares
/// an integer repr other than `u8` or has a discriminant outside `0..=255`;
/// empty otherwise. Both layouts store the tag in one byte, and the IDL records
/// the declared repr, so any other repr would describe a tag the bytes lack.
pub(super) fn one_byte_tag_conflict(
    ident: &syn::Ident,
    attrs: &[syn::Attribute],
    original_data: &syn::Data,
    borsh_cfg: &TokenStream,
    wincode_cfg: &TokenStream,
) -> TokenStream {
    let syn::Data::Enum(data_enum) = original_data else {
        return TokenStream::new();
    };
    let repr = naclac_syn::discriminator::detect_enum_repr(attrs);
    let discriminants = naclac_syn::discriminator::variant_discriminants(data_enum.variants.iter());
    let message = match repr.as_deref() {
        Some(repr) if repr != "u8" => format!(
            "Naclac Error: enum `{ident}` has #[repr({repr})], but Borsh and Wincode store the tag \
             in one byte. Use #[repr(u8)] or no repr, or the zero-copy layout."
        ),
        _ if discriminants.iter().all(|d| (0..=255).contains(d)) => return TokenStream::new(),
        _ => format!(
            "Naclac Error: `#[defined_type]` enum `{ident}` has a discriminant outside 0..=255, \
             which Borsh and Wincode can't encode as a one-byte tag. Use values in 0..=255."
        ),
    };
    quote::quote! {
        #borsh_cfg
        ::core::compile_error!(#message);
        #wincode_cfg
        ::core::compile_error!(#message);
    }
}

#[cfg(test)]
mod tests {
    use {super::one_byte_tag_conflict, crate::common::cfg_gates};

    fn conflict(source: &str) -> String {
        let input: syn::DeriveInput = syn::parse_str(source).unwrap();
        one_byte_tag_conflict(
            &input.ident,
            &input.attrs,
            &input.data,
            &cfg_gates::borsh(),
            &cfg_gates::wincode(),
        )
        .to_string()
    }

    #[test]
    fn a_wider_repr_is_rejected_for_borsh_and_wincode() {
        for repr in ["u16", "u32", "i8"] {
            let output = conflict(&format!(
                "#[repr({repr})] enum Level {{ Low = 1, Mid = 2 }}"
            ));
            assert!(output.contains(&format!("#[repr({repr})]")), "{output}");
            assert!(
                output.contains("\"borsh\"") && output.contains("\"wincode\""),
                "{output}"
            );
            assert!(!output.contains("\"bytemuck\""), "{output}");
        }
    }

    #[test]
    fn a_u8_repr_or_no_repr_with_one_byte_values_is_accepted() {
        assert!(conflict("#[repr(u8)] enum Color { Red, Blue = 7 }").is_empty());
        assert!(conflict("enum Color { Red, Green, Blue = 255 }").is_empty());
    }

    #[test]
    fn a_discriminant_outside_one_byte_is_rejected() {
        assert!(conflict("enum Big { A = 300 }").contains("outside 0..=255"));
        assert!(conflict("enum Negative { A = -1 }").contains("outside 0..=255"));
    }

    #[test]
    fn structs_are_never_rejected() {
        assert!(conflict("struct Limits { min: u64 }").is_empty());
    }
}
