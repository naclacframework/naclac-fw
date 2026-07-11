//! # Instruction Security Constraints
//!
//! Generates the actual enforcement logic for `#[account(...)]` constraints.
//! This module handles two phases of security checks:
//! 1. **Immediate Metadata Checks**: Validates raw `AccountInfo` properties (signer, mut, owner, PDA seeds).
//! 2. **Relational Checks**: Validates cross-account relationships (e.g., `has_one`) after all accounts are parsed.

use crate::instruction::parser::ParsedField;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::spanned::Spanned;

fn is_exact_signer_type(type_str: &str) -> bool {
    let clean: String = type_str.chars().filter(|c| !c.is_whitespace()).collect();
    clean == "Signer" || clean.starts_with("Signer<") || clean.contains("::Signer")
}

/// True if this field's declared type is a zero-copy account wrapper —
/// either explicitly (`AccountLoader<T>`/`ZcAccount<T>`) or implicitly, when
/// the whole Accounts struct is in zero-copy mode and the field is a plain
/// `Account<T>` (which resolves to `AccountLoader<T>` via the prelude alias
/// whenever `pinocchio` is active or `borsh` is not — see naclac-core's
/// prelude.rs). Without the `program_is_zero_copy` fallback, a plain
/// `Account<T>` field in a zero-copy program would be misclassified as a
/// Borsh account by this module, even though it is really an `AccountLoader`.
pub(crate) fn is_zero_copy_account_field(type_str: &str, program_is_zero_copy: bool) -> bool {
    type_str.contains("AccountLoader")
        || type_str.contains("ZcAccount")
        || (program_is_zero_copy
            && type_str.contains("Account")
            && !type_str.contains("AccountInfo"))
}

/// PHASE 1: Immediate Metadata Checks
///
/// Generates validation logic that runs immediately upon loading an individual `AccountInfo`.
/// This includes `is_signer`, `is_writable`, address matching, owner validation, and PDA seed derivations.
pub fn generate_security_checks(
    field: &ParsedField,
    all_fields: &[ParsedField],
    is_zero_copy: bool,
) -> (TokenStream, TokenStream) {
    let mut metadata_checks = Vec::new();
    let mut constraint_checks = Vec::new();
    let idx = field.index;

    // 1. Signer Check (Handled by the Wrapper try_from normally, but explicitly added if raw)
    if field.is_signer && !is_exact_signer_type(&field.type_str) {
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
            let __is_writable = {
                #[cfg(not(feature = "pinocchio"))]
                { info.is_writable }
                #[cfg(feature = "pinocchio")]
                { info.is_writable() }
            };
            if !__is_writable {
                return Err(naclac_lang::prelude::NaclacError::ConstraintMut.err(#idx));
            }
        });
    }

    // 2b. Executable Check
    if field.is_executable {
        metadata_checks.push(quote! {
            let __is_executable = {
                #[cfg(not(feature = "pinocchio"))]
                { info.executable }
                #[cfg(feature = "pinocchio")]
                { info.is_executable() }
            };
            if !__is_executable {
                return Err(naclac_lang::prelude::NaclacError::ConstraintExecutable.err(#idx));
            }
        });
    }

    // 3. Address Check
    if let Some(address_expr) = &field.address {
        let type_str = &field.type_str;
        let is_program_or_interface =
            type_str.contains("Program") || type_str.contains("Interface");
        if !is_program_or_interface {
            metadata_checks.push(quote! {
                let __key = (info).address();
                if __key != naclac_lang::prelude::ToAddress::address(&(#address_expr)) {
                    return Err(naclac_lang::prelude::NaclacError::ConstraintAddress.err(#idx));
                }
            });
        }
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
                        // Decode the base58 owner address at proc-macro compile time
                        // into a byte literal so we never depend on pinocchio::pubkey!.
                        let owner_bytes: Vec<u8> = bs58::decode(&s)
                            .into_vec()
                            .expect("Invalid base58 owner address in #[account(owner = \"...\")].");
                        assert_eq!(owner_bytes.len(), 32, "Owner address must be 32 bytes");
                        let byte_array: Vec<u8> = owner_bytes;
                        let byte_lit: Vec<_> =
                            byte_array.iter().map(|b| quote::quote! { #b }).collect();
                        metadata_checks.push(quote! {
                            let expected_owner = naclac_lang::prelude::Address::new_from_array([ #(#byte_lit),* ]);
                            let __owner = naclac_lang::prelude::Owner::program_owner(info);
                            if __owner != naclac_lang::prelude::ToAddress::address(&expected_owner) {
                                return Err(naclac_lang::prelude::NaclacError::ConstraintOwner.err(#idx));
                            }
                        });
                    } else {
                        metadata_checks.push(quote! {
                            let __owner = naclac_lang::prelude::Owner::program_owner(info);
                            if __owner != naclac_lang::prelude::ToAddress::address(&(#owner_expr)) {
                                return Err(naclac_lang::prelude::NaclacError::ConstraintOwner.err(#idx));
                            }
                        });
                    }
                }
                _ => {
                    metadata_checks.push(quote! {
                            let __owner = naclac_lang::prelude::Owner::program_owner(info);
                            if __owner != naclac_lang::prelude::ToAddress::address(&(#owner_expr)) {
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
        // DEFAULT OWNER CHECK: verify this account is owned by the current
        // program, in every mode (Borsh and zero-copy alike). This overlaps
        // with discriminator validation (both `Account<T>::try_from` and
        // `AccountLoader<T>::load` now reject a mismatched discriminator), but
        // is kept unconditionally as defense-in-depth and for a clearer,
        // distinct error (`ConstraintOwner` vs. a discriminator-mismatch error).
        metadata_checks.push(quote! {
            {
                let __owner = naclac_lang::prelude::Owner::program_owner(info);
                if __owner != naclac_lang::prelude::ToAddress::address(program_id) {
                    return Err(naclac_lang::prelude::NaclacError::ConstraintOwner.err(#idx));
                }
            }
        });
    }

    // 5. PDA Validation Check
    if let Some(seed_array) = &field.pda_seed {
        let field_ident = &field.ident;
        let bump_cap = format_ident!("__bump_{}", field_ident);

        let pda_program_tokens = if let Some(prog_str) = &field.pda_program {
            let prog_expr: syn::Expr = syn::parse_str(prog_str).unwrap();
            quote! { #prog_expr }
        } else {
            quote! { program_id }
        };

        // Resolve compile-time precomputed PDA if all seeds are literals
        let mut literal_seeds = Vec::new();
        let mut all_literals = true;
        for elem in &seed_array.elems {
            if let Some(bytes) = resolve_seed_literal(elem) {
                literal_seeds.push(bytes);
            } else {
                all_literals = false;
                break;
            }
        }

        let mut is_pda_precomputed = false;
        let mut pda_str = String::new();
        let mut bump_val = 0;
        let mut toml_path_str = String::new();

        if all_literals {
            let crate_name = std::env::var("CARGO_PKG_NAME").unwrap_or_default();
            if let Some((prog_id_str, path_str)) = find_program_id(&crate_name) {
                if let Some((computed_pda, computed_bump)) =
                    find_pda_at_compile_time(&literal_seeds, &prog_id_str)
                {
                    is_pda_precomputed = true;
                    pda_str = computed_pda;
                    bump_val = computed_bump;
                    toml_path_str = path_str;
                }
            }
        }

        // Generate bindings for each seed to prevent temporary drop issues (E0716)
        let seed_bindings: Vec<TokenStream> = seed_array.elems.iter().enumerate().map(|(i, expr)| {
            let ident = format_ident!("__seed_{}_{}", field_ident, i);
            let raw_str = quote! { #expr }.to_string();
            let expr_str: String = raw_str.chars().filter(|c| !c.is_whitespace()).collect();

            // Check if this expression is a Zc account access
            let binding_str = expr_str.clone();
            let mut is_zc_access = false;
            let mut zc_check_tokens = quote! {};

            for other_field in all_fields {
                let other_name = other_field.ident.to_string();
                if is_zero_copy_account_field(&other_field.type_str, is_zero_copy)
                   && binding_str.starts_with(&format!("{}." , other_name))
                {
                    let field_path = binding_str[other_name.len() + 1..].trim();
                    // A method call on the whole account (e.g. `.as_ref()`,
                    // `.address()`) is not a struct-field path — only genuine
                    // field-name continuations should be routed through
                    // `Deref<Target = T>`. Method calls fall through to the
                    // generic normalization below, which already knows how to
                    // rewrite them (e.g. via `AsRefByteSlice`) for both Borsh
                    // and zero-copy account fields alike.
                    if field_path.contains('(') {
                        continue;
                    }
                    let other_ident = &other_field.ident;
                    let field_path_expr: syn::Expr = syn::parse_str(field_path).unwrap();
                    is_zc_access = true;
                    // Since AccountLoader<T> implements Deref<Target=T>, we can access
                    // fields directly via auto-deref — no manual data() + NaclacZeroCopy::load() needed.
                    zc_check_tokens = quote! {
                        #other_ident.#field_path_expr
                    };
                    break;
                }
            }

            if is_zc_access {
                return quote! { let #ident = #zc_check_tokens; };
            }

            // Fallback: not a zero-copy field-path access (either a Borsh account,
            // or a method call on a zero-copy account handled generically below).
            let mut binding_str = expr_str.clone();

            let is_key_access = binding_str.contains(".address()");
            let has_le_bytes   = binding_str.contains(".to_le_bytes()");
            let has_be_bytes   = binding_str.contains(".to_be_bytes()");
            let has_as_bytes   = binding_str.contains(".as_bytes()");

            binding_str = binding_str
                .replace(".as_ref()", "")
                .replace(".as_bytes()", "")
                .replace(".to_le_bytes()", "")
                .replace(".to_be_bytes()", "");

            if has_le_bytes       { binding_str = format!("{}.to_le_bytes()", binding_str); }
            else if has_be_bytes  { binding_str = format!("{}.to_be_bytes()", binding_str); }
            else if has_as_bytes  { binding_str = format!("{}.as_bytes().to_vec()", binding_str); }

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

        let mut bump_verification_logic = quote! {};
        let pda_creation_logic;
        let mut use_find_pda = false;

        if is_pda_precomputed {
            // Decode the precomputed PDA at proc-macro compile time into a byte literal
            // so neither backend needs a pubkey! macro.
            let pda_bytes: Vec<u8> = bs58::decode(pda_str)
                .into_vec()
                .expect("Invalid precomputed PDA address");
            let pda_byte_lit: Vec<_> = pda_bytes.iter().map(|b| quote::quote! { #b }).collect();
            pda_creation_logic = quote! {
                const _: &[u8] = include_bytes!(#toml_path_str);
                let expected_pda = naclac_lang::prelude::Address::new_from_array([ #(#pda_byte_lit),* ]);
                let expected_bump = #bump_val;
            };

            if let Some(bump_name) = &field.pda_bump {
                if bump_name != "__naclac_auto_bump" {
                    if field.init_config.is_some() {
                        let err = syn::Error::new_spanned(
                            &field.ident,
                            "Naclac Error: `bump = <expr>` cannot be combined with `init` on a precomputed PDA. \
                             Account creation must use the canonical bump (write `bump` without a value).",
                        ).to_compile_error();
                        return (err.clone(), err);
                    }
                    if let Ok(bump_expr) = syn::parse_str::<syn::Expr>(bump_name) {
                        bump_verification_logic = quote! {
                            if expected_bump != (#bump_expr) {
                                return Err(naclac_lang::prelude::NaclacError::ConstraintSeeds.err(#idx));
                            }
                        };
                    }
                }
            }
        } else {
            // Dynamic PDA path (hash-and-compare optimization or find_program_address)
            let mut bump_expr = quote! {};

            if let Some(bump_name) = &field.pda_bump {
                if bump_name == "__naclac_auto_bump" {
                    if field.init_config.is_none() {
                        let is_spl_type = field.type_str.contains("TokenAccount")
                            || field.type_str.contains("Mint");
                        if is_zero_copy_account_field(&field.type_str, is_zero_copy) && !is_spl_type
                        {
                            // AccountLoader<T> implements Deref<Target=T>: access .bump directly.
                            bump_expr = quote! { #field_ident.bump };
                        } else if field.type_str.contains("Account")
                            && !field.type_str.contains("AccountInfo")
                            && !is_spl_type
                        {
                            // Only reachable for genuine Borsh `Account<T>` now — a
                            // zero-copy `Account<T>` (pinocchio, or solana zero-copy
                            // via the prelude alias) already took the branch above.
                            bump_expr = quote! {
                                {
                                    #[cfg(all(not(feature = "pinocchio"), feature = "borsh"))]
                                    let __b = #field_ident.data.bump;
                                    #[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
                                    let __b = #field_ident.bump;
                                    __b
                                }
                            };
                        } else {
                            use_find_pda = true;
                        }
                    } else {
                        use_find_pda = true;
                    }
                } else if let Ok(parsed_bump_expr) = syn::parse_str::<syn::Expr>(bump_name) {
                    bump_expr = quote! { (#parsed_bump_expr) };
                    bump_verification_logic = quote! {
                        if expected_bump != (#parsed_bump_expr) {
                            return Err(naclac_lang::prelude::NaclacError::ConstraintSeeds.err(#idx));
                        }
                    };
                } else {
                    use_find_pda = true;
                }
            } else {
                use_find_pda = true;
            }

            if use_find_pda {
                pda_creation_logic = quote! {
                    compile_error!("Naclac Error: PDA validation seeds constraint requires a 'bump' value or attribute to enable hash-and-compare optimization. On-chain 'find_program_address' is strictly banned in Naclac. Example: #[account(seeds = [...], bump)]");
                };
            } else {
                // Hash-and-compare optimization (0 CU PDA verify if owned, or ~100 CU hash instead of ~1000 CU on-curve check)
                pda_creation_logic = quote! {
                    #(#seed_bindings)*
                    let expected_bump = #bump_expr;
                    let __expected_bump_arr = [expected_bump];
                    let __pda_program = naclac_lang::prelude::ToAddress::address(&#pda_program_tokens);
                    let inputs = [
                        #( #seed_refs, )*
                        &__expected_bump_arr,
                        __pda_program.as_ref(),
                        b"ProgramDerivedAddress"
                    ];

                    #[cfg(not(feature = "pinocchio"))]
                    let expected_pda = {
                        let hash_result = naclac_lang::solana_program::hash::hashv(&inputs);
                        naclac_lang::prelude::Address::new_from_array(hash_result.to_bytes())
                    };

                    #[cfg(feature = "pinocchio")]
                    let expected_pda = {
                        extern "C" {
                            pub fn sol_sha256(
                                vals: *const u8,
                                val_len: u64,
                                hash_result: *mut u8,
                            ) -> u32;
                        }
                        let mut hash_result = [0u8; 32];
                        unsafe {
                            sol_sha256(
                                inputs.as_ptr() as *const u8,
                                inputs.len() as u64,
                                hash_result.as_mut_ptr(),
                            );
                        }
                        naclac_lang::prelude::Address::new_from_array(hash_result)
                    };
                };
            }
        }

        // When use_find_pda=true: the SHA256 loop already confirmed that the hash of
        // (seeds || [bump] || program_id || "PDA") == expected_pda (= Key::key(&field_ident)).
        // The post-loop address comparison is therefore tautologically true and is omitted.
        // When is_pda_precomputed=true or hash-and-compare path: the comparison IS needed
        // because expected_pda is a compile-time constant that might differ from the account key.
        let skip_address_check = false;
        if field.init_config.is_none() {
            if skip_address_check {
                constraint_checks.push(quote! {
                    #pda_creation_logic
                    #bump_verification_logic
                    let #bump_cap = expected_bump;
                });
            } else {
                constraint_checks.push(quote! {
                    #pda_creation_logic
                    if (#field_ident).address() != naclac_lang::prelude::ToAddress::address(&expected_pda) {
                        return Err(naclac_lang::prelude::NaclacError::ConstraintSeeds.err(#idx));
                    }
                    #bump_verification_logic
                    let #bump_cap = expected_bump;
                });
            }
        } else {
            if skip_address_check {
                // SHA256 loop already verified: sha256(seeds||bump||program_id||"PDA") == info.address().
                // The post-loop key comparison is tautologically true — skip it.
                // expected_pda is not defined in this path (use_find_pda=true).
                metadata_checks.push(quote! {
                    #pda_creation_logic
                    #bump_verification_logic
                    let #bump_cap = expected_bump;
                });
            } else {
                metadata_checks.push(quote! {
                    #pda_creation_logic
                    let __key = (info).address();
                    if __key != naclac_lang::prelude::ToAddress::address(&expected_pda) {
                        return Err(naclac_lang::prelude::NaclacError::ConstraintSeeds.err(#idx));
                    }
                    #bump_verification_logic
                    let #bump_cap = expected_bump;
                });
            }
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
                    let __data_info = naclac_lang::prelude::ToAccountInfo::to_account_info(&#field_ident);
                    #[cfg(feature = "pinocchio")]
                    let data = __data_info.data();

                    if data.len() < 32 { return Err(naclac_lang::prelude::NaclacError::ConstraintAccountIsNone.err(#idx)); }
                    let expected_mint: naclac_lang::prelude::Address = naclac_lang::prelude::Address::new_from_array(data[0..32].try_into().unwrap());
                    let actual_mint: naclac_lang::prelude::Address = #actual_mint_access;
                    if expected_mint != actual_mint {
                        return Err(naclac_lang::prelude::NaclacError::ConstraintAccountIsNone.err(#idx));
                    }
                }
            });
        }

        if let Some(authority_expr) = &field.token_authority {
            let actual_authority_access =
                resolve_smart_key(authority_expr, all_fields, "authority");
            token_checks.push(quote! {
                {
                    #[cfg(not(feature = "pinocchio"))]
                    let __data_info = #field_ident.to_account_info();
                    #[cfg(not(feature = "pinocchio"))]
                    let data = __data_info.try_borrow_data()?;
                    #[cfg(feature = "pinocchio")]
                    let __data_info = naclac_lang::prelude::ToAccountInfo::to_account_info(&#field_ident);
                    #[cfg(feature = "pinocchio")]
                    let data = __data_info.data();

                    if data.len() < 64 { return Err(naclac_lang::prelude::NaclacError::ConstraintAccountIsNone.err(#idx)); }
                    let expected_authority: naclac_lang::prelude::Address = naclac_lang::prelude::Address::new_from_array(data[32..64].try_into().unwrap());
                    let actual_authority: naclac_lang::prelude::Address = #actual_authority_access;
                    if expected_authority != actual_authority {
                        return Err(naclac_lang::prelude::NaclacError::ConstraintAddress.err(#idx));
                    }
                }
            });
        }

        if !token_checks.is_empty() {
            constraint_checks.push(quote! {
                { #(#token_checks)* }
            });
        }

        let mut mint_checks = Vec::new();

        if let Some(decimals_expr) = &field.mint_decimals {
            mint_checks.push(quote! {
                {
                    #[cfg(not(feature = "pinocchio"))]
                    let __data_info = #field_ident.to_account_info();
                    #[cfg(not(feature = "pinocchio"))]
                    let data = __data_info.try_borrow_data()?;
                    #[cfg(feature = "pinocchio")]
                    let __data_info = naclac_lang::prelude::ToAccountInfo::to_account_info(&#field_ident);
                    #[cfg(feature = "pinocchio")]
                    let data = __data_info.data();

                    if data.len() < 82 { return Err(naclac_lang::prelude::NaclacError::ConstraintAccountIsNone.err(#idx)); }
                    let expected_decimals = data[44];
                    let actual_decimals: u8 = #decimals_expr;
                    if expected_decimals != actual_decimals {
                        return Err(naclac_lang::prelude::NaclacError::Unauthorized.err(#idx));
                    }
                }
            });
        }

        if let Some(authority_expr) = &field.mint_authority {
            let actual_authority_access =
                resolve_smart_key(authority_expr, all_fields, "authority");
            mint_checks.push(quote! {
                {
                    #[cfg(not(feature = "pinocchio"))]
                    let __data_info = #field_ident.to_account_info();
                    #[cfg(not(feature = "pinocchio"))]
                    let data = __data_info.try_borrow_data()?;
                    #[cfg(feature = "pinocchio")]
                    let __data_info = naclac_lang::prelude::ToAccountInfo::to_account_info(&#field_ident);
                    #[cfg(feature = "pinocchio")]
                    let data = __data_info.data();

                    if data.len() < 82 { return Err(naclac_lang::prelude::NaclacError::ConstraintAccountIsNone.err(#idx)); }
                    let option_bytes = &data[0..4];
                    let has_authority = option_bytes == &[1, 0, 0, 0];
                    if !has_authority {
                        return Err(naclac_lang::prelude::NaclacError::Unauthorized.err(#idx));
                    }
                    let expected_authority = naclac_lang::prelude::Address::new_from_array(data[4..36].try_into().unwrap());
                    let actual_authority: naclac_lang::prelude::Address = #actual_authority_access;
                    if expected_authority != actual_authority {
                        return Err(naclac_lang::prelude::NaclacError::ConstraintAddress.err(#idx));
                    }
                }
            });
        }

        if let Some(freeze_authority_expr) = &field.mint_freeze_authority {
            let actual_freeze_authority_access =
                resolve_smart_key(freeze_authority_expr, all_fields, "freeze_authority");
            mint_checks.push(quote! {
                {
                    #[cfg(not(feature = "pinocchio"))]
                    let __data_info = #field_ident.to_account_info();
                    #[cfg(not(feature = "pinocchio"))]
                    let data = __data_info.try_borrow_data()?;
                    #[cfg(feature = "pinocchio")]
                    let __data_info = naclac_lang::prelude::ToAccountInfo::to_account_info(&#field_ident);
                    #[cfg(feature = "pinocchio")]
                    let data = __data_info.data();

                    if data.len() < 82 { return Err(naclac_lang::prelude::NaclacError::ConstraintAccountIsNone.err(#idx)); }
                    let option_bytes = &data[46..50];
                    let has_freeze = option_bytes == &[1, 0, 0, 0];
                    if !has_freeze {
                        return Err(naclac_lang::prelude::NaclacError::Unauthorized.err(#idx));
                    }
                    let expected_freeze = naclac_lang::prelude::Address::new_from_array(data[50..82].try_into().unwrap());
                    let actual_freeze: naclac_lang::prelude::Address = #actual_freeze_authority_access;
                    if expected_freeze != actual_freeze {
                        return Err(naclac_lang::prelude::NaclacError::ConstraintAddress.err(#idx));
                    }
                }
            });
        }

        if !mint_checks.is_empty() {
            constraint_checks.push(quote! {
                { #(#mint_checks)* }
            });
        }
    }

    let field_span = field.ident.span();
    let metadata_checks_span = field
        .address
        .as_ref()
        .map(|a| a.span())
        .or_else(|| field.owner.as_ref().map(|o| o.span()))
        .unwrap_or(field_span);
    let constraint_checks_span = field
        .pda_seed
        .as_ref()
        .map(|s| s.span())
        .or_else(|| field.token_mint.as_ref().map(|m| m.span()))
        .unwrap_or(field_span);

    (
        set_span_recur(quote! { #(#metadata_checks)* }, metadata_checks_span),
        set_span_recur(quote! { #(#constraint_checks)* }, constraint_checks_span),
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
                    if key_type == "mint" || key_type == "authority" {
                        return quote! { naclac_lang::prelude::ToAddress::address(&#target_ident) };
                    }
                } else if target_field.type_str.contains("Account") {
                    return quote! { naclac_lang::prelude::ToAddress::address(&#target_ident) };
                }
            }
        }
    }
    quote! { naclac_lang::prelude::ToAddress::address(&#expr) }
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

        for config in &field.relations {
            let relation_field = &config.field;
            let target_expr = &config.target;

            let mut target_name = None;
            if let syn::Expr::Path(expr_path) = target_expr {
                if let Some(ident) = expr_path.path.get_ident() {
                    target_name = Some(ident.to_string());
                }
            }

            if let Some(ref target_str) = target_name {
                if !fields.iter().any(|f| f.ident == target_str) {
                    let error_msg = format!(
                        "Field '{}' not found in struct for relation constraint on '{}'",
                        target_str, field_name
                    );
                    checks.push(quote! {
                        core::compile_error!(#error_msg);
                    });
                    continue;
                }
            }

            // We do NOT check relation if the account is being newly initialized!
            if field.init_config.is_none() {
                let idx = field.index;
                let error_logic = if let Some(custom_error) = &config.custom_error {
                    quote! { #custom_error.into() }
                } else {
                    quote! { naclac_lang::prelude::NaclacError::Unauthorized.err(#idx) }
                };

                // Since AccountLoader now implements Deref<Target=T>, we can access
                // fields directly via auto-deref for ALL account types — no more
                // manual data() + NaclacZeroCopy::load() dance.
                let check_logic = quote! {
                    let __val = #prefix.#field_name.#relation_field;
                    let __target_key = (#prefix.#target_expr).address();
                    if __val != __target_key {
                        return Err(#error_logic);
                    }
                };

                checks.push(check_logic);
            }
        }

        // Relational Owner Check
        if let Some(syn::Expr::Path(expr_path)) = &field.owner {
            if let Some(ident) = expr_path.path.get_ident() {
                if fields.iter().any(|f| &f.ident == ident) {
                    let idx = field.index;
                    checks.push(quote! {
                        let __expected_owner = naclac_lang::prelude::ToAddress::address(&#prefix.#ident);
                        let __actual_owner = naclac_lang::prelude::Owner::program_owner(&#prefix.#field_name);
                        if __actual_owner != __expected_owner {
                            return Err(naclac_lang::prelude::NaclacError::ConstraintOwner.err(#idx));
                        }
                    });
                }
            }
        }

        // token::program Check
        if let Some(prog_expr) = &field.token_program {
            let idx = field.index;
            checks.push(quote! {
                let __expected_prog = naclac_lang::prelude::ToAddress::address(&#prefix.#prog_expr);
                let __actual_owner = naclac_lang::prelude::Owner::program_owner(&#prefix.#field_name);
                if __actual_owner != __expected_prog {
                    return Err(naclac_lang::prelude::NaclacError::ProgramIdMismatch.err(#idx));
                }
            });
        }
    }

    checks
}

fn resolve_seed_literal(expr: &syn::Expr) -> Option<Vec<u8>> {
    match expr {
        syn::Expr::Lit(expr_lit) => match &expr_lit.lit {
            syn::Lit::Str(lit_str) => Some(lit_str.value().into_bytes()),
            syn::Lit::ByteStr(lit_byte_str) => Some(lit_byte_str.value()),
            syn::Lit::Byte(lit_byte) => Some(vec![lit_byte.value()]),
            _ => None,
        },
        syn::Expr::Path(expr_path) => {
            if let Some(ident) = expr_path.path.get_ident() {
                let ident_str = ident.to_string();
                if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
                    let src_dir = std::path::Path::new(&manifest_dir).join("src");
                    for file_path in collect_rs_files(&src_dir) {
                        if let Ok(content) = std::fs::read_to_string(&file_path) {
                            for line in content.lines() {
                                let trimmed = line.trim();
                                if trimmed.contains(&format!("const {}", ident_str)) {
                                    if let Some(eq_idx) = trimmed.find('=') {
                                        let rhs =
                                            trimmed[eq_idx + 1..].trim().trim_end_matches(';');
                                        if rhs.starts_with("b\"") && rhs.ends_with('"') {
                                            return Some(
                                                rhs.as_bytes()[2..rhs.len() - 1].to_vec(),
                                            );
                                        } else if rhs.starts_with('"') && rhs.ends_with('"') {
                                            return Some(
                                                rhs.as_bytes()[1..rhs.len() - 1].to_vec(),
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Recursively collects every `.rs` file under `dir`. Mirrors
/// `naclac-syn`'s `parse_workspace_program` source-discovery walk — a
/// literal seed constant can live anywhere under `src/` (a dedicated
/// `constants.rs`, `lib.rs` directly, or any other module), not just a
/// hardcoded `constants.rs` path. See `tests/single-file-layout/`, which
/// specifically exercises a constant declared in `lib.rs`.
fn collect_rs_files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    let mut dirs_to_visit = vec![dir.to_path_buf()];
    while let Some(d) = dirs_to_visit.pop() {
        if let Ok(entries) = std::fs::read_dir(&d) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    dirs_to_visit.push(path);
                } else if path.extension().unwrap_or_default() == "rs" {
                    files.push(path);
                }
            }
        }
    }
    files
}

fn find_program_id(crate_name: &str) -> Option<(String, String)> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").ok()?;
    let mut path = std::path::PathBuf::from(manifest_dir);
    for _ in 0..10 {
        let naclac_toml_path = path.join("Naclac.toml");
        if naclac_toml_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&naclac_toml_path) {
                for line in content.lines() {
                    let trimmed = line.trim();
                    // Skip section headers and empty lines
                    if trimmed.starts_with('[') || trimmed.is_empty() || trimmed.starts_with('#') {
                        continue;
                    }
                    if let Some(eq_idx) = trimmed.find('=') {
                        let key = trimmed[..eq_idx].trim();
                        // Exact match only — "counter" must not match "counter_zc" or "counter_pinocchio"
                        if key == crate_name {
                            let value = trimmed[eq_idx + 1..]
                                .trim()
                                .trim_matches('"')
                                .trim_matches('\'')
                                .to_string();
                            if !value.is_empty() {
                                return Some((value, naclac_toml_path.to_str()?.to_string()));
                            }
                        }
                    }
                }
            }
        }
        if !path.pop() {
            break;
        }
    }
    None
}

fn find_pda_at_compile_time(seeds: &[Vec<u8>], program_id_str: &str) -> Option<(String, u8)> {
    // solana_address::Address has no FromStr or Display impl, so we use bs58.
    let decoded = bs58::decode(program_id_str).into_vec().ok()?;
    if decoded.len() != 32 {
        return None;
    }
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&decoded);
    let addr = solana_address::Address::new_from_array(bytes);
    let seed_slices: Vec<&[u8]> = seeds.iter().map(|s| s.as_slice()).collect();
    let (pda, bump) = solana_address::Address::find_program_address(&seed_slices, &addr);
    // Encode the PDA back to base58 for the compile-time string literal
    let pda_str = bs58::encode(pda.as_ref()).into_string();
    Some((pda_str, bump))
}

fn set_span_recur(tokens: TokenStream, span: proc_macro2::Span) -> TokenStream {
    tokens
        .into_iter()
        .map(|mut tree| match &mut tree {
            proc_macro2::TokenTree::Group(group) => {
                let delim = group.delimiter();
                let stream = set_span_recur(group.stream(), span);
                let mut new_group = proc_macro2::Group::new(delim, stream);
                new_group.set_span(span);
                proc_macro2::TokenTree::Group(new_group)
            }
            _ => {
                tree.set_span(span);
                tree
            }
        })
        .collect()
}
