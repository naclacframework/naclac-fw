//! The zero-copy (`bytemuck`) representation of a `#[defined_type]`: a
//! `#[repr(C, packed)]` `Pod` struct, or an enum stored as its tag followed by
//! the active variant's fields through a generated `ZcCodec`.

use proc_macro2::TokenStream;

use crate::{common::derive_attrs::derive_attr_listing, pod_struct_checks, zc_enum};

/// The compile error for a user-written `#[derive(Default)]` on a struct that
/// gets a generated `Default` in the zero-copy representation, or an empty
/// stream when there is no conflict, gated by `cfg`.
///
/// A struct with a `Vec`/`String` field is never `Zeroable`, so it gets no
/// generated `Default` under any representation and the user's derive stays
/// valid.
pub(super) fn default_derive_conflict(
    input: &syn::DeriveInput,
    original_data: &syn::Data,
    cfg: &TokenStream,
) -> TokenStream {
    let syn::Data::Struct(data_struct) = original_data else {
        return TokenStream::new();
    };
    let has_heap_field = data_struct.fields.iter().any(|f| {
        matches!(
            crate::type_classify::base_ident(&f.ty)
                .as_ref()
                .map(syn::Ident::to_string)
                .as_deref(),
            Some("Vec") | Some("String")
        )
    });
    if has_heap_field {
        return TokenStream::new();
    }
    let Some(attr) = derive_attr_listing(&input.attrs, "Default") else {
        return TokenStream::new();
    };
    let error = syn::Error::new_spanned(
        attr,
        "Naclac Error: remove this `#[derive(Default)]`; in the zero-copy layout \
         `#[defined_type]` already generates `Default`.",
    )
    .to_compile_error();
    quote::quote! { #cfg #error }
}

/// Adds the `repr` and `Clone`/`Copy` derives the zero-copy representation
/// needs to `input`, then returns the item and its impls, gated by `cfg`.
///
/// `input.attrs` is left holding the final attributes, including the `repr`
/// added here, which is the enum's real compiled discriminant width and what
/// the `idl-build` output must record.
pub(super) fn variant(input: &mut syn::DeriveInput, cfg: &TokenStream) -> TokenStream {
    let ident = input.ident.clone();
    let generics = input.generics.clone();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let mut repr_conflict = TokenStream::new();
    match &input.data {
        syn::Data::Enum(_) => {
            if naclac_syn::discriminator::detect_enum_repr(&input.attrs).is_none() {
                input.attrs.push(syn::parse_quote!(#[repr(u8)]));
            }
        }
        syn::Data::Struct(_) => {
            match pod_struct_checks::repr_error(
                &input.attrs,
                "a zero-copy `#[defined_type]` struct",
            ) {
                Some(error) => repr_conflict = error.to_compile_error(),
                None => input.attrs.push(syn::parse_quote!(#[repr(C, packed)])),
            }
        }
        syn::Data::Union(_) => {}
    }

    // `Pod` and `ZcEnumKind` require `Copy`. Add only whichever of `Clone`/`Copy`
    // is missing: deriving one that is already present is a duplicate-derive error.
    let has_clone = derive_attr_listing(&input.attrs, "Clone").is_some();
    let has_copy = derive_attr_listing(&input.attrs, "Copy").is_some();
    if has_clone && !has_copy {
        input.attrs.push(syn::parse_quote!(#[derive(Copy)]));
    } else if has_copy && !has_clone {
        input.attrs.push(syn::parse_quote!(#[derive(Clone)]));
    } else if !has_clone && !has_copy {
        input.attrs.push(syn::parse_quote!(#[derive(Clone, Copy)]));
    }

    match &input.data {
        syn::Data::Enum(data_enum) => {
            let tag_ty: syn::Type = {
                let named = naclac_syn::discriminator::detect_enum_repr(&input.attrs);
                match syn::parse_str(named.as_deref().unwrap_or("u8")) {
                    Ok(tag_ty) => tag_ty,
                    Err(error) => return error.to_compile_error(),
                }
            };
            let codec_items = zc_enum::generate(&ident, &data_enum.variants, &tag_ty, cfg);
            quote::quote! {
                #cfg
                #input

                #codec_items

                #cfg
                impl #impl_generics naclac_lang::prelude::NaclacPod for #ident #ty_generics #where_clause {
                    #[inline(always)]
                    fn naclac_from_bytes(data: &[u8]) -> Self {
                        <Self as naclac_lang::prelude::ZcCodec>::decode(data)
                            .unwrap_or_else(|| panic!("defined_type: invalid enum discriminant in account/instruction bytes"))
                    }
                    #[inline(always)]
                    fn naclac_try_from_bytes(data: &[u8]) -> Option<Self> {
                        <Self as naclac_lang::prelude::ZcCodec>::decode(data)
                    }
                    #[inline(always)]
                    fn naclac_size() -> usize {
                        <Self as naclac_lang::prelude::ZcCodec>::SIZE
                    }
                }
            }
        }
        syn::Data::Struct(data_struct) => {
            let pod_checks = pod_struct_checks::generate(&ident, &data_struct.fields, cfg);
            quote::quote! {
                #cfg
                #repr_conflict

                #cfg
                #input

                #pod_checks

                // SAFETY: sound only because the type is `#[repr(C, packed)]` and
                // `Copy` (both added above) and because `pod_checks` (above)
                // verifies at compile time that the struct has no padding,
                // alignment 1, and every declared field is itself Pod.
                #cfg
                unsafe impl #impl_generics naclac_lang::prelude::Pod for #ident #ty_generics #where_clause {}
                // SAFETY: Zeroed memory is a valid initial state for Pod types.
                #cfg
                unsafe impl #impl_generics naclac_lang::prelude::Zeroable for #ident #ty_generics #where_clause {}

                // `Zeroable`, not `#[derive(Default)]`: `std`'s derive only covers
                // arrays up to length 32.
                #cfg
                impl #impl_generics core::default::Default for #ident #ty_generics #where_clause {
                    fn default() -> Self {
                        naclac_lang::prelude::bytemuck::Zeroable::zeroed()
                    }
                }

                #cfg
                impl #impl_generics naclac_lang::prelude::ZcCodec for #ident #ty_generics #where_clause {
                    const SIZE: usize = core::mem::size_of::<Self>();

                    fn encode(&self, out: &mut [u8]) {
                        naclac_lang::prelude::encode_pod(self, out);
                    }

                    fn decode(bytes: &[u8]) -> Option<Self> {
                        naclac_lang::prelude::decode_pod(bytes)
                    }
                }

                #cfg
                impl #impl_generics naclac_lang::prelude::NaclacPod for #ident #ty_generics #where_clause {
                    #[inline(always)]
                    fn naclac_from_bytes(data: &[u8]) -> Self {
                        naclac_lang::prelude::bytemuck::pod_read_unaligned(
                            &data[..core::mem::size_of::<Self>()],
                        )
                    }
                    #[inline(always)]
                    fn naclac_size() -> usize {
                        core::mem::size_of::<Self>()
                    }
                }
            }
        }
        syn::Data::Union(_) => TokenStream::new(),
    }
}
