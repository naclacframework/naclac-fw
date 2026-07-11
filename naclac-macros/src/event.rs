//! # Event Macro Logic
//!
//! Handles the expansion of the `#[event]` macro, generating efficient event logging logic.
//! It computes the 8-byte discriminator at compile time and emits events using either
//! highly optimized zero-copy slicing (`sol_log_data`) or standard Borsh serialization.

use proc_macro::TokenStream;
use quote::quote;
use sha2::{Digest, Sha256};
use syn::{parse_macro_input, ItemStruct};

/// Expands the `#[event]` macro.
///
/// Converts a standard Rust struct into an on-chain event emitter.
/// - **Zero-Copy Mode**: Emits events by casting the struct to bytes using `bytemuck` and calling
///   the raw runtime logging syscall (`sol_log_data`) with the pre-computed discriminator.
/// - **Borsh Mode**: Serializes the struct dynamically using Borsh and passes it to the
///   event emitter.
pub fn expand(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(item as ItemStruct);
    let struct_name = &ast.ident;
    let vis = &ast.vis;

    // Validate that no fields use the Pubkey type
    for field in ast.fields.iter() {
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

    // Zero-copy vs Borsh mode is auto-detected from the `borsh` feature; any
    // legacy `#[event(zero_copy)]`-style argument is silently ignored.

    let discriminator_preimage = format!("event:{}", struct_name);
    let mut hasher = Sha256::new();
    hasher.update(discriminator_preimage.as_bytes());
    let result = hasher.finalize();

    let b0 = result[0];
    let b1 = result[1];
    let b2 = result[2];
    let b3 = result[3];
    let b4 = result[4];
    let b5 = result[5];
    let b6 = result[6];
    let b7 = result[7];

    let zero_copy_predicate = quote! { any(feature = "pinocchio", not(feature = "borsh")) };

    // --- Size Calculation for Automatic Padding ---
    // Zero-copy structs must be properly padded and aligned to prevent memory faults
    // when using `bytemuck`. This logic computes the struct's byte size and injects dummy
    // padding fields (`_padding`) to achieve standard 8-byte or 16-byte alignment.
    let mut total_size: usize = 0;
    let mut fields_list = quote! {};
    for field in ast.fields.iter() {
        let f_vis = &field.vis;
        let f_ident = &field.ident;
        let f_ty = &field.ty;
        fields_list = quote! { #fields_list #f_vis #f_ident: #f_ty, };

        let ty_str = quote!(#f_ty).to_string().replace(" ", "");
        let size = match ty_str.as_str() {
            "Address" => 32,
            "u128" | "i128" => 16,
            "u64" | "i64" => 8,
            "u32" | "i32" => 4,
            "u16" | "i16" => 2,
            "u8" | "i8" | "bool" | "Bool" => 1,
            s if s.starts_with("Opt<") => {
                // Opt<T> is T + 1 byte tag + 7 bytes padding = T + 8
                let inner = s.trim_start_matches("Opt<").trim_end_matches(">");
                let inner_size = match inner {
                    "Address" => 32,
                    "u128" | "i128" => 16,
                    "u64" | "i64" => 8,
                    "u32" | "i32" => 4,
                    "u16" | "i16" => 2,
                    "u8" | "i8" | "bool" | "Bool" => 1,
                    _ => 8,
                };
                inner_size + 8
            }
            s if s.starts_with("[u8;") => s
                .trim_start_matches("[u8;")
                .trim_end_matches("]")
                .parse::<usize>()
                .unwrap_or(0),
            _ => 8,
        };
        total_size += size;
    }

    let mut has_u128 = false;
    for field in ast.fields.iter() {
        let ty_str = quote!(#field.ty).to_string().replace(" ", "");
        if ty_str.contains("u128") || ty_str.contains("i128") {
            has_u128 = true;
        }
    }

    let alignment = if has_u128 { 16 } else { 8 };
    let padding_size = if total_size.is_multiple_of(alignment) {
        0
    } else {
        alignment - (total_size % alignment)
    };
    let padding_field = if padding_size > 0 {
        let size = padding_size;
        quote! { pub _padding: [u8; #size], }
    } else {
        quote! {}
    };

    let expanded = quote! {
        #[cfg(not(#zero_copy_predicate))]
        #[derive(Clone, naclac_lang::prelude::BorshSerialize, Default)]
        #[cfg_attr(feature = "debug-mode", derive(Debug))]
        #[borsh(crate = "naclac_lang::prelude::borsh")]
        #ast

        #[cfg(#zero_copy_predicate)]
        #[cfg_attr(feature = "debug-mode", derive(Debug))]
        #[derive(Clone, Copy, Default, naclac_lang::prelude::Pod, naclac_lang::prelude::Zeroable)]
        #[bytemuck(crate = "naclac_lang::bytemuck")]
        #[repr(C)]
        #vis struct #struct_name {
            #fields_list
            #padding_field
        }

        impl #struct_name {
            pub fn emit(&self) {
                #[cfg(#zero_copy_predicate)]
                {
                    // Use direct zero-copy emission
                    let discriminator = [#b0, #b1, #b2, #b3, #b4, #b5, #b6, #b7];
                    let data = naclac_lang::prelude::bytemuck::bytes_of(self);
                    naclac_lang::prelude::sol_log_data(&[&discriminator, data]);
                }

                #[cfg(not(#zero_copy_predicate))]
                {
                    // Fallback to standard emission (Borsh)
                    naclac_lang::event::emit_event(self, [#b0, #b1, #b2, #b3, #b4, #b5, #b6, #b7]);
                }
            }
        }
    };

    TokenStream::from(expanded)
}
