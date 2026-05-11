//! # Instruction CPI Initialization Logic
//!
//! Generates the Cross-Program Invocation (CPI) logic for initializing accounts dynamically
//! within an instruction via the `init` constraint. This includes calculating rent, deriving PDA
//! signer seeds, and invoking the System Program to allocate memory.

use crate::instruction::parser::ParsedField;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Type;

fn extract_inner_type(ty: &Type) -> TokenStream {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "AccountLoader"
                || segment.ident == "ZcAccount"
                || segment.ident == "Account"
            {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    for arg in &args.args {
                        if let syn::GenericArgument::Type(inner) = arg {
                            return quote! { #inner };
                        }
                    }
                }
            }
        }
    }
    quote! { #ty }
}

fn signer_seeds_tokens(field: &ParsedField) -> TokenStream {
    if let Some(seed_array) = &field.pda_seed {
        let field_ident = &field.ident;
        let seed_bindings: Vec<TokenStream> = seed_array.elems.iter().enumerate().map(|(i, expr)| {
            let ident = format_ident!("__seed_{}_{}", field_ident, i);
            let raw_str = quote! { #expr }.to_string();
            let expr_str: String = raw_str.chars().filter(|c| !c.is_whitespace()).collect();

            let mut binding_str = if expr_str.contains(".key") && !expr_str.contains(".key()") && !expr_str.contains(".info.key") {
                expr_str.replace(".key", ".key()")
            } else {
                expr_str.clone()
            };

            let is_key_access = binding_str.contains(".key()");
            let has_le_bytes  = binding_str.contains(".to_le_bytes()");
            let has_be_bytes  = binding_str.contains(".to_be_bytes()");

            binding_str = binding_str
                .replace(".as_ref()", "")
                .replace(".as_bytes()", "")
                .replace(".to_le_bytes()", "")
                .replace(".to_be_bytes()", "");

            if has_le_bytes      { binding_str = format!("{}.to_le_bytes()", binding_str); }
            else if has_be_bytes { binding_str = format!("{}.to_be_bytes()", binding_str); }

            let binding_expr: syn::Expr = syn::parse_str(&binding_str).unwrap_or(expr.clone());

            if is_key_access {
                return quote! { let #ident = naclac_lang::prelude::ToPubkey::to_pubkey(&(#binding_expr)); };
            }

            quote! { let #ident = #binding_expr; }
        }).collect();
        let seed_refs: Vec<TokenStream> = seed_array
            .elems
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let ident = format_ident!("__seed_{}_{}", field_ident, i);
                quote! { naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(&#ident) }
            })
            .collect();

        quote! {
            let __expected_bump_arr = [expected_bump];
            #(#seed_bindings)*
            let signer_seeds: &[&[&[u8]]] = &[&[
                #(#seed_refs),*,
                &__expected_bump_arr
            ]];

            #[cfg(feature = "pinocchio")]
            let pinocchio_seeds = [
                #(naclac_lang::prelude::pinocchio::cpi::Seed::from(#seed_refs)),*,
                naclac_lang::prelude::pinocchio::cpi::Seed::from(&__expected_bump_arr[..])
            ];
            #[cfg(feature = "pinocchio")]
            let __pinocchio_signer = naclac_lang::prelude::pinocchio::cpi::Signer::from(&pinocchio_seeds);
            #[cfg(feature = "pinocchio")]
            let pinocchio_signer_seeds: &[naclac_lang::prelude::pinocchio::cpi::Signer] = &[__pinocchio_signer];
        }
    } else {
        quote! {
            let signer_seeds: &[&[&[u8]]] = &[];
            #[cfg(feature = "pinocchio")]
            let pinocchio_signer_seeds: &[naclac_lang::prelude::pinocchio::cpi::Signer] = &[];
        }
    }
}

/// Generates initialization CPI logic for `#[account(init, ...)]` constraints.
///
/// Automatically delegates to `pinocchio_system` or standard `solana_program` depending
/// on the active compilation target. It handles space calculation, lamport transfers,
/// and zero-copy discriminator injection.
pub fn generate_init_cpi(field: &ParsedField) -> TokenStream {
    let init_config = match &field.init_config {
        Some(config) => config,
        None => return quote! {},
    };

    let payer_str = &init_config.payer;
    let payer_ident = format_ident!("{}", payer_str);
    let signer_seeds_logic = signer_seeds_tokens(field);

    if let Some(mint_expr) = &field.token_mint {
        let authority_expr = field
            .token_authority
            .as_ref()
            .expect("Naclac: token::authority must be specified alongside token::mint");

        let idx = field.index;
        return quote! {
            if !info.data_is_empty() {
                return Err(naclac_lang::prelude::NaclacError::AccountAlreadyInitialized.err(#idx));
            }
            if info.data_is_empty() {
                #[cfg(not(feature = "pinocchio"))]
                {
                    let rent = naclac_lang::solana_program::rent::Rent::get()?;
                    let lamports = rent.minimum_balance(165_usize);

                    #signer_seeds_logic

                    let __sys_info = accounts.iter()
                        .find(|a| naclac_lang::prelude::Key::key(*a) == naclac_lang::prelude::SYSTEM_PROGRAM_ID)
                        .ok_or(naclac_lang::prelude::NaclacError::ProgramIdMismatch.err(0))?;

                    let __tok_prog_info = accounts.iter()
                        .find(|a| {
                            naclac_lang::prelude::Key::key(*a) == naclac_lang::prelude::TOKEN_PROGRAM_ID ||
                            naclac_lang::prelude::Key::key(*a) == naclac_lang::prelude::TOKEN_2022_PROGRAM_ID
                        })
                        .ok_or(naclac_lang::prelude::NaclacError::ProgramIdMismatch.err(0))?;

                    let __payer_info_wrapper = naclac_lang::prelude::ToAccountInfo::to_account_info(&#payer_ident);
                    let __payer_info = &__payer_info_wrapper;

                    naclac_lang::solana_program::program::invoke_signed(
                        &naclac_lang::prelude::solana_system_interface::instruction::create_account(
                            &naclac_lang::prelude::Key::key(__payer_info),
                            &naclac_lang::prelude::Key::key(info),
                            lamports,
                            165_u64,
                            &naclac_lang::prelude::Key::key(__tok_prog_info),
                        ),
                        &[__payer_info.clone(), info.clone(), __sys_info.clone()],
                        signer_seeds,
                    )?;

                    let __mint_info_wrapper = naclac_lang::prelude::ToAccountInfo::to_account_info(&#mint_expr);
                    let __mint_info = &__mint_info_wrapper;
                    let __authority_info = naclac_lang::prelude::ToAccountInfo::to_account_info(&#authority_expr);
                    let __authority_key = naclac_lang::prelude::Key::key(&__authority_info);

                    let mut __ix_data = [0u8; 33];
                    __ix_data[0] = 18; // InitializeAccount3 discriminant
                    __ix_data[1..].copy_from_slice(__authority_key.as_ref());

                    let __init_ix = naclac_lang::solana_program::instruction::Instruction {
                        program_id: naclac_lang::prelude::Key::key(__tok_prog_info),
                        accounts: naclac_lang::prelude::vec![
                            naclac_lang::solana_program::instruction::AccountMeta::new(naclac_lang::prelude::Key::key(info), false),
                            naclac_lang::solana_program::instruction::AccountMeta::new_readonly(naclac_lang::prelude::Key::key(__mint_info), false),
                        ],
                        data: __ix_data.to_vec(),
                    };

                    naclac_lang::solana_program::program::invoke_signed(
                        &__init_ix,
                        &[info.clone(), __mint_info.clone(), __tok_prog_info.clone()],
                        signer_seeds,
                    )?;
                }

                #[cfg(feature = "pinocchio")]
                {
                    #signer_seeds_logic

                    let __sys_info = accounts.iter()
                        .find(|a| naclac_lang::prelude::Key::key(*a) == naclac_lang::prelude::SYSTEM_PROGRAM_ID)
                        .ok_or(naclac_lang::prelude::NaclacError::ProgramIdMismatch.err(0))?;

                    let __tok_prog_info = accounts.iter()
                        .find(|a| {
                            naclac_lang::prelude::Key::key(*a) == naclac_lang::prelude::TOKEN_PROGRAM_ID ||
                            naclac_lang::prelude::Key::key(*a) == naclac_lang::prelude::TOKEN_2022_PROGRAM_ID
                        })
                        .ok_or(naclac_lang::prelude::NaclacError::ProgramIdMismatch.err(0))?;

                    let __tok_prog_key = naclac_lang::prelude::Key::key(__tok_prog_info);
                    let __is_token_2022 = __tok_prog_key == naclac_lang::prelude::TOKEN_2022_PROGRAM_ID;

                    let __payer_view = naclac_lang::prelude::ToAccountInfo::to_account_info(&#payer_ident).view;

                    naclac_lang::prelude::pinocchio_system::instructions::CreateAccount {
                        from: &__payer_view,
                        to: &info.view,
                        lamports: naclac_lang::prelude::pinocchio::sysvars::rent::Rent::get()?.minimum_balance(165),
                        space: 165,
                        owner: __tok_prog_key.as_address(),
                    }.invoke_signed(pinocchio_signer_seeds)?;

                    let __mint_info_wrapper = naclac_lang::prelude::ToAccountInfo::to_account_info(&#mint_expr);
                    let __mint_info = &__mint_info_wrapper;
                    let __authority_info = naclac_lang::prelude::ToAccountInfo::to_account_info(&#authority_expr);
                    let __authority_key = naclac_lang::prelude::Key::key(&__authority_info);

                    if __is_token_2022 {
                        naclac_lang::prelude::pinocchio_token_2022::instructions::InitializeAccount3 {
                            token_program: __tok_prog_key.as_address(),
                            account: &info.view,
                            mint: &__mint_info.view,
                            owner: __authority_key.as_address(),
                        }.invoke()?;
                    } else {
                        naclac_lang::prelude::pinocchio_token::instructions::InitializeAccount3 {
                            account: &info.view,
                            mint: &__mint_info.view,
                            owner: __authority_key.as_address(),
                        }.invoke()?;
                    }
                }
            }
        };
    }

    let inner_type = extract_inner_type(&field.ty);
    let space_tokens = if let Some(space) = &init_config.space {
        quote! { (#space) }
    } else {
        quote! { (8 + core::mem::size_of::<#inner_type>()) }
    };

    let idx = field.index;
    quote! {
        let __needs_init = if info.data_is_empty() {
            true
        } else {
            #[cfg(not(feature = "pinocchio"))]
            {
                let __data = info.try_borrow_data()?;
                __data.len() >= 8 && __data[0..8] == [0u8; 8]
            }
            #[cfg(feature = "pinocchio")]
            {
                let __data = info.data();
                __data.len() >= 8 && __data[0..8] == [0u8; 8]
            }
        };

        if !__needs_init {
            return Err(naclac_lang::prelude::NaclacError::AccountAlreadyInitialized.err(#idx));
        }

        if __needs_init {
            #[cfg(not(feature = "pinocchio"))]
            {
                let rent = naclac_lang::solana_program::rent::Rent::get()?;
                let lamports = rent.minimum_balance((#space_tokens) as usize);

                #signer_seeds_logic

                let __sys_info = accounts.iter()
                    .find(|a| naclac_lang::prelude::Key::key(*a) == naclac_lang::prelude::SYSTEM_PROGRAM_ID)
                    .ok_or(naclac_lang::prelude::NaclacError::ProgramIdMismatch.err(0))?;

                let __payer_info_wrapper = naclac_lang::prelude::ToAccountInfo::to_account_info(&#payer_ident);
                let __payer_info = &__payer_info_wrapper;

                if info.data_is_empty() {
                    naclac_lang::solana_program::program::invoke_signed(
                        &naclac_lang::prelude::solana_system_interface::instruction::create_account(
                            &naclac_lang::prelude::Key::key(__payer_info),
                            &naclac_lang::prelude::Key::key(info),
                            lamports,
                            (#space_tokens) as u64,
                            program_id,
                        ),
                        &[
                            __payer_info.clone(),
                            info.clone(),
                            __sys_info.clone(),
                        ],
                        signer_seeds,
                    )?;
                }

                let mut __account_data = info.try_borrow_mut_data()?;
                __account_data[0..8].copy_from_slice(&#inner_type::DISCRIMINATOR);
            }

            #[cfg(feature = "pinocchio")]
            {
                #signer_seeds_logic

                let __sys_info = accounts.iter()
                    .find(|a| a.key() == naclac_lang::prelude::SYSTEM_PROGRAM_ID)
                    .ok_or(naclac_lang::prelude::NaclacError::ProgramIdMismatch.err(0))?;

                let __payer_info = &#payer_ident;

                if info.data_is_empty() {
                    let create_account_ix = naclac_lang::prelude::pinocchio_system::instructions::CreateAccount::with_minimum_balance(
                        &__payer_info.view,
                        &info.view,
                        (#space_tokens) as u64,
                        program_id.as_address(),
                        None,
                    ).map_err(|e| e)?;

                    create_account_ix.invoke_signed(pinocchio_signer_seeds)?;
                }

                // SAFETY: We are initializing the account discriminator.
                unsafe {
                    let ptr = info.view.data_ptr() as *mut u8;
                    let slice = core::slice::from_raw_parts_mut(ptr, 8);
                    slice.copy_from_slice(&#inner_type::DISCRIMINATOR);
                }
            }
        }
    }
}
