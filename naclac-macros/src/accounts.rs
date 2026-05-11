//! # Accounts Macro Logic
//!
//! Handles the expansion of the `#[derive(Accounts)]` macro, generating highly
//! optimized account validation, instruction deserialization, and Zero-Copy safety logic.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

use crate::instruction::close_account;
use crate::instruction::init_cpi;
use crate::instruction::parser;
use crate::instruction::realloc;
use crate::instruction::security;

/// Entrypoint for standard `#[derive(Accounts)]` expansion.
pub fn expand_derive_accounts(item: TokenStream) -> TokenStream {
    expand_derive_accounts_inner(item, false)
}

/// Entrypoint for `#[derive(AccountsLoader)]` expansion (zero-copy specific).
pub fn expand_derive_accounts_loader(item: TokenStream) -> TokenStream {
    expand_derive_accounts_inner(item, true)
}

/// Core logic for Accounts derivation.
///
/// Processes field constraints, performs security checks, generates instruction data
/// deserialization, and manages the hydration/teardown pipeline.
fn expand_derive_accounts_inner(item: TokenStream, is_loader: bool) -> TokenStream {
    let ast = parse_macro_input!(item as DeriveInput);
    let struct_name = &ast.ident;

    // 1. Ensure the macro is only used on a Struct with named fields
    let fields_named = match &ast.data {
        Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(fields_named) => fields_named,
            _ => panic!("Naclac: #[derive(Accounts)] must have named fields"),
        },
        _ => panic!("Naclac: #[derive(Accounts)] can only be used on structs"),
    };

    // 2. Parse the fields using our new V2 parser
    let parsed_fields = parser::parse_struct_fields(fields_named)
        .expect("Naclac: Failed to parse account attributes");
    let mut instr_names: Vec<syn::Ident> = Vec::new();
    let mut instr_types: Vec<Box<syn::Type>> = Vec::new();

    for attr in &ast.attrs {
        if attr.path().is_ident("instruction") {
            if let Ok(parsed_args) = attr.parse_args_with(
                syn::punctuated::Punctuated::<syn::FnArg, syn::Token![,]>::parse_terminated,
            ) {
                for arg in parsed_args {
                    if let syn::FnArg::Typed(pat_type) = arg {
                        if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                            instr_names.push(pat_ident.ident.clone());
                            instr_types.push(pat_type.ty.clone());
                        }
                    }
                }
            }
        }
    }

    let is_zero_copy = if cfg!(feature = "pinocchio") {
        true
    } else {
        is_loader || {
            ast.attrs.iter().any(|attr| {
                if attr.path().is_ident("instruction") {
                    quote! { #attr }.to_string().contains("zero_copy")
                } else {
                    false
                }
            })
        }
    };

    let ix_deserializer = if !instr_names.is_empty() {
        let mut pod_instr_names: Vec<&syn::Ident> = Vec::new();
        let mut pod_instr_types: Vec<&Box<syn::Type>> = Vec::new();
        let mut dynamic_instr_names: Vec<&syn::Ident> = Vec::new();
        let mut dynamic_instr_types: Vec<&Box<syn::Type>> = Vec::new();
        let mut slice_instr_names: Vec<&syn::Ident> = Vec::new();

        for (name, ty) in instr_names.iter().zip(instr_types.iter()) {
            let ty_str = quote! { #ty }.to_string().replace(" ", "");
            if ty_str.contains("&[u8]") {
                slice_instr_names.push(name);
            } else if ty_str.contains("Vec") || ty_str.contains("String") {
                dynamic_instr_names.push(name);
                dynamic_instr_types.push(ty);
            } else {
                pod_instr_names.push(name);
                pod_instr_types.push(ty);
            }
        }

        if is_zero_copy {
            let pod_reads = pod_instr_names.iter().zip(pod_instr_types.iter()).map(|(name, ty)| {
                quote! {
                    let #name = {
                        let __sz = <#ty as naclac_lang::prelude::NaclacPod>::naclac_size();
                        if instruction_data.len() < __ix_offset + __sz {
                            return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                        }
                        let __val = <#ty as naclac_lang::prelude::NaclacPod>::naclac_from_bytes(
                            &instruction_data[__ix_offset..__ix_offset + __sz]
                        );
                        __ix_offset += __sz;
                        __val
                    };
                }
            });

            let dynamic_reads = dynamic_instr_names.iter().zip(dynamic_instr_types.iter()).map(|(name, ty)| {
                let ty_str = quote! { #ty }.to_string().replace(" ", "");
                let __v_logic = if ty_str.contains("Vec") {
                    quote! { <naclac_lang::prelude::Span<'_>>::from_bytes(&instruction_data[__ix_offset..__ix_offset + __len])? }
                } else {
                    quote! { <naclac_lang::prelude::ZcString<'_>>::from_bytes(&instruction_data[__ix_offset..__ix_offset + __len])? }
                };

                quote! {
                    let #name = {
                        if instruction_data.len() < __ix_offset + 4 {
                            return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                        }
                        let mut __len_bytes = [0u8; 4];
                        __len_bytes.copy_from_slice(&instruction_data[__ix_offset..__ix_offset+4]);
                        let __len = u32::from_le_bytes(__len_bytes) as usize;
                        __ix_offset += 4;
                        if instruction_data.len() < __ix_offset + __len {
                            return Err(naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0));
                        }
                        let __v = #__v_logic;
                        __ix_offset += __len;
                        __v
                    };
                }
            });

            let slice_reads = slice_instr_names.iter().map(|name| {
                quote! { let #name = &instruction_data[__ix_offset..]; }
            });

            quote! {
                let mut __ix_offset: usize = 8;
                #(#pod_reads)*
                #(#dynamic_reads)*
                #(#slice_reads)*
            }
        } else {
            // Standard Borsh Path: Heap-allocates instruction arguments dynamically.
            quote! {
                let mut __r: &[u8] = &instruction_data[8..];
                #(
                    let #instr_names = <#instr_types as naclac_lang::prelude::BorshDeserialize>::deserialize(&mut __r)
                        .map_err(|_| naclac_lang::prelude::NaclacError::InvalidInstructionData.err(0))?;
                )*
            }
        }
    } else {
        quote! {}
    };

    let mut field_loaders = Vec::new();
    let mut struct_instantiation = Vec::new();

    for field in &parsed_fields {
        let field_name = &field.ident;
        let type_str = &field.type_str;
        let ty = &field.ty;

        #[cfg(feature = "pinocchio")]
        {
            if type_str.contains("Account")
                && !type_str.contains("AccountLoader")
                && !type_str.contains("AccountInfo")
                && !type_str.contains("Signer")
                && !type_str.contains("Program")
                && !type_str.contains("Interface")
                && !type_str.contains("ZcAccount")
            {
                return quote! {
                     compile_error!("Naclac Error: In Pinocchio mode, you must use AccountLoader, not Account, to ensure zero-copy performance for '{}'.", quote!{#field_name}.to_string());
                 }.into();
            }
            if type_str.contains("Vec")
                || (type_str.contains("String") && !type_str.contains("FixedString"))
            {
                return quote! {
                     compile_error!("Naclac Error: In Zero-Copy mode, heap-based types like 'Vec' and 'String' are prohibited in account storage. Please use fixed-size arrays or 'FixedString' for '{}'.", quote!{#field_name}.to_string());
                 }.into();
            }
        }

        let idx = field.index;
        let (metadata_checks, constraint_checks) =
            security::generate_security_checks(field, &parsed_fields);
        let init_logic = init_cpi::generate_init_cpi(field);

        let loader_logic = if type_str.contains("Signer") {
            quote! {
                #[cfg(not(feature = "pinocchio"))]
                let info = naclac_lang::solana_program::account_info::next_account_info(account_info_iter)?;
                #[cfg(feature = "pinocchio")]
                let info = naclac_lang::prelude::next_account_info(account_info_iter)?;
                #metadata_checks
                #init_logic
                let mut #field_name: #ty = naclac_lang::prelude::Signer::try_from(info, #idx)?;
                #constraint_checks
            }
        } else if type_str.contains("Interface") {
            let expected_ids = if type_str.contains("TokenInterface") {
                quote! { &[naclac_lang::prelude::TOKEN_PROGRAM_ID, naclac_lang::prelude::TOKEN_2022_PROGRAM_ID] }
            } else {
                quote! { &[] }
            };
            quote! {
                #[cfg(not(feature = "pinocchio"))]
                let info = naclac_lang::solana_program::account_info::next_account_info(account_info_iter)?;
                #[cfg(feature = "pinocchio")]
                let info = naclac_lang::prelude::next_account_info(account_info_iter)?;
                #metadata_checks
                #init_logic
                let mut #field_name: #ty = naclac_lang::prelude::Interface::try_from(info, #expected_ids, #idx)?;
                #constraint_checks
            }
        } else if type_str.contains("Program") {
            let expected_id = if let Some(addr_expr) = &field.address {
                quote! { &(#addr_expr) }
            } else if type_str.contains("Token2022") {
                quote! { &naclac_lang::prelude::TOKEN_2022_PROGRAM_ID }
            } else if type_str.contains("Token") {
                quote! { &naclac_lang::prelude::TOKEN_PROGRAM_ID }
            } else if type_str.contains("System") {
                quote! { &naclac_lang::prelude::SYSTEM_PROGRAM_ID }
            } else if type_str.contains("AssociatedToken") {
                quote! { &naclac_lang::prelude::ASSOCIATED_TOKEN_PROGRAM_ID }
            } else {
                quote! { &naclac_lang::prelude::Key::key(info) }
            };
            quote! {
                #[cfg(not(feature = "pinocchio"))]
                let info = naclac_lang::solana_program::account_info::next_account_info(account_info_iter)?;
                #[cfg(feature = "pinocchio")]
                let info = naclac_lang::prelude::next_account_info(account_info_iter)?;
                #metadata_checks
                #init_logic
                let mut #field_name: #ty = naclac_lang::prelude::Program::try_from(info, #expected_id, #idx)?;
                #constraint_checks
            }
        } else if type_str.contains("AccountLoader") || type_str.contains("ZcAccount") {
            quote! {
                #[cfg(not(feature = "pinocchio"))]
                let info = naclac_lang::solana_program::account_info::next_account_info(account_info_iter)?;
                #[cfg(feature = "pinocchio")]
                let info = naclac_lang::prelude::next_account_info(account_info_iter)?;
                #metadata_checks
                #init_logic
                let mut #field_name: #ty = naclac_lang::prelude::AccountLoader::try_from(info, #idx)?;
                #constraint_checks
            }
        } else if type_str.contains("Account") && !type_str.contains("AccountInfo") {
            quote! {
                #[cfg(not(feature = "pinocchio"))]
                let info = naclac_lang::solana_program::account_info::next_account_info(account_info_iter)?;
                #[cfg(feature = "pinocchio")]
                let info = naclac_lang::prelude::next_account_info(account_info_iter)?;
                #metadata_checks
                #init_logic
                let mut #field_name: #ty = naclac_lang::prelude::Account::try_from(info, #idx)?;
                #constraint_checks
            }
        } else {
            quote! {
                #[cfg(not(feature = "pinocchio"))]
                let info = naclac_lang::solana_program::account_info::next_account_info(account_info_iter)?;
                #[cfg(feature = "pinocchio")]
                let info = naclac_lang::prelude::next_account_info(account_info_iter)?;
                #metadata_checks
                #init_logic
                let mut #field_name: #ty = info.clone();
                #constraint_checks
            }
        };

        field_loaders.push(loader_logic);
        struct_instantiation.push(quote! { #field_name });
    }

    // --- Security Check Generation ---
    let relational_checks = security::generate_relational_checks(&parsed_fields, quote! { self });
    let relational_checks_me =
        security::generate_relational_checks(&parsed_fields, quote! { __me });
    let realloc_logic = realloc::generate_realloc_logic(&parsed_fields);
    let close_logic = close_account::generate_close_logic(&parsed_fields);

    let save_logic: Vec<proc_macro2::TokenStream> = {
        let mut v = Vec::new();
        for field in &parsed_fields {
            let field_name = &field.ident;
            let type_str = &field.type_str;
            if field.is_mut
                && type_str.contains("Account")
                && !type_str.contains("AccountInfo")
                && !type_str.contains("AccountLoader")
                && !type_str.contains("ZcAccount")
            {
                v.push(quote! {
                    if self.#field_name.info.owner == program_id {
                        self.#field_name.exit()?;
                    }
                });
            }
        }
        v
    };

    let mut bumps_fields = Vec::new();
    let mut bumps_instantiations = Vec::new();
    for field in &parsed_fields {
        if field.pda_seed.is_some() {
            let field_name = &field.ident;
            let bump_var = quote::format_ident!("__bump_{}", field_name);
            bumps_fields.push(quote! { pub #field_name: u8 });
            bumps_instantiations.push(quote! { #field_name: #bump_var });
        }
    }

    let bumps_struct_name = quote::format_ident!("{}Bumps", struct_name);
    let bumps_struct_def = quote! {
        #[derive(Clone, Copy, Default)]
        pub struct #bumps_struct_name {
            #(#bumps_fields),*
        }
    };

    let bumps_instantiation_logic = quote! { #bumps_struct_name { #(#bumps_instantiations),* } };

    // --- Hydration System ---
    // Generates `Hydrated` structs that maintain zero-copy pointer guards and bounds.
    let mut hydrated_fields = Vec::new();
    let mut hydration_prep = Vec::new();
    let mut hydration_logic = Vec::new();
    let mut mut_zero_copy_fields = Vec::new();

    for field in &parsed_fields {
        let name = &field.ident;
        let ty = &field.ty;
        let type_str = &field.type_str;

        if type_str.contains("AccountLoader") || type_str.contains("ZcAccount") {
            let is_token_account = type_str.contains("TokenAccount");
            if (type_str.contains("mut") || field.is_mut)
                && field.pda_seed.is_some()
                && !is_token_account
            {
                mut_zero_copy_fields.push(name);
            }
            let inner_type = if type_str.contains("<") {
                let start = type_str.find(',').map(|i| i + 1).unwrap_or(0);
                let end = type_str.rfind('>').unwrap_or(type_str.len());
                let t = &type_str[start..end].trim();
                let ident: syn::Type = syn::parse_str(t).expect("Failed to parse inner type");
                quote! { #ident }
            } else {
                quote! { () }
            };

            if type_str.contains("mut") || field.is_mut {
                hydrated_fields.push(
                    quote! { pub #name: naclac_lang::prelude::KeyedRefMut<'info, #inner_type> },
                );
                hydration_prep.push(quote! { let #name = self.#name.hydrate_mut()?; });
            } else {
                hydrated_fields
                    .push(quote! { pub #name: naclac_lang::prelude::KeyedRef<'info, #inner_type> });
                hydration_prep.push(quote! { let #name = self.#name.hydrate()?; });
            }
        } else {
            hydrated_fields.push(quote! { pub #name: #ty });
            hydration_prep.push(quote! { let #name = self.#name; });
        }
        hydration_logic.push(quote! { #name });
    }

    let hydrated_struct_name = quote::format_ident!("{}Hydrated", struct_name);
    let vis = &ast.vis;
    let hydration_impl = if is_zero_copy {
        quote! {
            #vis struct #hydrated_struct_name<'info> {
                #( #hydrated_fields, )*
                pub __bumps: #bumps_struct_name,
            }

            impl<'info> naclac_lang::prelude::Bumps for #hydrated_struct_name<'info> {
                type BumpsStruct = #bumps_struct_name;
            }

            impl<'info> #struct_name<'info> {
                pub fn hydrate(self, bumps: #bumps_struct_name) -> naclac_lang::prelude::Result<#hydrated_struct_name<'info>> {
                    #(#hydration_prep)*
                    Ok(#hydrated_struct_name {
                        #( #hydration_logic, )*
                        __bumps: bumps,
                    })
                }
            }

            impl<'info> #hydrated_struct_name<'info> {
                pub fn teardown(&mut self, program_id: &'info naclac_lang::prelude::Pubkey) -> naclac_lang::prelude::Result {
                    #(#relational_checks)*
                    #(#realloc_logic)*
                    #( self.#mut_zero_copy_fields.bump = self.__bumps.#mut_zero_copy_fields; )*
                    #(#save_logic)*
                    #(#close_logic)*
                    Ok(())
                }
            }
        }
    } else {
        quote! {
            #vis type #hydrated_struct_name<'info> = #struct_name<'info>;
            impl<'info> #struct_name<'info> {
                pub fn hydrate(self, _bumps: #bumps_struct_name) -> naclac_lang::prelude::Result<Self> { Ok(self) }
            }
        }
    };

    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();
    let n_fields = parsed_fields.len();
    let field_names: Vec<_> = parsed_fields.iter().map(|f| &f.ident).collect();
    let is_signers: Vec<_> = parsed_fields
        .iter()
        .map(|f| f.type_str.contains("Signer"))
        .collect();
    let is_writables: Vec<_> = parsed_fields
        .iter()
        .map(|f| f.is_mut || f.type_str.contains("mut"))
        .collect();

    use heck::ToSnakeCase;
    let cpi_mod_name = quote::format_ident!("{}_cpi", struct_name.to_string().to_snake_case());

    let expanded = quote! {
        #bumps_struct_def
        #hydration_impl

        #[cfg(feature = "cpi")]
        pub mod #cpi_mod_name {
            use super::*;
            pub mod accounts {
                use super::*;
                #[derive(Clone)]
                pub struct #struct_name<'info> {
                    #( pub #field_names: naclac_lang::prelude::AccountInfo<'info>, )*
                }

                impl<'info> #struct_name<'info> {
                    pub fn to_account_metas(&self) -> [naclac_lang::prelude::AccountMeta; #n_fields] {
                        [
                            #(
                                naclac_lang::prelude::AccountMeta {
                                    pubkey: naclac_lang::prelude::Key::key(&self.#field_names),
                                    is_signer: #is_signers,
                                    is_writable: #is_writables,
                                },
                            )*
                        ]
                    }

                    pub fn to_account_infos(&self) -> [naclac_lang::prelude::AccountInfo<'info>; #n_fields] {
                        [
                            #(
                                naclac_lang::prelude::ToAccountInfo::to_account_info(&self.#field_names),
                            )*
                        ]
                    }
                }


            }
        }

        impl #impl_generics #struct_name #ty_generics #where_clause {
            pub fn to_account_metas(&self) -> [naclac_lang::prelude::AccountMeta; #n_fields] {
                [
                    #(
                        naclac_lang::prelude::AccountMeta {
                            pubkey: naclac_lang::prelude::Key::key(&self.#field_names),
                            is_signer: #is_signers,
                            is_writable: #is_writables,
                        },
                    )*
                ]
            }

            pub fn to_account_infos(&self) -> [naclac_lang::prelude::AccountInfo<'info>; #n_fields] {
                [
                    #(
                        naclac_lang::prelude::ToAccountInfo::to_account_info(&self.#field_names),
                    )*
                ]
            }
        }



        impl #impl_generics naclac_lang::prelude::Bumps for #struct_name #ty_generics #where_clause {
            type BumpsStruct = #bumps_struct_name;
        }

        impl #impl_generics #struct_name #ty_generics #where_clause {
            /// Loads and validates the accounts provided to the instruction.
            ///
            /// Performs metadata checks, constraint validation, and instruction deserialization.
            pub fn load_and_validate(
                program_id: &'info naclac_lang::prelude::Pubkey,
                accounts: &'info [naclac_lang::prelude::AccountType<'info>],
                instruction_data: &[u8]
            ) -> Result<(Self, &'info [naclac_lang::prelude::AccountType<'info>], #bumps_struct_name), naclac_lang::prelude::ProgramError> {
                #ix_deserializer
                let account_info_iter = &mut accounts.iter();
                #(#field_loaders)*
                let remaining_accounts = account_info_iter.as_slice();
                let mut __me = Self { #(#struct_instantiation),* };
                #(#relational_checks_me)*
                Ok((__me, remaining_accounts, #bumps_instantiation_logic))
            }

            /// Persists mutations and handles cleanup.
            ///
            /// Invokes account reallocation, ownership checks, and safely syncs bumps.
            pub fn teardown(&mut self, program_id: &'info naclac_lang::prelude::Pubkey) -> naclac_lang::prelude::Result {
                #(#relational_checks)*
                #(#realloc_logic)*
                #(#save_logic)*
                #(#close_logic)*
                Ok(())
            }
        }
    };

    expanded.into()
}
