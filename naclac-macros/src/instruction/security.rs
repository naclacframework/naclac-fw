//! # Instruction Security Constraints
//!
//! Generates the actual enforcement logic for `#[account(...)]` constraints.
//! This module handles two phases of security checks:
//! 1. **Immediate Metadata Checks**: Validates raw `AccountInfo` properties (signer, mut, owner, PDA seeds).
//! 2. **Relational Checks**: Validates cross-account relationships (e.g., `has_one`) after all accounts are parsed.

use crate::instruction::parser::ParsedField;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

/// PHASE 1: Immediate Metadata Checks
///
/// Generates validation logic that runs immediately upon loading an individual `AccountInfo`.
/// This includes `is_signer`, `is_writable`, address matching, owner validation, and PDA seed derivations.
pub fn generate_security_checks(
    field: &ParsedField,
    all_fields: &[ParsedField],
) -> (TokenStream, TokenStream) {
    let mut metadata_checks = Vec::new();
    let mut constraint_checks = Vec::new();
    let idx = field.index;

    // 1. Signer Check (Handled by the Wrapper try_from normally, but explicitly added if raw)
    if field.is_signer && !field.type_str.contains("Signer") {
        metadata_checks.push(quote! {
            #[cfg(not(feature = "pinocchio"))]
            let __is_signer = info.is_signer;
            #[cfg(feature = "pinocchio")]
            let __is_signer = info.is_signer();

            if !__is_signer {
                return Err(naclac_lang::prelude::NaclacError::ConstraintSigner.err(#idx));
            }
        });
    }

    // 2. Mutability Check
    if field.is_mut {
        metadata_checks.push(quote! {
            #[cfg(not(feature = "pinocchio"))]
            let __is_writable = info.is_writable;
            #[cfg(feature = "pinocchio")]
            let __is_writable = info.is_writable();

            if !__is_writable {
                return Err(naclac_lang::prelude::NaclacError::ConstraintMut.err(#idx));
            }
        });
    }

    // 3. Address Check
    if let Some(address_expr) = &field.address {
        metadata_checks.push(quote! {
            let __key = naclac_lang::prelude::Key::key(info);
            if __key != naclac_lang::prelude::ToPubkey::to_pubkey(&(#address_expr)) {
                return Err(naclac_lang::prelude::NaclacError::ConstraintAddress.err(#idx));
            }
        });
    }

    // 4. Owner Check
    if let Some(owner_expr) = &field.owner {
        // Check if this is a relational owner (pointing to another field)
        let mut is_relational = false;
        if let syn::Expr::Path(expr_path) = owner_expr {
            if let Some(ident) = expr_path.path.get_ident() {
                if all_fields.iter().any(|f| &f.ident == ident) {
                    is_relational = true;
                }
            }
        }

        if !is_relational {
            match owner_expr {
                syn::Expr::Lit(expr_lit) => {
                    if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                        let s = lit_str.value();
                        metadata_checks.push(quote! {
                            #[cfg(not(feature = "pinocchio"))]
                            let expected_owner = naclac_lang::solana_program::pubkey!(#s);
                            #[cfg(feature = "pinocchio")]
                            let expected_owner = naclac_lang::pinocchio::pubkey!(#s);

                            let __owner = naclac_lang::prelude::Owner::owner(info);
                            if __owner != naclac_lang::prelude::ToPubkey::to_pubkey(&expected_owner) {
                                return Err(naclac_lang::prelude::NaclacError::ConstraintOwner.err(#idx));
                            }
                        });
                    } else {
                        metadata_checks.push(quote! {
                            let __owner = naclac_lang::prelude::Owner::owner(info);
                            if __owner != naclac_lang::prelude::ToPubkey::to_pubkey(&(#owner_expr)) {
                                return Err(naclac_lang::prelude::NaclacError::ConstraintOwner.err(#idx));
                            }
                        });
                    }
                }
                _ => {
                    metadata_checks.push(quote! {
                            let __owner = naclac_lang::prelude::Owner::owner(info);
                            if __owner != naclac_lang::prelude::ToPubkey::to_pubkey(&(#owner_expr)) {
                                return Err(naclac_lang::prelude::NaclacError::ConstraintOwner.err(#idx));
                            }
                        });
                }
            }
        }
    } else if (field.type_str.contains("AccountLoader")
        || field.type_str.contains("Account")
        || field.type_str.contains("ZcAccount"))
        && !field.type_str.contains("AccountInfo")
        && !field.type_str.contains("Mint")
        && !field.type_str.contains("TokenAccount")
        && field.init_config.is_none()
    {
        // DEFAULT OWNER CHECK: If it's a typed Naclac account and not being initialized,
        // it MUST be owned by the current program. We exempt standard SPL types (Mint, TokenAccount).
        metadata_checks.push(quote! {
            let __owner = naclac_lang::prelude::Owner::owner(info);
            if __owner != naclac_lang::prelude::ToPubkey::to_pubkey(program_id) {
                return Err(naclac_lang::prelude::NaclacError::ConstraintOwner.err(#idx));
            }
        });
    }

    // 5. PDA Validation Check
    if let Some(seed_array) = &field.pda_seed {
        let field_ident = &field.ident;
        let bump_cap = format_ident!("__bump_{}", field_ident);

        let pda_program_tokens = if let Some(prog_str) = &field.pda_program {
            let prog_expr: syn::Expr = syn::parse_str(prog_str).unwrap();
            quote! { &(#prog_expr) }
        } else {
            quote! { program_id }
        };

        // Generate bindings for each seed to prevent temporary drop issues (E0716)
        let seed_bindings: Vec<TokenStream> = seed_array.elems.iter().enumerate().map(|(i, expr)| {
            let ident = format_ident!("__seed_{}_{}", field_ident, i);
            let raw_str = quote! { #expr }.to_string();
            let expr_str: String = raw_str.chars().filter(|c| !c.is_whitespace()).collect();

            // 1. Detect field access on AccountLoaders and inject .load()?.
            let mut binding_str = expr_str.clone();
            for other_field in all_fields {
                let other_name = other_field.ident.to_string();
                if (other_field.type_str.contains("AccountLoader") || other_field.type_str.contains("ZcAccount"))
                   && binding_str.starts_with(&format!("{}.", other_name)) 
                   && !binding_str.contains(".load()") 
                   && !binding_str.contains(&format!("{}.key", other_name))
                   && !binding_str.contains(&format!("{}.info", other_name))
                   && !binding_str.contains(&format!("{}.to_account_info", other_name)) {
                    binding_str = binding_str.replace(&format!("{}.", other_name), &format!("{}.load()?. ", other_name));
                }
            }

            // 2. Detect wrapper key access (.key not already .key()) and transform to .key()
            if binding_str.contains(".key") && !binding_str.contains(".key()") && !binding_str.contains(".info.key") {
                binding_str = binding_str.replace(".key", ".key()");
            }

            let is_key_access = binding_str.contains(".key()");
            let has_le_bytes   = binding_str.contains(".to_le_bytes()");
            let has_be_bytes   = binding_str.contains(".to_be_bytes()");
            let has_as_bytes   = binding_str.contains(".as_bytes()");

            // Strip trailing reference methods so the binding captures the owned value
            binding_str = binding_str
                .replace(".as_ref()", "")
                .replace(".as_bytes()", "")
                .replace(".to_le_bytes()", "")
                .replace(".to_be_bytes()", "");

            // Re-add byte-conversion methods so the binding is still the right type
            if has_le_bytes       { binding_str = format!("{}.to_le_bytes()", binding_str); }
            else if has_be_bytes  { binding_str = format!("{}.to_be_bytes()", binding_str); }
            else if has_as_bytes  { binding_str = format!("{}.as_bytes().to_vec()", binding_str); }

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

        let mut bump_verification_logic = quote! {};
        let pda_creation_logic;

        if let Some(bump_name) = &field.pda_bump {
            if bump_name == "__naclac_auto_bump" {
                // [SMART BUMP LOGIC]
                if field.init_config.is_none() {
                    // Optimized path for existing accounts: load bump from data if possible
                    let is_spl_type =
                        field.type_str.contains("TokenAccount") || field.type_str.contains("Mint");

                    if (field.type_str.contains("AccountLoader")
                        || field.type_str.contains("ZcAccount"))
                        && !is_spl_type
                    {
                        pda_creation_logic = quote! {
                            let expected_bump = #field_ident.load()?.bump;
                            #(#seed_bindings)*
                            let expected_pda = naclac_lang::prelude::Pubkey::create_program_address(
                                &[#(#seed_refs),*, &[expected_bump]],
                                #pda_program_tokens
                            ).map_err(|_| naclac_lang::prelude::NaclacError::ConstraintSeeds.err(#idx))?;
                        };
                    } else if field.type_str.contains("Account")
                        && !field.type_str.contains("AccountInfo")
                        && !is_spl_type
                    {
                        pda_creation_logic = quote! {
                            #[cfg(not(feature = "pinocchio"))]
                            let expected_bump = #field_ident.data.bump;
                            #[cfg(feature = "pinocchio")]
                            let expected_bump = #field_ident.bump;

                            #(#seed_bindings)*
                            let expected_pda = naclac_lang::prelude::Pubkey::create_program_address(
                                &[#(#seed_refs),*, &[expected_bump]],
                                #pda_program_tokens
                            ).map_err(|_| naclac_lang::prelude::NaclacError::ConstraintSeeds.err(#idx))?;
                        };
                    } else {
                        // Fallback for raw AccountInfo, TokenAccount, Mint or others
                        pda_creation_logic = quote! {
                            #(#seed_bindings)*
                            let (expected_pda, expected_bump) = naclac_lang::prelude::Pubkey::find_program_address(
                                &[#(#seed_refs),*],
                                #pda_program_tokens
                            );
                        };
                    }
                } else {
                    // Init logic: must use find_program_address to discover the canonical bump
                    pda_creation_logic = quote! {
                        #(#seed_bindings)*
                        let (expected_pda, expected_bump) = naclac_lang::prelude::Pubkey::find_program_address(
                            &[#(#seed_refs),*],
                            #pda_program_tokens
                        );
                    };
                }
            } else if let Ok(bump_expr) = syn::parse_str::<syn::Expr>(bump_name) {
                // Manual bump provided via bump = <expr>
                bump_verification_logic = quote! {
                    if expected_bump != (#bump_expr) {
                        return Err(naclac_lang::prelude::NaclacError::ConstraintSeeds.err(#idx));
                    }
                };
                pda_creation_logic = quote! {
                    let expected_bump = #bump_expr;
                    #(#seed_bindings)*
                    let expected_pda = naclac_lang::prelude::Pubkey::create_program_address(
                        &[#(#seed_refs),*, &[expected_bump]],
                        #pda_program_tokens
                    ).map_err(|_| naclac_lang::prelude::NaclacError::ConstraintSeeds.err(#idx))?;
                };
            } else {
                // Fallback for malformed or special bump names
                pda_creation_logic = quote! {
                    #(#seed_bindings)*
                    let (expected_pda, expected_bump) = naclac_lang::prelude::Pubkey::find_program_address(
                        &[#(#seed_refs),*],
                        #pda_program_tokens
                    );
                };
            }
        } else {
            // No bump attribute: standard find_program_address
            pda_creation_logic = quote! {
                #(#seed_bindings)*
                let (expected_pda, expected_bump) = naclac_lang::prelude::Pubkey::find_program_address(
                    &[#(#seed_refs),*],
                    #pda_program_tokens
                );
            };
        }

        if field.init_config.is_none() {
            constraint_checks.push(quote! {
                #pda_creation_logic
                if naclac_lang::prelude::Key::key(&#field_ident) != expected_pda {
                    return Err(naclac_lang::prelude::NaclacError::ConstraintSeeds.err(#idx));
                }
                #bump_verification_logic
                let #bump_cap = expected_bump;
            });
        } else {
            metadata_checks.push(quote! {
                #pda_creation_logic
                let __key = naclac_lang::prelude::Key::key(info);
                if __key != expected_pda {
                    return Err(naclac_lang::prelude::NaclacError::ConstraintSeeds.err(#idx));
                }
                #bump_verification_logic
                let #bump_cap = expected_bump;
            });
        }
    }

    // 6. Token Validation (for existing accounts)
    if field.init_config.is_none() {
        let field_ident = &field.ident;
        let mut token_checks = Vec::new();

        if let Some(mint_expr) = &field.token_mint {
            let actual_mint_access = resolve_smart_key(mint_expr, all_fields, "mint");
            token_checks.push(quote! {
                {
                    #[cfg(not(feature = "pinocchio"))]
                    let __data_info = #field_ident.to_account_info();
                    #[cfg(not(feature = "pinocchio"))]
                    let data = __data_info.try_borrow_data()?;
                    #[cfg(feature = "pinocchio")]
                    let data = #field_ident.data();

                    if data.len() < 32 { return Err(naclac_lang::prelude::NaclacError::ConstraintAccountIsNone.err(#idx)); }
                    let expected_mint: naclac_lang::prelude::Pubkey = naclac_lang::prelude::Pubkey::new_from_array(data[0..32].try_into().unwrap());
                    use naclac_lang::prelude::ToPubkey;
                    let actual_mint: naclac_lang::prelude::Pubkey = #actual_mint_access;
                    if expected_mint != actual_mint {
                        return Err(naclac_lang::prelude::NaclacError::ConstraintAccountIsNone.err(#idx));
                    }
                }
            });
        }

        if !token_checks.is_empty() {
            constraint_checks.push(quote! {
                { #(#token_checks)* }
            });
        }
    }

    (
        quote! { #(#metadata_checks)* },
        quote! { #(#constraint_checks)* },
    )
}

fn resolve_smart_key(expr: &syn::Expr, all_fields: &[ParsedField], key_type: &str) -> TokenStream {
    if let syn::Expr::Path(expr_path) = expr {
        if let Some(ident) = expr_path.path.get_ident() {
            if let Some(target_field) = all_fields.iter().find(|f| &f.ident == ident) {
                let target_ident = &target_field.ident;
                if target_field.type_str.contains("AccountLoader")
                    || target_field.type_str.contains("ZcAccount")
                {
                    if key_type == "mint" {
                        // Fallback order: stake_token_mint, mint
                        return quote! {
                            if let Ok(pool) = #target_ident.load() {
                                pool.stake_token_mint
                            } else {
                                (#expr).to_pubkey()
                            }
                        };
                    } else if key_type == "authority" {
                        return quote! { (#target_ident).to_pubkey() };
                    }
                } else if target_field.type_str.contains("Account") {
                    if key_type == "mint" {
                        return quote! { (#target_ident).stake_token_mint };
                    } else if key_type == "authority" {
                        return quote! { (#target_ident).to_pubkey() };
                    }
                    return quote! { (#target_ident).to_pubkey() };
                }
            }
        }
    }
    quote! { (#expr).to_pubkey() }
}

/// PHASE 2: Relational Checks
///
/// Generates validation logic that runs *after* all accounts have been loaded into the instruction struct.
/// This is necessary for constraints like `has_one` where one account's data must be compared against
/// the public key of another account in the struct.
pub fn generate_relational_checks(fields: &[ParsedField], prefix: TokenStream) -> Vec<TokenStream> {
    let mut checks = Vec::new();

    for field in fields {
        let field_name = &field.ident;

        for config in &field.has_one {
            let target_name = &config.target;
            let target_ident = format_ident!("{}", target_name);

            // Check if the target field exists in the instruction struct
            if !fields.iter().any(|f| f.ident == target_name) {
                let error_msg = format!(
                    "Field '{}' not found in struct for has_one constraint on '{}'",
                    target_name, field_name
                );
                checks.push(quote! {
                    core::compile_error!(#error_msg);
                });
                continue;
            }

            // We do NOT check `has_one` if the account is being newly initialized!
            if field.init_config.is_none() {
                let idx = field.index;
                let error_logic = if let Some(custom_error) = &config.custom_error {
                    quote! { #custom_error.into() }
                } else {
                    quote! { naclac_lang::prelude::NaclacError::Unauthorized.err(#idx) }
                };

                let check_logic = if field.type_str.contains("AccountLoader")
                    || field.type_str.contains("ZcAccount")
                {
                    quote! {
                        let __val = #prefix.#field_name.load()?.#target_ident;
                        let __target_key = naclac_lang::prelude::Key::key(&#prefix.#target_ident);
                        if __val != __target_key {
                            return Err(#error_logic);
                        }
                    }
                } else if field.type_str.contains("Account")
                    && !field.type_str.contains("AccountInfo")
                {
                    quote! {
                        #[cfg(not(feature = "pinocchio"))]
                        let __val = #prefix.#field_name.#target_ident;
                        #[cfg(feature = "pinocchio")]
                        let __val = #prefix.#field_name.#target_ident;

                        let __target_key = naclac_lang::prelude::Key::key(&#prefix.#target_ident);
                        if __val != __target_key {
                            return Err(#error_logic);
                        }
                    }
                } else {
                    quote! {
                        let __val = #prefix.#field_name.#target_ident;
                        let __target_key = naclac_lang::prelude::Key::key(&#prefix.#target_ident);
                        if __val != __target_key {
                            return Err(#error_logic);
                        }
                    }
                };

                checks.push(check_logic);
            }
        }

        // Relational Owner Check
        if let Some(owner_expr) = &field.owner {
            if let syn::Expr::Path(expr_path) = owner_expr {
                if let Some(ident) = expr_path.path.get_ident() {
                    if fields.iter().any(|f| &f.ident == ident) {
                        let idx = field.index;
                        checks.push(quote! {
                            let __expected_owner = naclac_lang::prelude::ToPubkey::to_pubkey(&#prefix.#ident);
                            let __actual_owner = naclac_lang::prelude::Owner::owner(&#prefix.#field_name);
                            if __actual_owner != __expected_owner {
                                return Err(naclac_lang::prelude::NaclacError::ConstraintOwner.err(#idx));
                            }
                        });
                    }
                }
            }
        }

        // token::program Check
        if let Some(prog_expr) = &field.token_program {
            let idx = field.index;
            checks.push(quote! {
                let __expected_prog = naclac_lang::prelude::ToPubkey::to_pubkey(&#prefix.#prog_expr);
                let __actual_owner = naclac_lang::prelude::Owner::owner(&#prefix.#field_name);
                if __actual_owner != __expected_prog {
                    return Err(naclac_lang::prelude::NaclacError::ProgramIdMismatch.err(#idx));
                }
            });
        }
    }

    checks
}
