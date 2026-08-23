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
    let is_box =
        matches!(ty, Type::Path(p) if p.path.segments.last().is_some_and(|s| s.ident == "Box"));
    let unboxed = if is_box {
        crate::type_classify::first_generic_type(ty).unwrap_or(ty)
    } else {
        ty
    };
    if crate::type_classify::is_account_wrapper(unboxed) {
        if let Some(inner) = crate::type_classify::first_generic_type(unboxed) {
            return quote! { #inner };
        }
    }
    quote! { #ty }
}

/// Returns the marker ident (e.g. `System`, `Token`, `Token2022`,
/// `AssociatedToken`, `TokenInterface`) for a field typed `Program<Marker>`
/// or `Interface<Marker>`, or `None` for any other type.
pub(crate) fn program_marker_ident(ty: &Type) -> Option<String> {
    if let Type::Path(type_path) = ty {
        let segment = type_path.path.segments.last()?;
        if segment.ident == "Program" || segment.ident == "Interface" {
            if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                for arg in &args.args {
                    if let syn::GenericArgument::Type(Type::Path(inner_path)) = arg {
                        return inner_path.path.segments.last().map(|s| s.ident.to_string());
                    }
                }
            }
        }
    }
    None
}

/// Resolves which token-program field an `init`'d mint/token-account/ATA
/// should CPI into. `token::program = <field>` (if present) always wins —
/// needed the moment a struct has more than one token-program-shaped field
/// (e.g. a hardcoded `Program<Token2022>` for one account and a generic
/// `Interface<TokenInterface>` for another), since without an explicit
/// pointer there's no way to tell which one a given `init` actually means.
/// Falls back to the first `Program<Token>`/`Program<Token2022>`/
/// `Interface<TokenInterface>` field in the struct otherwise, preserving
/// existing single-token-program-field behavior unchanged.
pub(crate) fn resolve_token_program_idx(
    field: &ParsedField,
    all_fields: &[ParsedField],
) -> Result<usize, TokenStream> {
    if let Some(syn::Expr::Path(expr_path)) = &field.token_program {
        if let Some(ident) = expr_path.path.get_ident() {
            if let Some(pos) = all_fields.iter().position(|f| &f.ident == ident) {
                return Ok(pos);
            }
        }
    }
    all_fields
        .iter()
        .position(|f| {
            matches!(
                program_marker_ident(&f.ty).as_deref(),
                Some("Token") | Some("Token2022") | Some("TokenInterface")
            )
        })
        .ok_or_else(|| {
            syn::Error::new_spanned(
                &field.ident,
                "Naclac Error: token_program must be defined in the Accounts struct for account \
                 initialization (or point at one explicitly via `token::program = <field>`).",
            )
            .to_compile_error()
        })
}

fn get_account_reference(expr: &syn::Expr, all_fields: &[ParsedField]) -> TokenStream {
    if let syn::Expr::Path(expr_path) = expr {
        if let Some(ident) = expr_path.path.get_ident() {
            if let Some(pos) = all_fields.iter().position(|f| &f.ident == ident) {
                return quote! { &accounts[#pos] };
            }
        }
    }
    quote! { &#expr }
}

fn signer_seeds_tokens(
    field: &ParsedField,
    all_fields: &[ParsedField],
    is_zero_copy: bool,
) -> TokenStream {
    if let Some(seed_array) = &field.pda_seed {
        let field_ident = &field.ident;
        let seed_bindings: Vec<TokenStream> = seed_array
            .elems
            .iter()
            .enumerate()
            .map(|(i, expr)| {
                let ident = format_ident!("__seed_{}_{}", field_ident, i);
                crate::instruction::security::seed_binding_tokens(
                    expr,
                    &ident,
                    field.index,
                    all_fields,
                    is_zero_copy,
                )
            })
            .collect();
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

/// Same as `signer_seeds_tokens`, but for a field referenced only as
/// `payer = <field>` on a *different* field's `init`/`init_if_needed` —
/// resolves `payer_field`'s own seeds directly rather than relying on an
/// `expected_bump` local, which the surrounding codegen only ever defines
/// for the field actually being initialized, never for whatever field
/// `payer` happens to name. Needed for a `payer` that's itself a
/// program-owned PDA (not a real wallet) — e.g. a lamport source raw-funded
/// earlier in the same instruction, which must sign this CPI via its own
/// seeds to authorize spending its lamports as rent.
///
/// Only `PdaBump::Explicit` is supported: an auto-derived `bump` on the
/// payer field would need this codegen to know how that field's own bump
/// was independently verified elsewhere in the struct, which isn't tracked
/// here — every real caller so far supplies an explicit bump anyway (naclac
/// never runs an on-chain bump-search loop), so this isn't a regression
/// against anything currently expressible.
fn payer_signer_seeds_tokens(
    payer_field: &ParsedField,
    all_fields: &[ParsedField],
    is_zero_copy: bool,
) -> TokenStream {
    let seed_array = match &payer_field.pda_seed {
        Some(seed_array) => seed_array,
        None => {
            return quote! { let __payer_signer_seeds: &[&[&[u8]]] = &[]; };
        }
    };
    let bump_expr = match &payer_field.pda_bump {
        Some(crate::instruction::parser::PdaBump::Explicit(expr)) => quote! { #expr },
        _ => {
            return syn::Error::new_spanned(
                &payer_field.ident,
                "Naclac Error: a `payer` field with `seeds = [...]` referenced by an ATA \
                 `init_if_needed` with `extra_accounts` must also have an explicit `bump = <expr>` \
                 — auto-derived `bump` isn't supported as a payer seed source yet.",
            )
            .to_compile_error();
        }
    };

    let field_ident = &payer_field.ident;
    let seed_bindings: Vec<TokenStream> = seed_array
        .elems
        .iter()
        .enumerate()
        .map(|(i, expr)| {
            let ident = format_ident!("__payer_seed_{}_{}", field_ident, i);
            crate::instruction::security::seed_binding_tokens(
                expr,
                &ident,
                payer_field.index,
                all_fields,
                is_zero_copy,
            )
        })
        .collect();
    let seed_refs: Vec<TokenStream> = seed_array
        .elems
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let ident = format_ident!("__payer_seed_{}_{}", field_ident, i);
            quote! { naclac_lang::prelude::AsRefByteSlice::as_ref_byte_slice(&#ident) }
        })
        .collect();

    quote! {
        let __payer_bump_arr = [#bump_expr];
        #(#seed_bindings)*
        let __payer_signer_seeds: &[&[&[u8]]] = &[&[
            #(#seed_refs),*,
            &__payer_bump_arr
        ]];
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

    if field.extra_accounts.is_some() && field.associated_token_mint.is_none() {
        return syn::Error::new_spanned(
            &field.ident,
            "Naclac Error: `extra_accounts` is only supported on an `init_if_needed` \
             associated-token field (one with `associated_token::mint = ...`) — every other \
             `init` path (`mint::`, `token::`, plain PDA `init`) doesn't wire it in yet.",
        )
        .to_compile_error();
    }

    // Resolved by raw index into the pre-walked accounts slice (same as
    // `mint_ref`/`authority_ref` below), not by referencing a previously-built
    // typed local variable — this is what lets `payer` be declared anywhere in
    // the Accounts struct relative to the account it funds, matching whatever
    // order the real on-chain account list requires instead of forcing payer
    // to come first.
    let payer_ref = get_account_reference(&init_config.payer, all_fields);
    let signer_seeds_logic = signer_seeds_tokens(field, all_fields, is_zero_copy);

    // Compile-time static index lookup for system program
    let system_program_idx = match all_fields
        .iter()
        .position(|f| program_marker_ident(&f.ty).as_deref() == Some("System"))
    {
        Some(idx) => idx,
        None => {
            return syn::Error::new_spanned(
                &field.ident,
                "Naclac Error: system_program (Program<System>) must be defined in the Accounts \
                 struct for account initialization.",
            )
            .to_compile_error();
        }
    };

    if let Some(decimals_expr) = &field.mint_decimals {
        let authority_expr = match field.mint_authority.as_ref() {
            Some(expr) => expr,
            None => {
                return syn::Error::new_spanned(
                    &field.ident,
                    "Naclac Error: `mint::authority` must be specified alongside `mint::decimals`.",
                )
                .to_compile_error();
            }
        };

        let authority_ref = get_account_reference(authority_expr, all_fields);

        let freeze_authority_tokens = match &field.mint_freeze_authority {
            Some(expr) => {
                let ref_tokens = get_account_reference(expr, all_fields);
                quote! { Some(naclac_lang::prelude::ToAddress::address(#ref_tokens)) }
            }
            None => quote! { None },
        };

        let token_program_idx = match resolve_token_program_idx(field, all_fields) {
            Ok(idx) => idx,
            Err(err) => return err,
        };

        let idx = field.index;
        let existence_check = if field.is_init_if_needed {
            quote! {}
        } else {
            quote! {
                if !info.data_is_empty() {
                    return Err(naclac_lang::prelude::NaclacError::AccountAlreadyInitialized.err(#idx));
                }
            }
        };
        return quote! {
            #existence_check
            if info.data_is_empty() {
                #signer_seeds_logic
                #[cfg(feature = "pinocchio")]
                let signer_seeds: &[&[&[u8]]] = &[];

                let __sys_info = &accounts[#system_program_idx];
                if (__sys_info).address() != naclac_lang::prelude::SYSTEM_PROGRAM_ID {
                    return Err(naclac_lang::prelude::NaclacError::ProgramIdMismatch.err(#system_program_idx));
                }

                let __tok_prog_info = &accounts[#token_program_idx];
                let __tok_prog_key = (__tok_prog_info).address();
                if __tok_prog_key != naclac_lang::prelude::TOKEN_PROGRAM_ID && __tok_prog_key != naclac_lang::prelude::TOKEN_2022_PROGRAM_ID {
                    return Err(naclac_lang::prelude::NaclacError::ProgramIdMismatch.err(#token_program_idx));
                }

                let __payer_info = naclac_lang::prelude::ToAccountInfo::to_account_info(#payer_ref);
                let __authority_info = naclac_lang::prelude::ToAccountInfo::to_account_info(#authority_ref);
                let __authority_key = naclac_lang::prelude::ToAddress::address(&__authority_info);
                let __freeze_authority: Option<naclac_lang::prelude::Address> = #freeze_authority_tokens;

                #[cfg(not(feature = "pinocchio"))]
                {
                    let rent = naclac_lang::solana_program::rent::Rent::get()?;
                    let lamports = rent.minimum_balance(82_usize);

                    naclac_lang::prelude::system_program::create_account_signed(
                        naclac_lang::prelude::CpiHandleMut { info: __payer_info.clone(), _phantom: core::marker::PhantomData },
                        naclac_lang::prelude::CpiHandleMut { info: info.clone(), _phantom: core::marker::PhantomData },
                        naclac_lang::prelude::CpiHandle { info: __tok_prog_info.clone(), _phantom: core::marker::PhantomData },
                        lamports,
                        82,
                        &__tok_prog_key,
                        signer_seeds,
                    )?;
                }
                #[cfg(feature = "pinocchio")]
                {
                    const __STORAGE_OVERHEAD: u64 = 128;
                    const __LAMPORTS_PER_BYTE: u64 = 6960;
                    let __lamports = (__STORAGE_OVERHEAD + 82u64).wrapping_mul(__LAMPORTS_PER_BYTE);
                    naclac_lang::prelude::system_program::create_account_checked_for_init(
                        &__payer_info.view,
                        &info.view,
                        __lamports,
                        82,
                        &__tok_prog_key,
                        pinocchio_signer_seeds,
                    )?;
                }

                naclac_lang::token::initialize_mint_signed(
                    naclac_lang::prelude::CpiHandle { info: __tok_prog_info.clone(), _phantom: core::marker::PhantomData },
                    naclac_lang::prelude::CpiHandleMut { info: info.clone(), _phantom: core::marker::PhantomData },
                    #decimals_expr,
                    &__authority_key,
                    __freeze_authority.as_ref(),
                    signer_seeds,
                )?;
            }
        };
    }

    if let Some(mint_expr) = &field.token_mint {
        let authority_expr = match field.token_authority.as_ref() {
            Some(expr) => expr,
            None => {
                return syn::Error::new_spanned(
                    &field.ident,
                    "Naclac Error: `token::authority` must be specified alongside `token::mint`.",
                )
                .to_compile_error();
            }
        };

        let mint_ref = get_account_reference(mint_expr, all_fields);
        let authority_ref = get_account_reference(authority_expr, all_fields);

        let token_program_idx = match resolve_token_program_idx(field, all_fields) {
            Ok(idx) => idx,
            Err(err) => return err,
        };

        let idx = field.index;
        let existence_check = if field.is_init_if_needed {
            quote! {}
        } else {
            quote! {
                if !info.data_is_empty() {
                    return Err(naclac_lang::prelude::NaclacError::AccountAlreadyInitialized.err(#idx));
                }
            }
        };
        return quote! {
            #existence_check
            if info.data_is_empty() {
                #signer_seeds_logic
                #[cfg(feature = "pinocchio")]
                let signer_seeds: &[&[&[u8]]] = &[];

                let __sys_info = &accounts[#system_program_idx];
                if (__sys_info).address() != naclac_lang::prelude::SYSTEM_PROGRAM_ID {
                    return Err(naclac_lang::prelude::NaclacError::ProgramIdMismatch.err(#system_program_idx));
                }

                let __tok_prog_info = &accounts[#token_program_idx];
                let __tok_prog_key = (__tok_prog_info).address();
                if __tok_prog_key != naclac_lang::prelude::TOKEN_PROGRAM_ID && __tok_prog_key != naclac_lang::prelude::TOKEN_2022_PROGRAM_ID {
                    return Err(naclac_lang::prelude::NaclacError::ProgramIdMismatch.err(#token_program_idx));
                }

                let __payer_info = naclac_lang::prelude::ToAccountInfo::to_account_info(#payer_ref);
                let __mint_info = naclac_lang::prelude::ToAccountInfo::to_account_info(#mint_ref);
                let __authority_info = naclac_lang::prelude::ToAccountInfo::to_account_info(#authority_ref);

                #[cfg(not(feature = "pinocchio"))]
                {
                    let rent = naclac_lang::solana_program::rent::Rent::get()?;
                    let lamports = rent.minimum_balance(165_usize);

                    naclac_lang::prelude::system_program::create_account_signed(
                        naclac_lang::prelude::CpiHandleMut { info: __payer_info.clone(), _phantom: core::marker::PhantomData },
                        naclac_lang::prelude::CpiHandleMut { info: info.clone(), _phantom: core::marker::PhantomData },
                        naclac_lang::prelude::CpiHandle { info: __tok_prog_info.clone(), _phantom: core::marker::PhantomData },
                        lamports,
                        165,
                        &__tok_prog_key,
                        signer_seeds,
                    )?;
                }
                #[cfg(feature = "pinocchio")]
                {
                    const __STORAGE_OVERHEAD: u64 = 128;
                    const __LAMPORTS_PER_BYTE: u64 = 6960;
                    let __lamports = (__STORAGE_OVERHEAD + 165u64).wrapping_mul(__LAMPORTS_PER_BYTE);
                    naclac_lang::prelude::system_program::create_account_checked_for_init(
                        &__payer_info.view,
                        &info.view,
                        __lamports,
                        165,
                        &__tok_prog_key,
                        pinocchio_signer_seeds,
                    )?;
                }

                naclac_lang::token::initialize_account_signed(
                    naclac_lang::prelude::CpiHandle { info: __tok_prog_info.clone(), _phantom: core::marker::PhantomData },
                    naclac_lang::prelude::CpiHandleMut { info: info.clone(), _phantom: core::marker::PhantomData },
                    naclac_lang::prelude::CpiHandle { info: __mint_info, _phantom: core::marker::PhantomData },
                    naclac_lang::prelude::CpiHandle { info: __authority_info, _phantom: core::marker::PhantomData },
                    signer_seeds,
                )?;
            }
        };
    }

    if let Some(ata_mint_expr) = &field.associated_token_mint {
        let ata_authority_expr = match field.associated_token_authority.as_ref() {
            Some(expr) => expr,
            None => {
                return syn::Error::new_spanned(
                    &field.ident,
                    "Naclac Error: `associated_token::authority` must be specified alongside \
                     `associated_token::mint`.",
                )
                .to_compile_error();
            }
        };

        let mint_ref = get_account_reference(ata_mint_expr, all_fields);
        let authority_ref = get_account_reference(ata_authority_expr, all_fields);

        let token_program_idx = match resolve_token_program_idx(field, all_fields) {
            Ok(idx) => idx,
            Err(err) => return err,
        };

        let associated_token_program_idx = match all_fields
            .iter()
            .position(|f| program_marker_ident(&f.ty).as_deref() == Some("AssociatedToken"))
        {
            Some(idx) => idx,
            None => {
                return syn::Error::new_spanned(
                    &field.ident,
                    "Naclac Error: associated_token_program (Program<AssociatedToken>) must be \
                     defined in the Accounts struct for associated token account initialization.",
                )
                .to_compile_error();
            }
        };

        let idx = field.index;

        // Unlike `mint::decimals` (which hand-rolls raw CPI instruction bytes
        // per backend, since there's no shared CreateAccount+InitializeMint
        // helper), ATA creation already has a single dual-backend function —
        // `naclac_lang::associated_token::create`/`create_idempotent` internally
        // branch on `#[cfg(feature = "pinocchio")]` themselves (confirmed by
        // reading `naclac-token/src/associated_token.rs` directly) — so this
        // codegen needs no cfg branches of its own, mirroring the same
        // already-unified-at-the-call-site pattern `realloc.rs` uses for
        // `system_program::transfer`.
        //
        // `init_if_needed` selects `create_idempotent` (no-op if the ATA
        // already exists) instead of the strict `create` (hard error on an
        // existing account) — confirmed against the real `pump_fees.so`
        // bytecode that `claim_social_fee_pda_v2` needs exactly this: a
        // second claim to the same recipient must not fail just because its
        // ATA was already created by a prior claim.
        let existence_check = if field.is_init_if_needed {
            quote! {}
        } else {
            quote! {
                if !info.data_is_empty() {
                    return Err(naclac_lang::prelude::NaclacError::AccountAlreadyInitialized.err(#idx));
                }
            }
        };
        if field.extra_accounts.is_some() && !field.is_init_if_needed {
            return syn::Error::new_spanned(
                &field.ident,
                "Naclac Error: `extra_accounts` on an associated-token `init` field requires \
                 `init_if_needed` (not plain `init`) — only \
                 `create_idempotent_with_extra_accounts_signed` exists, since nothing in this \
                 codebase needs the strict, non-idempotent variant padded.",
            )
            .to_compile_error();
        }

        let extra_accounts_tokens = match &field.extra_accounts {
            Some(extra_array) => {
                let refs: Vec<TokenStream> = extra_array.elems.iter().map(|expr| {
                    let account_ref = get_account_reference(expr, all_fields);
                    quote! {
                        naclac_lang::prelude::CpiHandle {
                            info: naclac_lang::prelude::ToAccountInfo::to_account_info(#account_ref),
                            _phantom: core::marker::PhantomData,
                        }
                    }
                }).collect();
                quote! { &[#(#refs),*] }
            }
            None => quote! { &[] },
        };

        // The payer may itself be a PDA (e.g. a program-owned lamport
        // source, not a real wallet) — resolved the same way
        // `signer_seeds_tokens` resolves `field`'s own seeds, just anchored
        // to whichever field `payer` names instead of `field` itself.
        let payer_field = if let syn::Expr::Path(expr_path) = &init_config.payer {
            expr_path
                .path
                .get_ident()
                .and_then(|ident| all_fields.iter().find(|f| &f.ident == ident))
        } else {
            None
        };
        // `create_idempotent_with_extra_accounts_signed`'s `signer_seeds` is
        // the same backend-uniform `&[&[&[u8]]]` format every other
        // `_signed` helper in this codebase already takes (it converts to
        // pinocchio's own `Signer`/`Seed` types internally on that backend)
        // — no per-backend branching needed here, unlike `signer_seeds_logic`
        // above for the field-being-initialized's own seeds.
        //
        // Only emitted when `extra_accounts` is actually set — `call_tail`
        // is the only consumer of `__payer_signer_seeds`, so emitting this
        // unconditionally would leave an unused-variable warning on every
        // `init_if_needed` associated-token field that doesn't use padding
        // (e.g. `pump_amm::create_pool`'s own `pool_base_token_account`).
        let payer_signer_seeds_logic = if field.extra_accounts.is_some() {
            match payer_field {
                Some(pf) if pf.pda_seed.is_some() => {
                    payer_signer_seeds_tokens(pf, all_fields, is_zero_copy)
                }
                _ => quote! { let __payer_signer_seeds: &[&[&[u8]]] = &[]; },
            }
        } else {
            quote! {}
        };

        let create_call = if field.extra_accounts.is_some() {
            quote! { naclac_lang::associated_token::create_idempotent_with_extra_accounts_signed }
        } else if field.is_init_if_needed {
            quote! { naclac_lang::associated_token::create_idempotent }
        } else {
            quote! { naclac_lang::associated_token::create }
        };
        // `create_idempotent_with_extra_accounts_signed` takes two extra
        // arguments (`extra_accounts`, `signer_seeds`) that `create`/
        // `create_idempotent` don't, so the call-site arguments differ per
        // branch.
        let call_tail = if field.extra_accounts.is_some() {
            quote! { , #extra_accounts_tokens, __payer_signer_seeds }
        } else {
            quote! {}
        };

        return quote! {
            #existence_check

            let __payer_info = naclac_lang::prelude::ToAccountInfo::to_account_info(#payer_ref);
            let __ata_info = naclac_lang::prelude::ToAccountInfo::to_account_info(info);
            let __authority_info = naclac_lang::prelude::ToAccountInfo::to_account_info(#authority_ref);
            let __mint_info = naclac_lang::prelude::ToAccountInfo::to_account_info(#mint_ref);
            let __sys_info = naclac_lang::prelude::ToAccountInfo::to_account_info(&accounts[#system_program_idx]);
            let __tok_prog_info = naclac_lang::prelude::ToAccountInfo::to_account_info(&accounts[#token_program_idx]);
            let __ata_prog_info = naclac_lang::prelude::ToAccountInfo::to_account_info(&accounts[#associated_token_program_idx]);

            if (&__sys_info).address() != naclac_lang::prelude::SYSTEM_PROGRAM_ID {
                return Err(naclac_lang::prelude::NaclacError::ProgramIdMismatch.err(#system_program_idx));
            }
            let __tok_prog_key = (&__tok_prog_info).address();
            if __tok_prog_key != naclac_lang::prelude::TOKEN_PROGRAM_ID && __tok_prog_key != naclac_lang::prelude::TOKEN_2022_PROGRAM_ID {
                return Err(naclac_lang::prelude::NaclacError::ProgramIdMismatch.err(#token_program_idx));
            }
            if (&__ata_prog_info).address() != naclac_lang::prelude::ASSOCIATED_TOKEN_PROGRAM_ID {
                return Err(naclac_lang::prelude::NaclacError::ProgramIdMismatch.err(#associated_token_program_idx));
            }

            #payer_signer_seeds_logic

            #create_call(
                naclac_lang::prelude::CpiHandle { info: __ata_prog_info, _phantom: core::marker::PhantomData },
                naclac_lang::associated_token::AtaCpiAccounts {
                    payer: naclac_lang::prelude::CpiHandleMut { info: __payer_info, _phantom: core::marker::PhantomData },
                    associated_token: naclac_lang::prelude::CpiHandleMut { info: __ata_info, _phantom: core::marker::PhantomData },
                    authority: naclac_lang::prelude::CpiHandle { info: __authority_info, _phantom: core::marker::PhantomData },
                    mint: naclac_lang::prelude::CpiHandle { info: __mint_info, _phantom: core::marker::PhantomData },
                    system_program: naclac_lang::prelude::CpiHandle { info: __sys_info, _phantom: core::marker::PhantomData },
                    token_program: naclac_lang::prelude::CpiHandle { info: __tok_prog_info, _phantom: core::marker::PhantomData },
                }
                #call_tail
            )?;
        };
    }

    let inner_type = extract_inner_type(&field.ty);
    let space_tokens = if let Some(space) = &init_config.space {
        quote! { (#space) }
    } else {
        // `#inner_type::SPACE` (generated by `#[component]`, both backends)
        // rather than `size_of::<#inner_type>()` — the latter is the
        // in-memory Rust struct size, not the real serialized size, and
        // under-allocates for a Borsh component with any `Vec`/`String`/
        // `#[max_len]` field (in-memory: a small pointer+len+cap constant;
        // serialized: the real `#[max_len]`-driven byte length `SPACE`
        // already accounts for). Zero-copy components don't have this
        // divergence (`Vec`/`String` are rejected outright), but `SPACE`
        // is correct there too, so no backend branch is needed here.
        quote! { (#inner_type::SPACE) }
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

                let __payer_info_wrapper = naclac_lang::prelude::ToAccountInfo::to_account_info(#payer_ref);

                if info.data_is_empty() {
                    // Delegates to the crate's own public `system_program::create_account_signed`
                    // instead of hand-rolling a second copy of the same raw CreateAccount CPI —
                    // this used to be an independent, byte-for-byte-identical reimplementation
                    // that could silently drift from the public helper. Mirrors the same
                    // "call the shared dual-backend helper" pattern already used by
                    // `realloc.rs` (`system_program::transfer`) and the `associated_token::mint`
                    // constraint (`associated_token::create`).
                    naclac_lang::prelude::system_program::create_account_signed(
                        naclac_lang::prelude::CpiHandleMut { info: __payer_info_wrapper, _phantom: core::marker::PhantomData },
                        naclac_lang::prelude::CpiHandleMut { info: info.clone(), _phantom: core::marker::PhantomData },
                        naclac_lang::prelude::CpiHandle { info: __sys_info.clone(), _phantom: core::marker::PhantomData },
                        lamports,
                        (#space_tokens) as u64,
                        program_id,
                        signer_seeds,
                    )?;
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

                let __payer_info = #payer_ref;

                if info.data_is_empty() {
                    // Const-rent formula: eliminates Rent::get() sysvar call.
                    // (ACCOUNT_STORAGE_OVERHEAD + space) * DEFAULT_LAMPORTS_PER_BYTE
                    const __STORAGE_OVERHEAD: u64 = 128;
                    const __LAMPORTS_PER_BYTE: u64 = 6960;
                    let __space = (#space_tokens) as u64;
                    let __lamports = (__STORAGE_OVERHEAD + __space).wrapping_mul(__LAMPORTS_PER_BYTE);
                    naclac_lang::prelude::system_program::create_account_checked_for_init(
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
