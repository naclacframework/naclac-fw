//! # Component Macro Logic
//!
//! Handles the expansion of the `#[component]` attribute, transforming Rust structs into
//! on-chain data accounts. It automatically generates the 8-byte discriminator and handles
//! the serialization/deserialization traits based on the active execution mode.

use proc_macro::TokenStream;
use quote::quote;
use sha2::{Digest, Sha256};
use syn::{parse_macro_input, ItemStruct};

/// Expands the `#[component]` macro.
///
/// This generates the structural foundation for an on-chain account.
/// - **Zero-Copy Mode**: Forces `#[repr(C)]`, derives `Pod`/`Zeroable`, and replaces `Vec`/`String`
///   with their memory-mapped equivalents (`Span`/`ZcString`).
/// - **Borsh Mode**: Derives `BorshSerialize` and `BorshDeserialize` for standard heap-allocated SBF execution.
pub fn expand(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(item as ItemStruct);
    let struct_name = &ast.ident;
    let vis = &ast.vis;
    let fields = &ast.fields;

    // Validate that no fields use the Pubkey type
    for field in fields.iter() {
        let ty = &field.ty;
        let ty_str = quote! { #ty }.to_string().replace(" ", "");
        if ty_str.contains("Pubkey") {
            return syn::Error::new_spanned(
                ty,
                "Naclac Error: 'Pubkey' has been deprecated in favor of 'Address' in Solana v3. Please replace it with 'Address'."
            )
            .to_compile_error()
            .into();
        }
    }

    // Zero-copy vs Borsh is determined automatically: pinocchio is always
    // zero-copy, and otherwise the presence of the `borsh` feature is a
    // complete signal (there is no third representation for account data).
    // Any legacy `#[component(zero_copy)]`-style argument is silently
    // ignored rather than rejected — it has no effect on codegen either way.

    let is_zero_copy =
        crate::caller_has_feature("pinocchio") || !crate::caller_has_feature("borsh");

    // 2. Compute the 8-byte Anchor-standard hash ONCE during compilation
    let struct_str = struct_name.to_string();
    let preimage = format!("account:{}", struct_str);
    let mut hasher = Sha256::new();
    hasher.update(preimage.as_bytes());
    let disc: [u8; 8] = hasher.finalize()[..8].try_into().unwrap();

    let b0 = disc[0];
    let b1 = disc[1];
    let b2 = disc[2];
    let b3 = disc[3];
    let b4 = disc[4];
    let b5 = disc[5];
    let b6 = disc[6];
    let b7 = disc[7];

    // 3. Prepare cleaned fields (remove #[max_len]).
    //
    // NOTE: zero-copy component fields must NOT be silently rewritten from
    // `Vec<T>`/`String` to `Span<T>`/`ZcString` here. Unlike an instruction
    // argument (parsed fresh from the current instruction's byte buffer, used
    // and discarded within that same call), a `#[component]` field is *persisted*
    // account data — it gets round-tripped through `unsafe impl Pod`/raw byte
    // casting across separate transactions. `Span<T>` is a raw pointer + length;
    // a pointer value written into on-chain bytes in one transaction is
    // meaningless (or attacker-controlled) when read back in a later one —
    // dereferencing it then is a real, exploitable memory-safety bug, not just
    // a missed optimization. There is no sound way to make this rewrite work,
    // so it's rejected below instead of silently generating unsound code.
    let mut cleaned_ast = ast.clone();
    if let syn::Fields::Named(fields_named) = &mut cleaned_ast.fields {
        for field in &mut fields_named.named {
            field.attrs.retain(|attr| !attr.path().is_ident("max_len"));

            if is_zero_copy {
                let ty = &field.ty;
                let ty_str = quote! { #ty }.to_string().replace(" ", "");
                if ty_str.contains("Vec<") || ty_str == "String" {
                    let field_name = field
                        .ident
                        .as_ref()
                        .map(|i| i.to_string())
                        .unwrap_or_default();
                    return syn::Error::new_spanned(
                        ty,
                        format!(
                            "Naclac Error: field '{}' uses a heap-allocated type ('Vec'/'String') in a zero-copy \
                             #[component]. Persisted zero-copy account data is read via raw byte-casting, and a \
                             heap pointer cannot survive being written to an account and read back in a later \
                             transaction. Please use a fixed-size array (e.g. '[u8; 64]') instead.",
                            field_name
                        )
                    ).to_compile_error().into();
                }
            }
        }
    }
    let cleaned_fields = &cleaned_ast.fields;

    let expanded = if is_zero_copy {
        quote! {
            #[repr(C)]
            #vis struct #struct_name #cleaned_fields

            impl core::clone::Clone for #struct_name {
                #[inline(always)]
                fn clone(&self) -> Self { *self }
            }
            impl core::marker::Copy for #struct_name {}

            unsafe impl naclac_lang::prelude::bytemuck::Pod for #struct_name {}
            unsafe impl naclac_lang::prelude::bytemuck::Zeroable for #struct_name {}

            impl #struct_name {
                pub const DISCRIMINATOR: [u8; 8] = [#b0, #b1, #b2, #b3, #b4, #b5, #b6, #b7];
                pub const SPACE: usize = core::mem::size_of::<Self>() + 8;

                #[cfg(not(target_os = "solana"))]
                #[inline(always)]
                pub fn load(data: &[u8]) -> naclac_lang::prelude::Result<&Self> {
                    if data.len() < Self::SPACE {
                        return Err(naclac_lang::prelude::NaclacError::AccountDataTooSmall.err(0));
                    }
                    if data[0..8] != Self::DISCRIMINATOR {
                        return Err(naclac_lang::prelude::NaclacError::AccountNotInitialized.err(0));
                    }
                    Ok(naclac_lang::prelude::bytemuck::from_bytes(&data[8..Self::SPACE]))
                }

                #[cfg(not(target_os = "solana"))]
                #[inline(always)]
                pub fn load_mut(data: &mut [u8]) -> naclac_lang::prelude::Result<&mut Self> {
                    if data.len() < Self::SPACE {
                        return Err(naclac_lang::prelude::NaclacError::AccountDataTooSmall.err(0));
                    }
                    if data[0..8] != Self::DISCRIMINATOR {
                        return Err(naclac_lang::prelude::NaclacError::AccountNotInitialized.err(0));
                    }
                    Ok(naclac_lang::prelude::bytemuck::from_bytes_mut(&mut data[8..Self::SPACE]))
                }
            }

            impl naclac_lang::prelude::NaclacZeroCopy for #struct_name {}

            impl naclac_lang::prelude::Discriminator for #struct_name {
                const DISCRIMINATOR: [u8; 8] = [#b0, #b1, #b2, #b3, #b4, #b5, #b6, #b7];
            }
        }
    } else {
        // --- Borsh Engine Generation ---
        // Emits dynamic heap-allocated serialization logic for standard Solana environments.
        let field_sizes: Vec<proc_macro2::TokenStream> = fields
            .iter()
            .map(|f| {
                let ty = &f.ty;
                let mut max_len = None;

                for attr in &f.attrs {
                    if attr.path().is_ident("max_len") {
                        if let Ok(lit) = attr.parse_args::<syn::LitInt>() {
                            max_len = Some(lit.base10_parse::<usize>().unwrap());
                        }
                    }
                }

                if let Some(len) = max_len {
                    quote! { (4 + #len) }
                } else {
                    let type_str = quote! { #ty }.to_string().replace(" ", "");
                    match type_str.as_str() {
                        "u8" | "i8" | "bool" => quote! { 1 },
                        "u16" | "i16" => quote! { 2 },
                        "u32" | "i32" | "f32" => quote! { 4 },
                        "u64" | "i64" | "f64" => quote! { 8 },
                        "u128" | "i128" => quote! { 16 },
                        "Address" | "naclac_lang::prelude::Address" => quote! { 32 },
                        _ => quote! { core::mem::size_of::<#ty>() },
                    }
                }
            })
            .collect();

        quote! {
            #[cfg(not(feature = "pinocchio"))]
            #[derive(Clone, naclac_lang::prelude::BorshSerialize, naclac_lang::prelude::BorshDeserialize)]
            #[cfg_attr(not(feature = "pinocchio"), borsh(crate = "naclac_lang::prelude::borsh"))]
            #vis struct #struct_name #cleaned_fields

            #[cfg(not(feature = "pinocchio"))]
            impl #struct_name {
                pub const DISCRIMINATOR: [u8; 8] = [#b0, #b1, #b2, #b3, #b4, #b5, #b6, #b7];
                pub const SPACE: usize = 8 + #(#field_sizes)+*;

                pub fn try_deserialize(data: &mut &[u8]) -> naclac_lang::prelude::Result<Self> {
                    if data.len() < 8 {
                        return Err(naclac_lang::prelude::NaclacError::AccountDataTooSmall.err(0));
                    }
                    if data[0..8] != Self::DISCRIMINATOR {
                        return Err(naclac_lang::prelude::NaclacError::AccountNotInitialized.err(0));
                    }
                    let mut reader = &data[8..];
                    naclac_lang::prelude::BorshDeserialize::deserialize(&mut reader)
                        .map_err(|_| naclac_lang::prelude::NaclacError::DeserializationFailed.err(0))
                }

                pub fn try_serialize<W: naclac_lang::prelude::borsh::io::Write>(&self, writer: &mut W) -> naclac_lang::prelude::Result<()> {
                    writer.write_all(&Self::DISCRIMINATOR)
                        .map_err(|_| naclac_lang::prelude::NaclacError::SerializationFailed.err(0))?;
                    naclac_lang::prelude::BorshSerialize::serialize(self, writer)
                        .map_err(|_| naclac_lang::prelude::NaclacError::SerializationFailed.err(0))
                }
            }

            #[cfg(not(feature = "pinocchio"))]
            impl naclac_lang::prelude::Discriminator for #struct_name {
                const DISCRIMINATOR: [u8; 8] = [#b0, #b1, #b2, #b3, #b4, #b5, #b6, #b7];
            }
        }
    };

    TokenStream::from(expanded)
}
