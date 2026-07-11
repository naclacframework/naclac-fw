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
            // Transparently unwrap Box<...> — a boxed field (Box<Account<T>>) must
            // resolve to the same inner T as an unboxed Account<T> field.
            if segment.ident == "Box" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    for arg in &args.args {
                        if let syn::GenericArgument::Type(inner) = arg {
                            return extract_inner_type(inner);
                        }
                    }
                }
            }
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

fn get_account_reference(expr: &syn::Expr, all_fields: &[ParsedField]) -> TokenStream {
    let expr_str = quote! { #expr }.to_string().replace(" ", "");
    if let Some(pos) = all_fields.iter().position(|f| f.ident == expr_str) {
        quote! { &accounts[#pos] }
    } else {
        quote! { &#expr }
    }
}

fn signer_seeds_tokens(
    field: &ParsedField,
    all_fields: &[ParsedField],
    is_zero_copy: bool,
) -> TokenStream {
    if let Some(seed_array) = &field.pda_seed {
        let field_ident = &field.ident;
        let seed_bindings: Vec<TokenStream> = seed_array.elems.iter().enumerate().map(|(i, expr)| {
            let ident = format_ident!("__seed_{}_{}", field_ident, i);
            let raw_str = quote! { #expr }.to_string();
            let expr_str: String = raw_str.chars().filter(|c| !c.is_whitespace()).collect();

            // Same classification as security.rs's PDA-verification seed binding:
            // a genuine struct-field path on a zero-copy account (e.g.
            // `pool_state.bump`) is reached through `Deref<Target = T>`, but a
            // method call on the whole account (e.g. `.as_ref()`) is not a
            // field path and must fall through to the generic normalization
            // below instead. Kept in sync with `security.rs::generate_security_checks`
            // deliberately, rather than as an independent reimplementation, so
            // the two codegens can't silently diverge on the same seed again.
            let binding_str = expr_str.clone();
            for other_field in all_fields {
                let other_name = other_field.ident.to_string();
                if crate::instruction::security::is_zero_copy_account_field(&other_field.type_str, is_zero_copy)
                   && binding_str.starts_with(&format!("{}.", other_name))
                {
                    let field_path = binding_str[other_name.len() + 1..].trim();
                    if field_path.contains('(') {
                        break;
                    }
                    let other_ident = &other_field.ident;
                    let field_path_expr: syn::Expr = syn::parse_str(field_path).unwrap();
                    return quote! { let #ident = &#other_ident.#field_path_expr; };
                }
            }

            let mut binding_str = expr_str.clone();
            let is_key_access = binding_str.contains(".address()");
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
                return quote! { let #ident = naclac_lang::prelude::ToAddress::address(&(#binding_expr)); };
            }

            quote! { let #ident = &#binding_expr; }
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

            // signer_seeds (Solana-program format) is only needed on the non-pinocchio path.
            // In pinocchio mode we use pinocchio_seeds / pinocchio_signer_seeds exclusively.
            #[cfg(not(feature = "pinocchio"))]
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
            #[cfg(not(feature = "pinocchio"))]
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
pub fn generate_init_cpi(
    field: &ParsedField,
    all_fields: &[ParsedField],
    is_zero_copy: bool,
) -> TokenStream {
    let init_config = match &field.init_config {
        Some(config) => config,
        None => return quote! {},
    };

    let payer_str = &init_config.payer;
    let payer_ident = format_ident!("{}", payer_str);
    let signer_seeds_logic = signer_seeds_tokens(field, all_fields, is_zero_copy);

    // Compile-time static index lookup for system program
    let system_program_idx = all_fields.iter().position(|f| {
        let clean = f.type_str.chars().filter(|c| !c.is_whitespace()).collect::<String>();
        clean.contains("System") || clean.contains("SYSTEM_PROGRAM_ID")
    }).expect("Naclac Error: system_program (Program<System>) must be defined in the Accounts struct for account initialization.");

    if let Some(decimals_expr) = &field.mint_decimals {
        let authority_expr = field
            .mint_authority
            .as_ref()
            .expect("Naclac: mint::authority must be specified alongside mint::decimals");

        let authority_ref = get_account_reference(authority_expr, all_fields);

        let (non_pinocchio_freeze_logic, pinocchio_freeze_logic) = match &field
            .mint_freeze_authority
        {
            Some(expr) => {
                let ref_tokens = get_account_reference(expr, all_fields);
                (
                    quote! {
                        let mut __freeze_key = [0u8; 32];
                        __freeze_key.copy_from_slice(naclac_lang::prelude::ToAddress::address(#ref_tokens).as_ref());
                        let __has_freeze = 1u8;
                    },
                    quote! {
                        let __freeze_key = naclac_lang::prelude::ToAddress::address(#ref_tokens);
                        let __freeze_key_view = Some(__freeze_key.as_address());
                    },
                )
            }
            None => (
                quote! {
                    let __freeze_key = [0u8; 32];
                    let __has_freeze = 0u8;
                },
                quote! {
                    let __freeze_key_view = None;
                },
            ),
        };

        let token_program_idx = all_fields.iter().position(|f| {
            let clean = f.type_str.chars().filter(|c| !c.is_whitespace()).collect::<String>();
            clean.contains("Token") || clean.contains("Token2022") || clean.contains("TokenInterface")
        }).expect("Naclac Error: token_program must be defined in the Accounts struct for mint initialization.");

        let idx = field.index;
        return quote! {
            if !info.data_is_empty() {
                return Err(naclac_lang::prelude::NaclacError::AccountAlreadyInitialized.err(#idx));
            }
            if info.data_is_empty() {
                #[cfg(not(feature = "pinocchio"))]
                {
                    let rent = naclac_lang::solana_program::rent::Rent::get()?;
                    let lamports = rent.minimum_balance(82_usize);

                    #signer_seeds_logic

                    let __sys_info = &accounts[#system_program_idx];
                    if (__sys_info).address() != naclac_lang::prelude::SYSTEM_PROGRAM_ID {
                        return Err(naclac_lang::prelude::NaclacError::ProgramIdMismatch.err(#system_program_idx));
                    }

                    let __tok_prog_info = &accounts[#token_program_idx];
                    let __tok_prog_key = (__tok_prog_info).address();
                    if __tok_prog_key != naclac_lang::prelude::TOKEN_PROGRAM_ID && __tok_prog_key != naclac_lang::prelude::TOKEN_2022_PROGRAM_ID {
                        return Err(naclac_lang::prelude::NaclacError::ProgramIdMismatch.err(#token_program_idx));
                    }

                    let __payer_info_wrapper = naclac_lang::prelude::ToAccountInfo::to_account_info(&#payer_ident);
                    let __payer_info = &__payer_info_wrapper;

                    let payer_pubkey = (__payer_info).address();
                    let info_pubkey = (info).address();
                    let sys_pubkey = (__sys_info).address();

                    #[repr(C)]
                    pub struct StableAccountMeta {
                        pub pubkey: naclac_lang::solana_program::pubkey::Pubkey,
                        pub is_writable: bool,
                        pub is_signer: bool,
                    }

                    #[repr(C)]
                    pub struct StableVec<T> {
                        pub ptr: *const T,
                        pub len: usize,
                        pub cap: usize,
                    }

                    #[repr(C)]
                    pub struct StableInstruction {
                        pub accounts: StableVec<StableAccountMeta>,
                        pub data: StableVec<u8>,
                        pub program_id: naclac_lang::solana_program::pubkey::Pubkey,
                    }

                    let stable_accounts = [
                        StableAccountMeta {
                            pubkey: payer_pubkey,
                            is_writable: true,
                            is_signer: true,
                        },
                        StableAccountMeta {
                            pubkey: info_pubkey,
                            is_writable: true,
                            is_signer: true,
                        },
                    ];

                    let mut ix_data = [0u8; 52];
                    ix_data[0..4].copy_from_slice(&0u32.to_le_bytes()); // CreateAccount discriminator = 0
                    ix_data[4..12].copy_from_slice(&lamports.to_le_bytes());
                    ix_data[12..20].copy_from_slice(&82_u64.to_le_bytes());
                    ix_data[20..52].copy_from_slice(__tok_prog_key.as_ref());

                    let stable_ix = StableInstruction {
                        accounts: StableVec {
                            ptr: stable_accounts.as_ptr(),
                            len: stable_accounts.len(),
                            cap: stable_accounts.len(),
                        },
                        data: StableVec {
                            ptr: ix_data.as_ptr(),
                            len: ix_data.len(),
                            cap: ix_data.len(),
                        },
                        program_id: sys_pubkey,
                    };

                    let account_infos = unsafe { [
                        __payer_info.to_lifetime(),
                        info.to_lifetime(),
                        __sys_info.to_lifetime(),
                    ] };

                    extern "C" {
                        fn sol_invoke_signed_rust(
                            instruction_addr: *const u8,
                            account_infos_addr: *const u8,
                            account_infos_len: u64,
                            signers_seeds_addr: *const u8,
                            signers_seeds_len: u64,
                        ) -> u64;
                    }

                    unsafe {
                        let result = sol_invoke_signed_rust(
                            &stable_ix as *const StableInstruction as *const u8,
                            account_infos.as_ptr() as *const u8,
                            account_infos.len() as u64,
                            signer_seeds.as_ptr() as *const u8,
                            signer_seeds.len() as u64,
                        );
                        if result != 0 {
                            return Err(naclac_lang::solana_program::program_error::ProgramError::Custom(result as u32));
                        }
                    }

                    let __authority_info = naclac_lang::prelude::ToAccountInfo::to_account_info(#authority_ref);
                    let __authority_key = (&__authority_info).address();

                    #non_pinocchio_freeze_logic

                    let mut __ix_data = naclac_lang::prelude::vec![20u8]; // InitializeMint2
                    __ix_data.push(#decimals_expr);
                    __ix_data.extend_from_slice(__authority_key.as_ref());
                    __ix_data.push(__has_freeze);
                    __ix_data.extend_from_slice(&__freeze_key);

                    let __init_ix = naclac_lang::solana_program::instruction::Instruction {
                        program_id: __tok_prog_key,
                        accounts: naclac_lang::prelude::vec![
                            naclac_lang::solana_program::instruction::AccountMeta::new((info).address(), false),
                        ],
                        data: __ix_data,
                    };

                    unsafe {
                        naclac_lang::solana_program::program::invoke_signed(
                            &__init_ix,
                            &[info.to_lifetime(), __tok_prog_info.to_lifetime()],
                            signer_seeds,
                        )?;
                    }
                }

                #[cfg(feature = "pinocchio")]
                {
                    #signer_seeds_logic

                    let __sys_info = &accounts[#system_program_idx];
                    if __sys_info.address() != naclac_lang::prelude::SYSTEM_PROGRAM_ID {
                        return Err(naclac_lang::prelude::NaclacError::ProgramIdMismatch.err(#system_program_idx));
                    }

                    let __tok_prog_info = &accounts[#token_program_idx];
                    let __tok_prog_key = __tok_prog_info.address();
                    if __tok_prog_key != naclac_lang::prelude::TOKEN_PROGRAM_ID && __tok_prog_key != naclac_lang::prelude::TOKEN_2022_PROGRAM_ID {
                        return Err(naclac_lang::prelude::NaclacError::ProgramIdMismatch.err(#token_program_idx));
                    }

                    let __payer_view = naclac_lang::prelude::ToAccountInfo::to_account_info(&#payer_ident).view;

                    let __lamports = {
                        const __STORAGE_OVERHEAD: u64 = 128;
                        const __LAMPORTS_PER_BYTE: u64 = 6960;
                        (__STORAGE_OVERHEAD + 82u64).wrapping_mul(__LAMPORTS_PER_BYTE)
                    };

                    naclac_lang::prelude::system_program::create_account_unchecked(
                        &__payer_view,
                        &info.view,
                        __lamports,
                        82,
                        &__tok_prog_key,
                        pinocchio_signer_seeds,
                    )?;

                    let __authority_info = naclac_lang::prelude::ToAccountInfo::to_account_info(#authority_ref);
                    let __authority_key = (&__authority_info).address();

                    #pinocchio_freeze_logic

                    let __init_ix = naclac_lang::prelude::pinocchio_token_2022::instructions::InitializeMint2 {
                        token_program: __tok_prog_key.as_address(),
                        mint: &info.view,
                        decimals: #decimals_expr,
                        mint_authority: __authority_key.as_address(),
                        freeze_authority: __freeze_key_view,
                    };
                    __init_ix.invoke()?;
                }
            }
        };
    }

    if let Some(mint_expr) = &field.token_mint {
        let authority_expr = field
            .token_authority
            .as_ref()
            .expect("Naclac: token::authority must be specified alongside token::mint");

        let mint_ref = get_account_reference(mint_expr, all_fields);
        let authority_ref = get_account_reference(authority_expr, all_fields);

        let token_program_idx = all_fields.iter().position(|f| {
            let clean = f.type_str.chars().filter(|c| !c.is_whitespace()).collect::<String>();
            clean.contains("Token") || clean.contains("Token2022") || clean.contains("TokenInterface")
        }).expect("Naclac Error: token_program must be defined in the Accounts struct for token account initialization.");

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

                    let __sys_info = &accounts[#system_program_idx];
                    if (__sys_info).address() != naclac_lang::prelude::SYSTEM_PROGRAM_ID {
                        return Err(naclac_lang::prelude::NaclacError::ProgramIdMismatch.err(#system_program_idx));
                    }

                    let __tok_prog_info = &accounts[#token_program_idx];
                    let __tok_prog_key = (__tok_prog_info).address();
                    if __tok_prog_key != naclac_lang::prelude::TOKEN_PROGRAM_ID && __tok_prog_key != naclac_lang::prelude::TOKEN_2022_PROGRAM_ID {
                        return Err(naclac_lang::prelude::NaclacError::ProgramIdMismatch.err(#token_program_idx));
                    }

                    let __payer_info_wrapper = naclac_lang::prelude::ToAccountInfo::to_account_info(&#payer_ident);
                    let __payer_info = &__payer_info_wrapper;

                    let payer_pubkey = (__payer_info).address();
                    let info_pubkey = (info).address();
                    let sys_pubkey = (__sys_info).address();

                    // Stack-Allocated Zero-Copy CPI
                    #[repr(C)]
                    pub struct StableAccountMeta {
                        pub pubkey: naclac_lang::solana_program::pubkey::Pubkey,
                        pub is_writable: bool,
                        pub is_signer: bool,
                    }

                    #[repr(C)]
                    pub struct StableVec<T> {
                        pub ptr: *const T,
                        pub len: usize,
                        pub cap: usize,
                    }

                    #[repr(C)]
                    pub struct StableInstruction {
                        pub accounts: StableVec<StableAccountMeta>,
                        pub data: StableVec<u8>,
                        pub program_id: naclac_lang::solana_program::pubkey::Pubkey,
                    }

                    let stable_accounts = [
                        StableAccountMeta {
                            pubkey: payer_pubkey,
                            is_writable: true,
                            is_signer: true,
                        },
                        StableAccountMeta {
                            pubkey: info_pubkey,
                            is_writable: true,
                            is_signer: true,
                        },
                    ];

                    let mut ix_data = [0u8; 52];
                    ix_data[0..4].copy_from_slice(&0u32.to_le_bytes()); // CreateAccount discriminator = 0
                    ix_data[4..12].copy_from_slice(&lamports.to_le_bytes());
                    ix_data[12..20].copy_from_slice(&165_u64.to_le_bytes());
                    ix_data[20..52].copy_from_slice(__tok_prog_key.as_ref());

                    let stable_ix = StableInstruction {
                        accounts: StableVec {
                            ptr: stable_accounts.as_ptr(),
                            len: stable_accounts.len(),
                            cap: stable_accounts.len(),
                        },
                        data: StableVec {
                            ptr: ix_data.as_ptr(),
                            len: ix_data.len(),
                            cap: ix_data.len(),
                        },
                        program_id: sys_pubkey,
                    };

                    let account_infos = unsafe { [
                        __payer_info.to_lifetime(),
                        info.to_lifetime(),
                        __sys_info.to_lifetime(),
                    ] };

                    extern "C" {
                        fn sol_invoke_signed_rust(
                            instruction_addr: *const u8,
                            account_infos_addr: *const u8,
                            account_infos_len: u64,
                            signers_seeds_addr: *const u8,
                            signers_seeds_len: u64,
                        ) -> u64;
                    }

                    unsafe {
                        let result = sol_invoke_signed_rust(
                            &stable_ix as *const StableInstruction as *const u8,
                            account_infos.as_ptr() as *const u8,
                            account_infos.len() as u64,
                            signer_seeds.as_ptr() as *const u8,
                            signer_seeds.len() as u64,
                        );
                        if result != 0 {
                            return Err(naclac_lang::solana_program::program_error::ProgramError::Custom(result as u32));
                        }
                    }

                    let __mint_info_wrapper = naclac_lang::prelude::ToAccountInfo::to_account_info(#mint_ref);
                    let __mint_info = &__mint_info_wrapper;
                    let __authority_info = naclac_lang::prelude::ToAccountInfo::to_account_info(#authority_ref);
                    let __authority_key = (&__authority_info).address();

                    let mut __ix_data = [0u8; 33];
                    __ix_data[0] = 18; // InitializeAccount3 discriminant
                    __ix_data[1..].copy_from_slice(__authority_key.as_ref());

                    let __init_ix = naclac_lang::solana_program::instruction::Instruction {
                        program_id: __tok_prog_key,
                        accounts: naclac_lang::prelude::vec![
                            naclac_lang::solana_program::instruction::AccountMeta::new((info).address(), false),
                            naclac_lang::solana_program::instruction::AccountMeta::new_readonly((__mint_info).address(), false),
                        ],
                        data: __ix_data.to_vec(),
                    };

                    unsafe {
                        naclac_lang::solana_program::program::invoke_signed(
                            &__init_ix,
                            &[info.to_lifetime(), __mint_info.to_lifetime(), __tok_prog_info.to_lifetime()],
                            signer_seeds,
                        )?;
                    }
                }

                #[cfg(feature = "pinocchio")]
                {
                    #signer_seeds_logic

                    let __sys_info = &accounts[#system_program_idx];
                    if __sys_info.address() != naclac_lang::prelude::SYSTEM_PROGRAM_ID {
                        return Err(naclac_lang::prelude::NaclacError::ProgramIdMismatch.err(#system_program_idx));
                    }

                    let __tok_prog_info = &accounts[#token_program_idx];
                    let __tok_prog_key = __tok_prog_info.address();
                    if __tok_prog_key != naclac_lang::prelude::TOKEN_PROGRAM_ID && __tok_prog_key != naclac_lang::prelude::TOKEN_2022_PROGRAM_ID {
                        return Err(naclac_lang::prelude::NaclacError::ProgramIdMismatch.err(#token_program_idx));
                    }

                    let __is_token_2022 = __tok_prog_key == naclac_lang::prelude::TOKEN_2022_PROGRAM_ID;
                    let __payer_view = naclac_lang::prelude::ToAccountInfo::to_account_info(&#payer_ident).view;

                    let __lamports = {
                        // Const-rent: eliminates Rent::get() sysvar call from binary.
                        // Formula: (ACCOUNT_STORAGE_OVERHEAD + space) * DEFAULT_LAMPORTS_PER_BYTE
                        // ACCOUNT_STORAGE_OVERHEAD = 128, DEFAULT_LAMPORTS_PER_BYTE = 6960
                        // Same approach as Anchor lang-v2 `const-rent` feature.
                        const __STORAGE_OVERHEAD: u64 = 128;
                        const __LAMPORTS_PER_BYTE: u64 = 6960;
                        (__STORAGE_OVERHEAD + 165u64).wrapping_mul(__LAMPORTS_PER_BYTE)
                    };

                    naclac_lang::prelude::system_program::create_account_unchecked(
                        &__payer_view,
                        &info.view,
                        __lamports,
                        165,
                        &__tok_prog_key,
                        pinocchio_signer_seeds,
                    )?;

                    let __mint_info_wrapper = naclac_lang::prelude::ToAccountInfo::to_account_info(#mint_ref);
                    let __mint_info = &__mint_info_wrapper;
                    let __authority_info = naclac_lang::prelude::ToAccountInfo::to_account_info(#authority_ref);
                    let __authority_key = (&__authority_info).address();

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
    let check_init_tokens = if field.is_init_if_needed {
        quote! {}
    } else {
        quote! {
            if !__needs_init {
                return Err(naclac_lang::prelude::NaclacError::AccountAlreadyInitialized.err(#idx));
            }
        }
    };

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
                // Read first 8 bytes as u64 and check == 0 (all-zero discriminator = uninit).
                // Avoids bounds-check panic string from data[0..8] indexing.
                // SAFETY: data_is_empty() was false above, and Solana accounts are
                // always >= 8 bytes when non-empty (minimum data size is 8 + header).
                let __data_ptr = info.view.data_ptr();
                let __data_len = info.view.data_len();
                __data_len >= 8 && unsafe { *(__data_ptr as *const u64) } == 0
            }
        };

        #check_init_tokens

        if __needs_init {
            #[cfg(not(feature = "pinocchio"))]
            {
                let rent = naclac_lang::solana_program::rent::Rent::get()?;
                let lamports = rent.minimum_balance((#space_tokens) as usize);

                #signer_seeds_logic

                let __sys_info = &accounts[#system_program_idx];
                if (__sys_info).address() != naclac_lang::prelude::SYSTEM_PROGRAM_ID {
                    return Err(naclac_lang::prelude::NaclacError::ProgramIdMismatch.err(#system_program_idx));
                }

                let __payer_info_wrapper = naclac_lang::prelude::ToAccountInfo::to_account_info(&#payer_ident);
                let __payer_info = &__payer_info_wrapper;

                if info.data_is_empty() {
                    let payer_pubkey = (__payer_info).address();
                    let info_pubkey = (info).address();
                    let sys_pubkey = (__sys_info).address();

                    #[repr(C)]
                    pub struct StableAccountMeta {
                        pub pubkey: naclac_lang::solana_program::pubkey::Pubkey,
                        pub is_writable: bool,
                        pub is_signer: bool,
                    }

                    #[repr(C)]
                    pub struct StableVec<T> {
                        pub ptr: *const T,
                        pub len: usize,
                        pub cap: usize,
                    }

                    #[repr(C)]
                    pub struct StableInstruction {
                        pub accounts: StableVec<StableAccountMeta>,
                        pub data: StableVec<u8>,
                        pub program_id: naclac_lang::solana_program::pubkey::Pubkey,
                    }

                    extern "C" {
                        fn sol_invoke_signed_rust(
                            instruction_addr: *const u8,
                            account_infos_addr: *const u8,
                            account_infos_len: u64,
                            signers_seeds_addr: *const u8,
                            signers_seeds_len: u64,
                        ) -> u64;
                    }

                    let mut ix_data = [0u8; 52];
                    ix_data[0..4].copy_from_slice(&0u32.to_le_bytes()); // CreateAccount discriminator = 0
                    ix_data[4..12].copy_from_slice(&lamports.to_le_bytes());
                    ix_data[12..20].copy_from_slice(&((#space_tokens) as u64).to_le_bytes());
                    ix_data[20..52].copy_from_slice(program_id.as_ref());

                    let accounts = [
                        StableAccountMeta {
                            pubkey: payer_pubkey,
                            is_writable: true,
                            is_signer: true,
                        },
                        StableAccountMeta {
                            pubkey: info_pubkey,
                            is_writable: true,
                            is_signer: true,
                        },
                    ];

                    let stable_ix = StableInstruction {
                        accounts: StableVec {
                            ptr: accounts.as_ptr(),
                            len: accounts.len(),
                            cap: accounts.len(),
                        },
                        data: StableVec {
                            ptr: ix_data.as_ptr(),
                            len: ix_data.len(),
                            cap: ix_data.len(),
                        },
                        program_id: sys_pubkey,
                    };

                    let account_infos = unsafe { [
                        __payer_info.to_lifetime(),
                        info.to_lifetime(),
                        __sys_info.to_lifetime(),
                    ] };

                    unsafe {
                        let result = sol_invoke_signed_rust(
                            &stable_ix as *const StableInstruction as *const u8,
                            account_infos.as_ptr() as *const u8,
                            account_infos.len() as u64,
                            signer_seeds.as_ptr() as *const u8,
                            signer_seeds.len() as u64,
                        );
                        if result != 0 {
                            return Err(naclac_lang::solana_program::program_error::ProgramError::Custom(result as u32));
                        }
                    }
                }

                let mut __account_data = info.try_borrow_mut_data()?;
                __account_data[0..8].copy_from_slice(&#inner_type::DISCRIMINATOR);
            }

            #[cfg(feature = "pinocchio")]
            {
                #signer_seeds_logic

                let __sys_info = &accounts[#system_program_idx];
                if __sys_info.address() != naclac_lang::prelude::SYSTEM_PROGRAM_ID {
                    return Err(naclac_lang::prelude::NaclacError::ProgramIdMismatch.err(#system_program_idx));
                }

                let __payer_info = &#payer_ident;

                if info.data_is_empty() {
                    // Const-rent formula: eliminates Rent::get() sysvar call.
                    // (ACCOUNT_STORAGE_OVERHEAD + space) * DEFAULT_LAMPORTS_PER_BYTE
                    const __STORAGE_OVERHEAD: u64 = 128;
                    const __LAMPORTS_PER_BYTE: u64 = 6960;
                    let __space = (#space_tokens) as u64;
                    let __lamports = (__STORAGE_OVERHEAD + __space).wrapping_mul(__LAMPORTS_PER_BYTE);
                    naclac_lang::prelude::system_program::create_account_unchecked(
                        &__payer_info.view,
                        &info.view,
                        __lamports,
                        __space,
                        program_id,
                        pinocchio_signer_seeds,
                    )?;
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
