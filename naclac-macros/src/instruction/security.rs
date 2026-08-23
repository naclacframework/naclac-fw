//! # Instruction Security Constraints
//!
//! Generates the actual enforcement logic for `#[account(...)]` constraints.
//! This module handles two phases of security checks:
//! 1. **Immediate Metadata Checks**: Validates raw `AccountInfo` properties (signer, mut, owner, PDA seeds).
//! 2. **Relational Checks**: Validates cross-account relationships (e.g., `has_one`) after all accounts are parsed.

use crate::instruction::parser::{ParsedField, PdaBump};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::spanned::Spanned;

/// Common "borrow this field's raw account data" prelude shared by every
/// `token::*`/`mint::*`/`associated_token::*` existing-account check —
/// these constraints validate raw bytes directly against the fixed SPL
/// Token/Token-2022 layout (see `naclac_token::token::{TokenAccount, Mint}`'s
/// `check_*` methods), independent of whether the field has been hydrated as
/// a typed zero-copy/Borsh account yet.
fn borrow_field_data_tokens(field_ident: &syn::Ident) -> TokenStream {
    quote! {
        #[cfg(not(feature = "pinocchio"))]
        let __data_info = #field_ident.to_account_info();
        #[cfg(not(feature = "pinocchio"))]
        let data = __data_info.try_borrow_data()?;
        #[cfg(feature = "pinocchio")]
        let __data_info = naclac_lang::prelude::ToAccountInfo::to_account_info(&#field_ident);
        #[cfg(feature = "pinocchio")]
        let data = __data_info.data();
    }
}

fn is_exact_signer_type(ty: &syn::Type) -> bool {
    crate::type_classify::is_exactly(ty, "Signer")
}

/// True if this field's declared type is an account wrapper
/// (`Account<T>`/`InterfaceAccount<T>`) *and* the whole crate is compiled in
/// zero-copy mode (`pinocchio`, or `solana` without `borsh` — the same
/// condition both wrapper types themselves cfg-route on internally, see
/// `naclac-core/src/wrappers/accounts/`). `program_is_zero_copy` is computed
/// once by the caller from the crate's own feature flags and passed in.
pub(crate) fn is_zero_copy_account_field(ty: &syn::Type, program_is_zero_copy: bool) -> bool {
    crate::type_classify::is_account_wrapper(ty) && program_is_zero_copy
}

/// If `expr` is a chain of only `Expr::Field` accesses bottoming out at a
/// bare `Expr::Path` (e.g. `pool_account.bump`, `pool_account.nested.field`
/// — no method calls or other operations anywhere in it), returns that
/// chain's root identifier. `None` for anything else, including a chain with
/// a trailing method call (`pool_account.bump.as_ref()`), since such a call
/// can only appear as the outermost node here and any other node shape means
/// this isn't a plain field-access path at all.
fn seed_field_chain_root(expr: &syn::Expr) -> Option<&syn::Ident> {
    match expr {
        syn::Expr::Path(p) => p.path.get_ident(),
        syn::Expr::Field(f) => seed_field_chain_root(&f.base),
        _ => None,
    }
}

/// Rebuilds `expr`'s field chain with its root replaced by `replacement`,
/// reusing the chain's original `.member` nodes directly via `quote`
/// interpolation rather than stringifying and re-parsing it.
fn substitute_seed_chain_root(expr: &syn::Expr, replacement: &TokenStream) -> TokenStream {
    match expr {
        syn::Expr::Field(f) => {
            let base = substitute_seed_chain_root(&f.base, replacement);
            let member = &f.member;
            quote! { #base.#member }
        }
        _ => replacement.clone(),
    }
}

/// The "real" trailing operation on a seed expression, once any wrapping
/// `.as_ref()`/`.as_bytes()` call has been peeled off.
enum SeedTrailingOp {
    /// A `.address()` call — routed through `ToAddress::address`, which has
    /// an identity impl on `Address` (`naclac-core/src/wrappers/mod.rs`), so
    /// this is safe whether `.address()` is already present or not.
    Address,
    /// Anything else (a `.to_le_bytes()`/`.to_be_bytes()` call, a bare
    /// value, a literal, ...) — used as-is via `AsRefByteSlice`.
    Other,
}

/// Peels exactly one outer `.as_ref()`/`.as_bytes()` layer off `expr` if
/// present — these are always redundant here, since the raw value already
/// satisfies `AsRefByteSlice` directly — and classifies what remains.
/// Detection looks through a single leading `&` (e.g. `&seed.to_le_bytes()`)
/// without removing it; the caller always wraps the result in `&` again
/// regardless, so a leading reference already in the source is preserved as
/// part of what gets re-referenced, not specially unwrapped.
fn peel_seed_wrapper(expr: &syn::Expr) -> (syn::Expr, SeedTrailingOp) {
    let peeled = match expr {
        syn::Expr::MethodCall(mc)
            if mc.args.is_empty() && (mc.method == "as_ref" || mc.method == "as_bytes") =>
        {
            (*mc.receiver).clone()
        }
        _ => expr.clone(),
    };

    let inspect = match &peeled {
        syn::Expr::Reference(r) => &*r.expr,
        other => other,
    };
    let op = match inspect {
        syn::Expr::MethodCall(mc) if mc.args.is_empty() && mc.method == "address" => {
            SeedTrailingOp::Address
        }
        _ => SeedTrailingOp::Other,
    };
    (peeled, op)
}

/// Generates the `let #ident = ...;` binding for one element of a
/// `seeds = [...]` array. Shared by `security.rs`'s own PDA-verification
/// seed bindings and `init_cpi.rs`'s signer-seeds bindings, which must
/// derive byte-identical seed material for the same field — previously two
/// independent, textually near-identical reimplementations that could
/// silently drift from each other; this is now the one place that logic
/// lives.
///
/// Two cases:
/// - The expression is a plain field-access path rooted at another
///   zero-copy account field in the same `Accounts` struct: reached via
///   that account's `Deref<Target = T>`, using either its already-bound
///   local (if declared earlier) or a by-index reconstruction (if declared
///   later — its local isn't in scope yet).
/// - Anything else: peeled of a redundant `.as_ref()`/`.as_bytes()`
///   wrapper, then either routed through `ToAddress::address` (if the
///   trailing operation is `.address()`) or used directly via
///   `AsRefByteSlice`.
pub(crate) fn seed_binding_tokens(
    expr: &syn::Expr,
    ident: &syn::Ident,
    current_field_index: usize,
    all_fields: &[ParsedField],
    is_zero_copy: bool,
) -> TokenStream {
    if let Some(root) = seed_field_chain_root(expr) {
        if let Some(other_field) = all_fields
            .iter()
            .find(|f| &f.ident == root && is_zero_copy_account_field(&f.ty, is_zero_copy))
        {
            let replacement = if other_field.index < current_field_index {
                let other_ident = &other_field.ident;
                quote! { #other_ident }
            } else {
                let other_ty = &other_field.ty;
                let other_idx = other_field.index;
                quote! { <#other_ty as naclac_lang::prelude::NaclacAccount>::try_from(&accounts[#other_idx], #other_idx)? }
            };
            let rebuilt = substitute_seed_chain_root(expr, &replacement);
            return quote! { let #ident = #rebuilt; };
        }
    }

    let (peeled, op) = peel_seed_wrapper(expr);
    match op {
        SeedTrailingOp::Address => quote! {
            let #ident = naclac_lang::prelude::ToAddress::address(&(#peeled));
        },
        SeedTrailingOp::Other => quote! {
            let #ident = &#peeled;
        },
    }
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
    if field.is_signer && !is_exact_signer_type(&field.ty) {
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

    // 2c. Rent-Exemption Check
    if field.is_rent_exempt {
        metadata_checks.push(quote! {
            let __rent_exempt_lamports = info.lamports();
            #[cfg(not(feature = "pinocchio"))]
            let __rent_exempt_data_len = info.try_borrow_data()?.len();
            #[cfg(feature = "pinocchio")]
            let __rent_exempt_data_len = info.data().len();

            #[cfg(not(feature = "pinocchio"))]
            let __is_rent_exempt = naclac_lang::solana_program::rent::Rent::get()?
                .is_exempt(__rent_exempt_lamports, __rent_exempt_data_len);
            #[cfg(feature = "pinocchio")]
            let __is_rent_exempt = {
                // Const-rent: same formula established in `realloc.rs`/`init_cpi.rs`
                // for this exact reason (no `Rent::get()` sysvar call available
                // here without pulling in the real sysvar account).
                const __STORAGE_OVERHEAD: u64 = 128;
                const __LAMPORTS_PER_BYTE: u64 = 6960;
                let __required = (__STORAGE_OVERHEAD + __rent_exempt_data_len as u64)
                    .wrapping_mul(__LAMPORTS_PER_BYTE);
                __rent_exempt_lamports >= __required
            };

            if !__is_rent_exempt {
                return Err(naclac_lang::prelude::NaclacError::ConstraintRentExempt.err(#idx));
            }
        });
    }

    // Resolves `expr` to the index of another field in the struct, if it's a
    // bare identifier matching one — used by both `address =` and `owner =`
    // to detect a relational (cross-field) reference.
    let relational_field_index = |expr: &syn::Expr| -> Option<usize> {
        if let syn::Expr::Path(expr_path) = expr {
            if let Some(ident) = expr_path.path.get_ident() {
                return all_fields.iter().position(|f| &f.ident == ident);
            }
        }
        None
    };

    // 3. Address Check
    if let Some(address_expr) = &field.address {
        let is_program_or_interface = matches!(
            crate::type_classify::base_ident(&field.ty)
                .as_ref()
                .map(syn::Ident::to_string)
                .as_deref(),
            Some("Program") | Some("Interface")
        );
        if !is_program_or_interface {
            if let Some(other_idx) = relational_field_index(address_expr) {
                metadata_checks.push(quote! {
                    let __key = (info).address();
                    if __key != naclac_lang::prelude::ToAddress::address(&accounts[#other_idx]) {
                        return Err(naclac_lang::prelude::NaclacError::ConstraintAddress.err(#idx));
                    }
                });
            } else {
                metadata_checks.push(quote! {
                    let __key = (info).address();
                    if __key != naclac_lang::prelude::ToAddress::address(&(#address_expr)) {
                        return Err(naclac_lang::prelude::NaclacError::ConstraintAddress.err(#idx));
                    }
                });
            }
        }
    }

    // 4. Owner Check
    if let Some(owner_expr) = &field.owner {
        if let Some(other_idx) = relational_field_index(owner_expr) {
            metadata_checks.push(quote! {
                let __owner = naclac_lang::prelude::Owner::program_owner(info);
                if __owner != naclac_lang::prelude::ToAddress::address(&accounts[#other_idx]) {
                    return Err(naclac_lang::prelude::NaclacError::ConstraintOwner.err(#idx));
                }
            });
        } else {
            match owner_expr {
                syn::Expr::Lit(expr_lit) => {
                    if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                        let s = lit_str.value();
                        // Decode the base58 owner address at proc-macro compile time
                        // into a byte literal so we never depend on pinocchio::pubkey!.
                        match bs58::decode(&s).into_vec() {
                            Ok(owner_bytes) if owner_bytes.len() == 32 => {
                                let byte_lit: Vec<_> =
                                    owner_bytes.iter().map(|b| quote::quote! { #b }).collect();
                                metadata_checks.push(quote! {
                                    let expected_owner = naclac_lang::prelude::Address::new_from_array([ #(#byte_lit),* ]);
                                    let __owner = naclac_lang::prelude::Owner::program_owner(info);
                                    if __owner != naclac_lang::prelude::ToAddress::address(&expected_owner) {
                                        return Err(naclac_lang::prelude::NaclacError::ConstraintOwner.err(#idx));
                                    }
                                });
                            }
                            Ok(_) => {
                                metadata_checks.push(
                                    syn::Error::new_spanned(
                                        lit_str,
                                        "Naclac Error: `owner = \"...\"` must decode to exactly 32 bytes.",
                                    )
                                    .to_compile_error(),
                                );
                            }
                            Err(_) => {
                                metadata_checks.push(
                                    syn::Error::new_spanned(
                                        lit_str,
                                        "Naclac Error: invalid base58 address in `owner = \"...\"`.",
                                    )
                                    .to_compile_error(),
                                );
                            }
                        }
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
    } else if crate::type_classify::is_account_wrapper(&field.ty)
        && !crate::type_classify::inner_is(&field.ty, "Mint")
        && !crate::type_classify::inner_is(&field.ty, "TokenAccount")
        && field.init_config.is_none()
    {
        // DEFAULT OWNER CHECK: verify this account is owned by the current
        // program, in every mode (Borsh and zero-copy alike). This overlaps
        // with discriminator validation (`Account<T>::try_from` and its
        // zero-copy branch's `load` both reject a mismatched discriminator),
        // but is kept unconditionally as defense-in-depth and for a clearer,
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

        let pda_program_tokens = if let Some(prog_expr) = &field.pda_program {
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

        // Precompute requires the PDA's program ID to be known at macro-expansion
        // time: the crate's own program ID by default, or, when `seeds::program = X`
        // is set, only if X resolves to a literal address via the same source-scan
        // `resolve_seed_literal` uses for seed bytes (see `resolve_foreign_program_id`,
        // below) — e.g. a `const PUMP_PROGRAM_ID: Address = address!("...")` declared
        // elsewhere in `src/`. Any other `pda_program` expression (a runtime value,
        // method call, cross-crate constant, etc.) falls through to the dynamic
        // hash-and-compare path; that fallback never changes what a successful
        // precompute verifies, only whether precompute happens at all.
        let resolved_foreign_program_id: Option<(String, String)> = field
            .pda_program
            .as_ref()
            .and_then(resolve_program_id_literal);

        if all_literals && (field.pda_program.is_none() || resolved_foreign_program_id.is_some()) {
            let own_program_lookup = || {
                let crate_name = std::env::var("CARGO_PKG_NAME").unwrap_or_default();
                find_program_id(&crate_name)
            };
            let program_id_result = match &resolved_foreign_program_id {
                Some(resolved) => Some(resolved.clone()),
                None => own_program_lookup(),
            };
            if let Some((prog_id_str, path_str)) = program_id_result {
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
        let seed_bindings: Vec<TokenStream> = seed_array
            .elems
            .iter()
            .enumerate()
            .map(|(i, expr)| {
                let ident = format_ident!("__seed_{}_{}", field_ident, i);
                seed_binding_tokens(expr, &ident, field.index, all_fields, is_zero_copy)
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

            // `resolve_seed_literal` got these bytes by parsing source text,
            // not by asking the compiler to resolve the identifier — proc
            // macros run before name resolution, so no such API exists here.
            let literal_seed_exprs: Vec<_> = seed_array.elems.iter().collect();
            // Same reasoning as the seed-expr re-reference below: when
            // `seeds::program = X` is what made this PDA precomputable, `X`
            // itself never appears in the generated code (its address is
            // already baked into `pda_byte_lit`) — re-reference it too, or a
            // foreign-program-only import spuriously looks unused.
            let foreign_program_expr: Vec<&syn::Expr> = field.pda_program.iter().collect();
            pda_creation_logic = quote! {
                const _: &[u8] = include_bytes!(#toml_path_str);
                // Not dead code: re-references the user's seed constant so
                // rustc's post-expansion resolution sees their `use` as
                // used, avoiding a spurious unused-import warning.
                #(let _ = &#literal_seed_exprs;)*
                #(let _ = &#foreign_program_expr;)*
                let expected_pda = naclac_lang::prelude::Address::new_from_array([ #(#pda_byte_lit),* ]);
                let expected_bump = #bump_val;
            };

            if let Some(PdaBump::Explicit(bump_expr)) = &field.pda_bump {
                if field.init_config.is_some() {
                    let err = syn::Error::new_spanned(
                        &field.ident,
                        "Naclac Error: `bump = <expr>` cannot be combined with `init` on a precomputed PDA. \
                         Account creation must use the canonical bump (write `bump` without a value).",
                    ).to_compile_error();
                    return (err.clone(), err);
                }
                bump_verification_logic = quote! {
                    if expected_bump != (#bump_expr) {
                        return Err(naclac_lang::prelude::NaclacError::ConstraintSeeds.err(#idx));
                    }
                };
            }
        } else {
            // Dynamic PDA path (hash-and-compare optimization or find_program_address)
            let mut bump_expr = quote! {};

            match &field.pda_bump {
                Some(PdaBump::Auto) if field.init_config.is_none() => {
                    let is_spl_type = crate::type_classify::inner_is(&field.ty, "TokenAccount")
                        || crate::type_classify::inner_is(&field.ty, "Mint");
                    if is_zero_copy_account_field(&field.ty, is_zero_copy) && !is_spl_type {
                        // Account<T>'s zero-copy branch implements Deref<Target=T>: access .bump directly.
                        bump_expr = quote! { #field_ident.bump };
                    } else if crate::type_classify::is_exactly(&field.ty, "Account") && !is_spl_type
                    {
                        // Only reachable for genuine Borsh `Account<T>` now — a
                        // zero-copy `Account<T>` (pinocchio, or solana zero-copy
                        // via the prelude alias) already took the branch above.
                        // `is_zero_copy` (resolved by the caller via
                        // `caller_has_feature`, not a raw `cfg`) is already
                        // known false here, but branch on it explicitly rather
                        // than assume, matching how `Account<T>`'s own shape
                        // is chosen elsewhere.
                        bump_expr = if is_zero_copy {
                            quote! { #field_ident.bump }
                        } else {
                            quote! { #field_ident.data.bump }
                        };
                    } else {
                        use_find_pda = true;
                    }
                }
                Some(PdaBump::Auto) => {
                    use_find_pda = true;
                }
                Some(PdaBump::Explicit(parsed_bump_expr)) => {
                    bump_expr = quote! { (#parsed_bump_expr) };
                    bump_verification_logic = quote! {
                        if expected_bump != (#parsed_bump_expr) {
                            return Err(naclac_lang::prelude::NaclacError::ConstraintSeeds.err(#idx));
                        }
                    };
                }
                None => {
                    use_find_pda = true;
                }
            }

            if use_find_pda {
                pda_creation_logic = quote! {
                    compile_error!("Naclac Error: PDA validation seeds constraint requires a 'bump' value or attribute to enable hash-and-compare optimization. On-chain 'find_program_address' is strictly banned in Naclac. Example: #[account(seeds = [...], bump)]");
                };
            } else {
                // Hash-and-compare optimization (0 CU PDA verify if owned, or ~100 CU hash instead of ~1000 CU on-curve check)
                // — delegates to `naclac_lang::prelude::derive_program_address`,
                // the one shared implementation of this formula, rather than
                // inlining its own copy.
                pda_creation_logic = quote! {
                    #(#seed_bindings)*
                    let expected_bump = #bump_expr;
                    let __pda_program = naclac_lang::prelude::ToAddress::address(&#pda_program_tokens);
                    let __seeds: &[&[u8]] = &[ #( #seed_refs, )* ];
                    let expected_pda = naclac_lang::prelude::derive_program_address(
                        __seeds,
                        expected_bump,
                        &__pda_program,
                    );
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
    } else if field.pda_bump.is_some() {
        constraint_checks.push(quote! {
            compile_error!("Naclac Error: `bump` requires `seeds = [...]` on the same field — a bare `bump` with no `seeds` has no PDA to verify against and would otherwise be silently ignored. Example: #[account(seeds = [...], bump)]");
        });
    }

    // 6. Token Validation (for existing accounts)
    //
    // Also runs for `init_if_needed` fields, not just plain existing
    // accounts: `init_cpi.rs`'s `mint::`/`token::` branches only run their
    // creation CPI inside `if info.data_is_empty()`, so an `init_if_needed`
    // field whose account already exists gets no creation CPI *and*, before
    // this condition included `is_init_if_needed`, no validation either —
    // any pre-existing account satisfied `mint::decimals =`/`mint::authority
    // =`/etc regardless of its actual contents. Safe to run unconditionally
    // here because `init_logic` always executes before `constraint_checks`
    // (see `accounts.rs`'s per-field emission order): on a freshly created
    // account the check trivially passes against the values just written: on
    // a pre-existing one it now actually verifies them.
    if field.init_config.is_none() || field.is_init_if_needed {
        let field_ident = &field.ident;
        let data_prelude = borrow_field_data_tokens(field_ident);
        let mut token_checks = Vec::new();

        if let Some(mint_expr) = &field.token_mint {
            let actual_mint_access = resolve_smart_key(mint_expr, all_fields);
            token_checks.push(quote! {
                {
                    #data_prelude
                    let actual_mint: naclac_lang::prelude::Address = #actual_mint_access;
                    naclac_lang::token::TokenAccount::check_mint(&data, &actual_mint)
                        .map_err(|e| e.err(#idx))?;
                }
            });
        }

        if let Some(authority_expr) = &field.token_authority {
            let actual_authority_access = resolve_smart_key(authority_expr, all_fields);
            token_checks.push(quote! {
                {
                    #data_prelude
                    let actual_authority: naclac_lang::prelude::Address = #actual_authority_access;
                    naclac_lang::token::TokenAccount::check_authority(&data, &actual_authority)
                        .map_err(|e| e.err(#idx))?;
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
                    #data_prelude
                    let actual_decimals: u8 = #decimals_expr;
                    naclac_lang::token::Mint::check_decimals(&data, actual_decimals)
                        .map_err(|e| e.err(#idx))?;
                }
            });
        }

        if let Some(authority_expr) = &field.mint_authority {
            let actual_authority_access = resolve_smart_key(authority_expr, all_fields);
            mint_checks.push(quote! {
                {
                    #data_prelude
                    let actual_authority: naclac_lang::prelude::Address = #actual_authority_access;
                    naclac_lang::token::Mint::check_authority(&data, &actual_authority)
                        .map_err(|e| e.err(#idx))?;
                }
            });
        }

        if let Some(freeze_authority_expr) = &field.mint_freeze_authority {
            let actual_freeze_authority_access =
                resolve_smart_key(freeze_authority_expr, all_fields);
            mint_checks.push(quote! {
                {
                    #data_prelude
                    let actual_freeze: naclac_lang::prelude::Address = #actual_freeze_authority_access;
                    naclac_lang::token::Mint::check_freeze_authority(&data, &actual_freeze)
                        .map_err(|e| e.err(#idx))?;
                }
            });
        }

        if !mint_checks.is_empty() {
            constraint_checks.push(quote! {
                { #(#mint_checks)* }
            });
        }
    }

    // 7. Associated Token Validation (for existing accounts)
    //
    // Deliberately narrower than block 6's gate above: an `init_if_needed`
    // associated-token field doesn't need this, since `init_cpi.rs`'s ATA
    // branch calls `create`/`create_idempotent` unconditionally every call
    // (not gated behind `data_is_empty()`), so the real Associated Token
    // Program CPI always runs and self-validates the account's address from
    // `(authority, mint, token_program)` on its own — widening this gate the
    // same way as block 6 would wrongly force `associated_token::bump =` to
    // be required on every `init_if_needed` ATA field, which never needed it
    // (confirmed the hard way: doing so broke `create_ata_idempotent.rs`).
    if field.init_config.is_none() {
        let field_ident = &field.ident;
        let data_prelude = borrow_field_data_tokens(field_ident);
        // Entering this branch on EITHER attribute alone (not just both) is
        // deliberate: it's what lets the match arms below emit a clear
        // compile_error! when only part of the required trio is present,
        // instead of the constraint silently doing nothing — which is
        // exactly the bug this replaces (see
        // naclac-token/docs/04-associated-token-existing-account-gap.md).
        if field.associated_token_mint.is_some() || field.associated_token_authority.is_some() {
            match (
                field.associated_token_mint.as_ref(),
                field.associated_token_authority.as_ref(),
                field.associated_token_bump.as_ref(),
            ) {
                (Some(ata_mint_expr), Some(ata_authority_expr), Some(ata_bump_expr)) => {
                    let actual_mint_access = resolve_smart_key(ata_mint_expr, all_fields);
                    let actual_authority_access = resolve_smart_key(ata_authority_expr, all_fields);

                    match crate::instruction::init_cpi::resolve_token_program_idx(field, all_fields)
                    {
                        Ok(token_program_idx) => {
                            constraint_checks.push(quote! {
                                {
                                    #data_prelude
                                    let actual_mint: naclac_lang::prelude::Address = #actual_mint_access;
                                    let actual_authority: naclac_lang::prelude::Address = #actual_authority_access;
                                    naclac_lang::token::TokenAccount::check_mint(&data, &actual_mint)
                                        .map_err(|e| e.err(#idx))?;
                                    naclac_lang::token::TokenAccount::check_authority(&data, &actual_authority)
                                        .map_err(|e| e.err(#idx))?;

                                    let __token_program_addr = naclac_lang::prelude::ToAddress::address(&accounts[#token_program_idx]);
                                    let __actual_owner = naclac_lang::prelude::Owner::program_owner(info);
                                    if __actual_owner != __token_program_addr {
                                        return Err(naclac_lang::prelude::NaclacError::ConstraintOwner.err(#idx));
                                    }

                                    let __ata_bump: u8 = #ata_bump_expr;
                                    let __actual_address = (info).address();
                                    naclac_lang::associated_token::check_associated_token_address(
                                        &__actual_address,
                                        &actual_authority,
                                        &__token_program_addr,
                                        &actual_mint,
                                        __ata_bump,
                                    ).map_err(|e| e.err(#idx))?;
                                }
                            });
                        }
                        Err(err) => constraint_checks.push(err),
                    }
                }
                (None, _, _) => {
                    constraint_checks.push(quote! {
                        compile_error!("Naclac Error: associated_token::mint must be specified alongside associated_token::authority and associated_token::bump. Example: #[account(associated_token::mint = mint, associated_token::authority = owner, associated_token::bump = <precomputed_bump>)]");
                    });
                }
                (_, None, _) => {
                    constraint_checks.push(quote! {
                        compile_error!("Naclac Error: associated_token::authority must be specified alongside associated_token::mint and associated_token::bump. Example: #[account(associated_token::mint = mint, associated_token::authority = owner, associated_token::bump = <precomputed_bump>)]");
                    });
                }
                (_, _, None) => {
                    constraint_checks.push(quote! {
                        compile_error!("Naclac Error: associated_token::bump must be specified on existing (non-init) associated_token accounts. On-chain 'find_program_address' is strictly banned in Naclac, so the canonical ATA bump must be supplied explicitly. Example: #[account(associated_token::mint = mint, associated_token::authority = owner, associated_token::bump = <precomputed_bump>)]");
                    });
                }
            }
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

/// Resolves a constraint's target expression (e.g. the `mint_authority` in
/// `mint::authority = mint_authority`) to its address. Resolves via indexed
/// `accounts[idx]` access into the raw pre-walked slice — the same
/// technique `get_account_reference` (`init_cpi.rs`) uses — rather than
/// referencing a typed `self.field`-style local, so the referenced field
/// may be declared anywhere in the struct relative to the field whose
/// constraint references it. `accounts` is a `load_and_validate` parameter,
/// in scope and fully populated before any per-field loading/validation
/// runs, so this is always valid regardless of field declaration order.
fn resolve_smart_key(expr: &syn::Expr, all_fields: &[ParsedField]) -> TokenStream {
    if let syn::Expr::Path(expr_path) = expr {
        if let Some(ident) = expr_path.path.get_ident() {
            if let Some(pos) = all_fields.iter().position(|f| &f.ident == ident) {
                let target_field = &all_fields[pos];
                if crate::type_classify::is_account_wrapper(&target_field.ty)
                    || crate::type_classify::is_exactly(&target_field.ty, "AccountInfo")
                {
                    return quote! { naclac_lang::prelude::ToAddress::address(&accounts[#pos]) };
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

                // Account<T> implements Deref<Target=T> in both its Borsh and
                // zero-copy branches, so fields are reachable via auto-deref
                // regardless of backend.
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
            let ident_str = expr_path.path.get_ident()?.to_string();
            let (const_expr, _file_path) = find_const_expr(&ident_str)?;
            expr_to_bytes(&const_expr)
        }
        _ => None,
    }
}

/// Locates the top-level `const IDENT: T = <expr>;` declaration for
/// `ident_str` anywhere under the crate's own `src/` tree. Must resolve
/// to the same constant initializer expression naclac-syn's IDL generator
/// sees, since the seed/PDA-program-id precomputation path depends on it.
fn find_const_expr(ident_str: &str) -> Option<(syn::Expr, String)> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").ok()?;
    let src_dir = std::path::Path::new(&manifest_dir).join("src");
    for file_path in collect_rs_files(&src_dir) {
        let Ok(content) = std::fs::read_to_string(&file_path) else {
            continue;
        };
        let Ok(syntax_tree) = syn::parse_file(&content) else {
            continue;
        };
        for item in &syntax_tree.items {
            if let syn::Item::Const(item_const) = item {
                if item_const.ident == ident_str {
                    return Some(((*item_const.expr).clone(), file_path.to_str()?.to_string()));
                }
            }
        }
    }
    None
}

/// True if the component struct named `ident_str` (searched the same way
/// `find_const_expr` finds top-level consts — anywhere under the crate's
/// own `src/` tree, via real `syn` parsing) declares a field literally
/// named `bump`. Used to decide whether an `init`-bump write-back into
/// `.bump` is even possible for a given account type: a component with no
/// `bump` field of its own has nothing to cache the derived bump into, and
/// that's a legitimate, deliberate choice (nothing else in the program
/// ever reads this account's bump back), not an error.
pub(crate) fn component_declares_bump_field(ident_str: &str) -> bool {
    let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") else {
        return false;
    };
    let src_dir = std::path::Path::new(&manifest_dir).join("src");
    for file_path in collect_rs_files(&src_dir) {
        let Ok(content) = std::fs::read_to_string(&file_path) else {
            continue;
        };
        let Ok(syntax_tree) = syn::parse_file(&content) else {
            continue;
        };
        for item in &syntax_tree.items {
            if let syn::Item::Struct(item_struct) = item {
                if item_struct.ident == ident_str {
                    return item_struct
                        .fields
                        .iter()
                        .any(|f| f.ident.as_ref().is_some_and(|id| id == "bump"));
                }
            }
        }
    }
    false
}

/// Extracts raw bytes from a constant initializer expression: a byte-string,
/// string, or single-byte literal, or an array literal of integer elements
/// (`[u8; N]`) — optionally behind a `&` reference or `*` dereference.
fn expr_to_bytes(expr: &syn::Expr) -> Option<Vec<u8>> {
    let expr = match expr {
        syn::Expr::Reference(reference) => &*reference.expr,
        syn::Expr::Unary(syn::ExprUnary {
            op: syn::UnOp::Deref(_),
            expr: inner,
            ..
        }) => &**inner,
        _ => expr,
    };
    match expr {
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(lit_str),
            ..
        }) => Some(lit_str.value().into_bytes()),
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::ByteStr(lit_byte_str),
            ..
        }) => Some(lit_byte_str.value()),
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Byte(lit_byte),
            ..
        }) => Some(vec![lit_byte.value()]),
        syn::Expr::Array(array) => array
            .elems
            .iter()
            .map(|elem| match elem {
                syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Int(lit_int),
                    ..
                }) => lit_int.base10_parse::<u8>().ok(),
                _ => None,
            })
            .collect(),
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

/// Resolves a `seeds::program = X` expression to a literal base58 address,
/// for the precompute path — looks for a top-level `const IDENT: Address =
/// address!("...")` (or `pubkey!("...")`) declaration via real `syn` parsing,
/// mirroring `find_const_expr`. Returns `(base58_address, source_file_path)`
/// — the file path lets the generated code `include_bytes!` it, so editing
/// the constant invalidates the cached expansion, matching `find_program_id`'s
/// own cache-dependency trick for `Naclac.toml`.
fn resolve_program_id_literal(expr: &syn::Expr) -> Option<(String, String)> {
    let syn::Expr::Path(expr_path) = expr else {
        return None;
    };
    let ident_str = expr_path.path.segments.last()?.ident.to_string();

    let (const_expr, file_path) = find_const_expr(&ident_str)?;
    let syn::Expr::Macro(expr_macro) = &const_expr else {
        return None;
    };
    if !expr_macro.mac.path.is_ident("address") && !expr_macro.mac.path.is_ident("pubkey") {
        return None;
    }
    let lit: syn::LitStr = expr_macro.mac.parse_body().ok()?;
    Some((lit.value(), file_path))
}

fn find_program_id(crate_name: &str) -> Option<(String, String)> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").ok()?;
    let mut path = std::path::PathBuf::from(manifest_dir);
    for _ in 0..10 {
        let naclac_toml_path = path.join("Naclac.toml");
        if naclac_toml_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&naclac_toml_path) {
                if let Ok(manifest) = toml::from_str::<toml::Value>(&content) {
                    // Scoped to the active cluster's own `[programs.<cluster>]` table
                    // (defaulting to "localnet", matching every Naclac.toml in this repo)
                    // rather than matching `crate_name` against a key anywhere in the
                    // file — a program name repeated under a different cluster's table
                    // must not be picked up here.
                    let cluster = manifest
                        .get("provider")
                        .and_then(|p| p.get("cluster"))
                        .and_then(|c| c.as_str())
                        .unwrap_or("localnet");
                    let value = manifest
                        .get("programs")
                        .and_then(|programs| programs.get(cluster))
                        .and_then(|table| table.get(crate_name))
                        .and_then(|v| v.as_str())
                        .filter(|v| !v.is_empty());
                    if let Some(value) = value {
                        return Some((value.to_string(), naclac_toml_path.to_str()?.to_string()));
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
