//! Generates the packed stored form of a `#[defined_type]` enum in its
//! zero-copy representation: `ZcCodec` (`SIZE`, `encode`, `decode`) and
//! `ZcEnumKind`, so the enum can be stored as `ZcEnum<E>` in a zero-copy
//! struct.
//!
//! The stored bytes are the tag (the enum's `repr` integer, little-endian),
//! then the active variant's fields one after another in declaration order
//! with no padding, then zeros up to the size of the largest variant. `SIZE`
//! is the tag plus the largest variant's fields. Every field type must itself
//! implement `ZcCodec`, which is how a variant field that is another
//! `#[defined_type]` enum or struct recurses without any type being named
//! here.
//!
//! `decode` returns `None` for an unknown tag, a slice too short for the
//! variant the tag names, or any invalid nested value; it never reads past the
//! slice. `encode` writes only inside the slice it is given.

use {
    proc_macro2::{Ident, Literal, TokenStream},
    quote::{format_ident, quote},
    syn::{Fields, Type, Variant},
};

/// The field types of `variant`, in order.
fn field_types(variant: &Variant) -> Vec<&Type> {
    variant.fields.iter().map(|field| &field.ty).collect()
}

/// `SIZE_0 + SIZE_1 + ...` over the first `count` field types (`0usize` when
/// there are none): the offset of field `count` inside the variant's payload.
fn offset_expr(field_tys: &[&Type], count: usize, core: &syn::Path) -> TokenStream {
    let mut sizes = field_tys[..count]
        .iter()
        .map(|ty| quote! { <#ty as #core::ZcCodec>::SIZE });
    match sizes.next() {
        None => quote! { 0usize },
        Some(first) => sizes.fold(first, |sum, size| quote! { #sum + #size }),
    }
}

/// Returns the `ZcCodec` and `ZcEnumKind` impls for `ident`, whose tag type is
/// `tag_ty`. `cfg` is prepended to each item, since one attribute cannot gate
/// several sibling items at once. `core` locates `ZcCodec`, `ZcEnumKind` and
/// `ZcStorage`.
pub fn generate(
    ident: &Ident,
    variants: &syn::punctuated::Punctuated<Variant, syn::Token![,]>,
    tag_ty: &Type,
    cfg: &TokenStream,
    core: &syn::Path,
) -> TokenStream {
    let discriminants = crate::discriminator::variant_discriminants(variants.iter());

    let mut payload_sizes = Vec::new();
    let mut encode_arms = Vec::new();
    let mut decode_arms = Vec::new();

    for (variant, discriminant) in variants.iter().zip(discriminants.iter()) {
        let variant_ident = &variant.ident;
        let tag = Literal::i128_unsuffixed(*discriminant);
        let field_tys = field_types(variant);
        let bindings: Vec<Ident> = (0..field_tys.len())
            .map(|index| format_ident!("__field{}", index))
            .collect();
        let offsets: Vec<TokenStream> = (0..field_tys.len())
            .map(|index| offset_expr(&field_tys, index, core))
            .collect();

        payload_sizes.push(offset_expr(&field_tys, field_tys.len(), core));

        let pattern = match &variant.fields {
            Fields::Unit => quote! { Self::#variant_ident },
            Fields::Unnamed(_) => quote! { Self::#variant_ident ( #(#bindings),* ) },
            Fields::Named(named) => {
                let names = named.named.iter().filter_map(|field| field.ident.as_ref());
                quote! { Self::#variant_ident { #(#names: #bindings),* } }
            }
        };
        let construct = match &variant.fields {
            Fields::Unit => quote! { Self::#variant_ident },
            Fields::Unnamed(_) => quote! { Self::#variant_ident ( #(#bindings),* ) },
            Fields::Named(named) => {
                let names = named.named.iter().filter_map(|field| field.ident.as_ref());
                quote! { Self::#variant_ident { #(#names: #bindings),* } }
            }
        };

        encode_arms.push(quote! {
            #pattern => {
                let tag_value: #tag_ty = #tag;
                <#tag_ty as #core::ZcCodec>::encode(&tag_value, tag_slot);
                #(
                    let start = #offsets;
                    if let Some(slot) = payload.get_mut(
                        start..start + <#field_tys as #core::ZcCodec>::SIZE,
                    ) {
                        <#field_tys as #core::ZcCodec>::encode(#bindings, slot);
                    }
                )*
            }
        });

        decode_arms.push(quote! {
            #tag => {
                #(
                    let start = #offsets;
                    let #bindings = <#field_tys as #core::ZcCodec>::decode(
                        payload.get(
                            start..start + <#field_tys as #core::ZcCodec>::SIZE,
                        )?,
                    )?;
                )*
                Some(#construct)
            }
        });
    }

    quote! {
        #cfg
        impl #core::ZcCodec for #ident {
            const SIZE: usize = {
                let mut payload = 0usize;
                #(
                    {
                        let size = #payload_sizes;
                        if size > payload {
                            payload = size;
                        }
                    }
                )*
                core::mem::size_of::<#tag_ty>() + payload
            };

            fn encode(&self, out: &mut [u8]) {
                let Some((tag_slot, payload)) = out.split_at_mut_checked(core::mem::size_of::<#tag_ty>())
                else {
                    return;
                };
                let _ = &payload;
                match self {
                    #(#encode_arms)*
                }
            }

            fn decode(bytes: &[u8]) -> Option<Self> {
                let tag_size = core::mem::size_of::<#tag_ty>();
                let tag = <#tag_ty as #core::ZcCodec>::decode(bytes.get(..tag_size)?)?;
                let payload = bytes.get(tag_size..)?;
                let _ = payload;
                match tag {
                    #(#decode_arms)*
                    _ => None,
                }
            }
        }

        #cfg
        impl #core::ZcEnumKind for #ident {
            type Stored = #core::ZcStorage<
                #ident,
                { <#ident as #core::ZcCodec>::SIZE },
            >;
        }
    }
}
