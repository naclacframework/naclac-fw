//! # Naclac Macros
//!
//! Procedural macros for the Naclac framework. This crate provides the core attributes and derives
//! necessary to build high-performance smart contracts.
//!
//! ## Supported Execution Modes
//! The macros dynamically generate code optimized for three distinct execution environments:
//! 1. **Solana Program + Borsh**: Traditional `std` mode using standard Borsh serialization.
//! 2. **Solana Program + Zero-Copy**: Highly optimized `std` mode utilizing native, zero-copy instruction parsing and custom heap allocation for dynamic types.
//! 3. **Pinocchio + Zero-Copy**: Strictly `no_std` mode built entirely on zero-copy principles for peak SBF performance and minimal compute.

extern crate proc_macro;

mod accounts;
mod component;
mod error_code;
mod event;
mod instruction;
mod program;
mod system;
mod interface;

use proc_macro::TokenStream;

/// Defines a zero-copy aligned data structure (Account).
///
/// Automatically adds `#[repr(C, packed)]` when zero-copy is enabled.
#[proc_macro_attribute]
pub fn component(attr: TokenStream, item: TokenStream) -> TokenStream {
    component::expand(attr, item)
}

/// Unused in current framework iteration (reserved for future systems).
#[proc_macro_attribute]
pub fn system(attr: TokenStream, item: TokenStream) -> TokenStream {
    system::expand(attr, item)
}

/// Attribute for annotating an instruction logic function.
#[proc_macro_attribute]
pub fn instruction(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    instruction::expand(attr, item)
}

/// The main entrypoint macro for a Naclac smart contract.
///
/// This generates the overarching instruction dispatcher and handles Cross-Program
/// Invocation (CPI) module generation. It supports `zero_copy` configuration natively.
#[proc_macro_attribute]
pub fn naclac_program(attr: TokenStream, item: TokenStream) -> TokenStream {
    program::expand(attr, item)
}

/// Generates standardized custom error codes for the program.
#[proc_macro_attribute]
pub fn error_code(attr: TokenStream, item: TokenStream) -> TokenStream {
    error_code::expand(attr, item)
}

/// Declare a CPI interface for an external naclac program.
///
/// This macro should be used within a crate that depends on `naclac-interface`.
/// It generates type-safe CPI helpers that automatically route calls through
/// either the standard Solana or Pinocchio backend based on the caller's environment.
#[proc_macro_attribute]
pub fn naclac_interface(attr: TokenStream, item: TokenStream) -> TokenStream {
    interface::expand(attr, item)
}

/// Defines an event struct.
///
/// If zero-copy is active, generates highly optimized, allocator-free event emission
/// logic directly into the Solana runtime via `sol_invoke`.
#[proc_macro_attribute]
pub fn event(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    event::expand(attr, item)
}

/// Stub for CPI attributes (ignored, acts as a marker).
#[proc_macro_attribute]
pub fn cpi(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Stub for constant declarations (ignored, acts as a marker).
#[proc_macro_attribute]
pub fn constant(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Derives the necessary validation, deserialization, and constraints for an Accounts struct.
///
/// Generates zero-copy fixed array CPI properties (`[AccountMeta; N]`) regardless of the execution mode.
#[proc_macro_derive(Accounts, attributes(account, instruction))]
pub fn derive_accounts(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    accounts::expand_derive_accounts(item)
}

/// Implements lightweight account loaders (zero-copy specific).
#[proc_macro_derive(AccountsLoader, attributes(account, instruction))]
pub fn derive_accounts_loader(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    accounts::expand_derive_accounts_loader(item)
}

/// Internal derivation for legacy Borsh serialization fallback.
#[proc_macro_derive(NaclacSerialize)]
pub fn derive_naclac_serialize(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::DeriveInput);
    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let body = match &input.data {
        syn::Data::Struct(data_struct) => {
            let mut field_serializations = Vec::new();
            for field in &data_struct.fields {
                let field_ident = &field.ident;
                field_serializations.push(quote::quote! {
                    naclac_lang::prelude::borsh::BorshSerialize::serialize(&self.#field_ident, writer)?;
                });
            }
            quote::quote! { #(#field_serializations)* }
        }
        syn::Data::Enum(data_enum) => {
            let mut arms = Vec::new();
            for (i, variant) in data_enum.variants.iter().enumerate() {
                let variant_ident = &variant.ident;
                let idx = i as u8;
                arms.push(quote::quote! {
                    #ident::#variant_ident => {
                        writer.write_all(&[#idx])?;
                    }
                });
            }
            quote::quote! {
                match self {
                    #(#arms)*
                }
            }
        }
        _ => panic!("NaclacSerialize only supports Structs and Enums"),
    };

    let expanded = quote::quote! {
        impl #impl_generics naclac_lang::prelude::borsh::BorshSerialize for #ident #ty_generics #where_clause {
            fn serialize<W: naclac_lang::prelude::borsh::io::Write>(&self, writer: &mut W) -> naclac_lang::prelude::borsh::io::Result<()> {
                #body
                Ok(())
            }
        }
    };
    expanded.into()
}

/// Derives the necessary `Pod` and `Zeroable` traits for safe byte-level casting.
///
/// Critical for achieving `no_std` zero-copy performance without data corruption.
#[proc_macro_attribute]
pub fn naclac_pod(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let mut input = syn::parse_macro_input!(item as syn::DeriveInput);
    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // Check if it's an Enum and add #[repr(u8)] if no repr is present
    if let syn::Data::Enum(_) = &input.data {
        let has_repr = input.attrs.iter().any(|attr| attr.path().is_ident("repr"));
        if !has_repr {
            input.attrs.push(syn::parse_quote!(#[repr(u8)]));
        }
    }

    let expanded = quote::quote! {
        #input

        // SAFETY: We explicitly apply #[repr(C)] or #[repr(u8)] to structs/enums derived with
        // ZcType to ensure memory layout predictability, making it safe to treat as Pod.
        unsafe impl #impl_generics naclac_lang::prelude::Pod for #ident #ty_generics #where_clause {}
        // SAFETY: Zeroed memory is a valid initial state for Pod types.
        unsafe impl #impl_generics naclac_lang::prelude::Zeroable for #ident #ty_generics #where_clause {}

        impl #impl_generics naclac_lang::prelude::NaclacPod for #ident #ty_generics #where_clause {
            #[inline(always)]
            fn naclac_from_bytes(data: &[u8]) -> Self {
                // SAFETY: We use `read_unaligned` to safely read the struct from the byte slice.
                // This is required because SBF instruction data streams may not align properly
                // to the struct's natural alignment boundaries, preventing standard bytemuck casting.
                unsafe { core::ptr::read_unaligned(data.as_ptr() as *const Self) }
            }
            #[inline(always)]
            fn naclac_size() -> usize {
                core::mem::size_of::<Self>()
            }
        }
    };
    expanded.into()
}

/// Internal derivation for legacy Borsh deserialization fallback.
#[proc_macro_derive(NaclacDeserialize)]
pub fn derive_naclac_deserialize(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::DeriveInput);
    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let body = match &input.data {
        syn::Data::Struct(data_struct) => {
            let mut field_deserializations = Vec::new();
            let mut field_names = Vec::new();
            for field in &data_struct.fields {
                let field_ident = &field.ident;
                field_names.push(field_ident);
                field_deserializations.push(quote::quote! {
                    let #field_ident = naclac_lang::prelude::borsh::BorshDeserialize::deserialize_reader(reader)?;
                });
            }
            quote::quote! {
                #(#field_deserializations)*
                Ok(Self { #(#field_names),* })
            }
        }
        syn::Data::Enum(data_enum) => {
            let mut arms = Vec::new();
            for (i, variant) in data_enum.variants.iter().enumerate() {
                let variant_ident = &variant.ident;
                let idx = i as u8;
                arms.push(quote::quote! {
                    #idx => Ok(#ident::#variant_ident),
                });
            }
            quote::quote! {
                let mut tag = [0u8; 1];
                reader.read_exact(&mut tag)?;
                match tag[0] {
                    #(#arms)*
                    _ => Err(naclac_lang::prelude::borsh::io::Error::new(
                        naclac_lang::prelude::borsh::io::ErrorKind::InvalidData,
                        "",
                    )),
                }
            }
        }
        _ => panic!("NaclacDeserialize only supports Structs and Enums"),
    };

    let expanded = quote::quote! {
        impl #impl_generics naclac_lang::prelude::borsh::BorshDeserialize for #ident #ty_generics #where_clause {
            fn deserialize_reader<R: naclac_lang::prelude::borsh::io::Read>(reader: &mut R) -> naclac_lang::prelude::borsh::io::Result<Self> {
                #body
            }
        }
    };
    expanded.into()
}
