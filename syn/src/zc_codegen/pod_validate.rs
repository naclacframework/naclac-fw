//! The `PodValidate` impl of a zero-copy struct, used by `defined_type`,
//! `#[account]` and `#[instruction_args]` in the macros and by the generated
//! Rust clients. Handles named-field and tuple structs alike.
//!
//! The impl validates each field from its own stored bytes, found at the
//! field's offset, so it never forms a reference to a packed field and copies
//! nothing. A struct is trivially valid when every field is.

use {
    proc_macro2::{Ident, TokenStream},
    quote::quote,
    syn::Fields,
};

/// The `PodValidate` impl for the struct `ident` with `fields`. `cfg` is
/// prepended to the impl (empty for unconditional). `core` locates
/// `PodValidate` and `PodError`.
pub fn generate(
    ident: &Ident,
    fields: &Fields,
    cfg: &TokenStream,
    core: &syn::Path,
) -> TokenStream {
    let field_tys: Vec<&syn::Type> = fields.iter().map(|f| &f.ty).collect();
    let members: Vec<syn::Member> = fields.members().collect();
    let bytes_param = if field_tys.is_empty() {
        quote! { _ }
    } else {
        quote! { bytes }
    };

    quote! {
        #cfg
        impl #core::PodValidate for #ident {
            const TRIVIAL: bool = true #(&& <#field_tys as #core::PodValidate>::TRIVIAL)*;

            #[inline(always)]
            fn validate_bytes(#bytes_param: &[u8]) -> ::core::result::Result<(), #core::PodError> {
                if Self::TRIVIAL {
                    return Ok(());
                }
                #(
                    <#field_tys as #core::PodValidate>::validate_bytes(
                        &bytes[::core::mem::offset_of!(#ident, #members)..]
                            [..::core::mem::size_of::<#field_tys>()],
                    )?;
                )*
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expansion(source: &str) -> String {
        let item: syn::ItemStruct = syn::parse_str(source).unwrap();
        generate(
            &item.ident,
            &item.fields,
            &TokenStream::new(),
            &syn::parse_quote!(core_path),
        )
        .to_string()
    }

    #[test]
    fn a_named_field_is_validated_at_its_offset() {
        let output = expansion("struct Roster { flag: Bool, scores: FixedVec<u64, 4> }");
        assert!(output.contains("offset_of ! (Roster , flag)"), "{output}");
        assert!(output.contains("offset_of ! (Roster , scores)"), "{output}");
        assert!(
            output.contains("size_of :: < FixedVec < u64 , 4 > >"),
            "{output}"
        );
    }

    #[test]
    fn a_tuple_field_is_validated_by_index() {
        let output = expansion("struct Wrapper(Bool, u8);");
        assert!(output.contains("offset_of ! (Wrapper , 0)"), "{output}");
        assert!(output.contains("offset_of ! (Wrapper , 1)"), "{output}");
    }

    #[test]
    fn a_struct_without_fields_ignores_its_bytes() {
        let output = expansion("struct Empty {}");
        assert!(output.contains("validate_bytes (_ : & [u8])"), "{output}");
        assert!(output.contains("const TRIVIAL : bool = true ;"), "{output}");
    }
}
