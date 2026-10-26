//! # Instruction Security Constraints
//!
//! Generates the actual enforcement logic for `#[account(...)]` constraints.
//! This module handles two phases of security checks:
//! 1. **Immediate Metadata Checks**: Validates raw `AccountInfo` properties (signer, mut, owner, PDA seeds).
//! 2. **Relational Checks**: Validates cross-account relationships (`field = target` relations) after all accounts are parsed.

use {
    crate::instruction::parser::{ParsedField, PdaBump},
    proc_macro2::{Ident, TokenStream},
    quote::{format_ident, quote},
    syn::spanned::Spanned,
};

/// Common "borrow this field's raw account data" prelude shared by every
/// `token::*`/`mint::*`/`associated_token::*` existing-account check —
/// these constraints validate raw bytes directly against the fixed SPL
/// Token/Token-2022 layout (see `naclac_token::token::{TokenAccount, Mint}`'s
/// `check_*` methods), independent of whether the field has been hydrated as
/// a typed zero-copy/Borsh account yet.
fn borrow_field_data_tokens(field_ident: &syn::Ident) -> TokenStream {
    quote! {
        #[cfg(feature = "solana")]
        let __data_info = #field_ident.to_account_info();
        #[cfg(feature = "solana")]
        let data = __data_info.try_borrow_data()?;
        #[cfg(not(feature = "solana"))]
        let __data_info = naclac_lang::prelude::ToAccountInfo::to_account_info(&#field_ident);
        #[cfg(not(feature = "solana"))]
        let data = __data_info.data();
    }
}

fn is_exact_signer_type(ty: &syn::Type) -> bool {
    crate::type_classify::is_exactly(ty, "Signer")
}

const HASH_ONLY_INELIGIBLE_MSG: &str =
    "Naclac Error: this existing PDA field has no bump, and its type can't have one discovered \
     on-chain. Bump discovery skips the curve check, which is only sound when the account type \
     proves the program that derives it created the account: `Account<T>`/`InterfaceAccount<T>` \
     (owner- and discriminator-validated). Use one of those types, or supply the bump explicitly. \
     Example: #[account(seeds = [...], bump = <precomputed_bump>)]";

/// True if this field's declared type is an account wrapper
/// (`Account<T>`/`InterfaceAccount<T>`) *and* the whole crate is compiled in
/// zero-copy mode (the `bytemuck` layout — the same condition both wrapper
/// types themselves cfg-route on internally, see
/// `core/src/wrappers/accounts/`). `program_is_zero_copy` is computed
/// once by the caller from the crate's own feature flags and passed in.
pub(crate) fn is_zero_copy_account_field(ty: &syn::Type, program_is_zero_copy: bool) -> bool {
    crate::type_classify::is_account_wrapper(ty) && program_is_zero_copy
}

/// The "real" trailing operation on a seed expression, once any wrapping
/// `.as_ref()`/`.as_bytes()` call has been peeled off.
enum SeedTrailingOp {
    /// A `.address()` call — routed through `ToAddress::address`, which has
    /// an identity impl on `Address` (`core/src/wrappers/mod.rs`), so
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
/// A seed naming another field of the struct refers to that field's loaded
/// local; `load_order` loads every such field first, whatever the
/// declaration order. The expression is peeled of a redundant
/// `.as_ref()`/`.as_bytes()` wrapper, then either routed through
/// `ToAddress::address` (if the trailing operation is `.address()`) or used
/// directly via `AsRefByteSlice`.
pub(crate) fn seed_binding_tokens(expr: &syn::Expr, ident: &syn::Ident) -> TokenStream {
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

    // Binds `__actual_address: &Address` to this field's own account
    // address. Pinocchio borrows it from the raw `info`; the other backends
    // copy it.
    let actual_address_ref = quote! {
        #[cfg(not(feature = "solana"))]
        let __actual_address: &naclac_lang::prelude::Address =
            naclac_lang::prelude::Address::from_address_ref(info.view.address());
        #[cfg(feature = "solana")]
        let __actual_address_owned: naclac_lang::prelude::Address = (info).address();
        #[cfg(feature = "solana")]
        let __actual_address: &naclac_lang::prelude::Address = &__actual_address_owned;
    };

    // 1-2b. Signer / mutability / executable checks. On pinocchio these are
    // enforced by `load_and_validate`'s declared-account walk before
    // any field loader runs, so only the other backends emit them here.
    if field.is_signer && !is_exact_signer_type(&field.ty) {
        metadata_checks.push(quote! {
            #[cfg(feature = "solana")]
            {
                if !info.is_signer {
                    return Err(naclac_lang::prelude::NaclacError::ConstraintSigner.err(#idx));
                }
            }
        });
    }

    if field.is_mut {
        metadata_checks.push(quote! {
            #[cfg(feature = "solana")]
            {
                if !info.is_writable {
                    return Err(naclac_lang::prelude::NaclacError::ConstraintMut.err(#idx));
                }
            }
        });
    }

    if field.is_executable {
        metadata_checks.push(quote! {
            #[cfg(feature = "solana")]
            {
                if !info.executable {
                    return Err(naclac_lang::prelude::NaclacError::ConstraintExecutable.err(#idx));
                }
            }
        });
    }

    // 2c. Rent-Exemption Check
    if field.is_rent_exempt {
        metadata_checks.push(quote! {
            let __rent_exempt_lamports = info.lamports();
            #[cfg(feature = "solana")]
            let __rent_exempt_data_len = info.try_borrow_data()?.len();
            #[cfg(not(feature = "solana"))]
            let __rent_exempt_data_len = info.data().len();

            let __is_rent_exempt = __rent_exempt_lamports
                >= naclac_lang::prelude::const_rent_lamports(__rent_exempt_data_len);

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
        } else {
            let ty = &field.ty;
            metadata_checks.push(quote! {
                const _: () = assert!(
                    <#ty as naclac_lang::prelude::NaclacAccount>::CHECKS_PROGRAM_ID,
                    "no address check is emitted for this field, so its type must check the program id"
                );
            });
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
    } else if field.init_config.is_none() {
        // DEFAULT OWNER CHECK, in every mode (Borsh and zero-copy alike). It is
        // the only owner check a program-owned `Account<T>` gets: its
        // `try_from` verifies the discriminator, which any program can write
        // into an account it owns. Whether it applies is the field type's
        // `NaclacAccount::PROGRAM_OWNED`, never its spelling.
        let ty = &field.ty;
        metadata_checks.push(quote! {
            if <#ty as naclac_lang::prelude::NaclacAccount>::PROGRAM_OWNED {
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

        // Binds `__pda_program: &Address`. Pinocchio borrows the
        // instruction's own `program_id` when no foreign `seeds::program` is
        // given; every other case copies it.
        let pda_program_ref = if let Some(prog_expr) = &field.pda_program {
            quote! {
                let __pda_program_owned = naclac_lang::prelude::ToAddress::address(&#prog_expr);
                let __pda_program: &naclac_lang::prelude::Address = &__pda_program_owned;
            }
        } else {
            quote! {
                #[cfg(not(feature = "solana"))]
                let __pda_program: &naclac_lang::prelude::Address = program_id;
                #[cfg(feature = "solana")]
                let __pda_program_owned = naclac_lang::prelude::ToAddress::address(&program_id);
                #[cfg(feature = "solana")]
                let __pda_program: &naclac_lang::prelude::Address = &__pda_program_owned;
            }
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
                seed_binding_tokens(expr, &ident)
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
        // Set (with a message specific to how this was reached) instead of a
        // bare bool, so the compile_error! below can say precisely what's
        // wrong and what actually fixes it — the two call sites that set
        // this are NOT interchangeable: for a freshly-`init`ed account, the
        // real fix (bare `bump`) triggers a genuine, allowed, bounded
        // on-chain search; for an existing account it needs an already-known
        // bump supplied instead, since no such search happens for one.
        let mut find_pda_error: Option<String> = None;
        let mut use_manual_search = false;
        let mut use_hash_only_search = false;

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
            let literal_seed_exprs: Vec<_> =
                seed_array.elems.iter().map(seed_literal_root).collect();
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
            let mut bump_expr = quote! {};

            match &field.pda_bump {
                Some(PdaBump::Auto) if field.init_config.is_none() => {
                    let stores_bump = crate::type_classify::wrapped_data_ident(&field.ty)
                        .is_some_and(|ident| component_declares_bump_field(&ident.to_string()));
                    if is_zero_copy_account_field(&field.ty, is_zero_copy) && stores_bump {
                        // Account<T>'s zero-copy branch implements Deref<Target=T>: access .bump directly.
                        bump_expr = quote! { #field_ident.bump };
                    } else if crate::type_classify::is_exactly(&field.ty, "Account") && stores_bump
                    {
                        // Only reachable for a heap-allocated (Borsh or Wincode)
                        // `Account<T>`: a zero-copy `Account<T>` already took the
                        // branch above. `is_zero_copy` is already known false here
                        // (the caller, `accounts.rs`'s `build_variant`, calls this
                        // once per representation and only ever passes `false` for
                        // the variants emitted behind the Borsh and Wincode
                        // `#[cfg(...)]`s), but branch on it explicitly rather than
                        // assume, matching how `Account<T>`'s own shape is chosen
                        // elsewhere.
                        bump_expr = if is_zero_copy {
                            quote! { #field_ident.bump }
                        } else {
                            quote! { #field_ident.data.bump }
                        };
                    } else if crate::type_classify::is_account_wrapper(&field.ty) {
                        use_hash_only_search = true;
                    } else {
                        find_pda_error = Some(HASH_ONLY_INELIGIBLE_MSG.to_string());
                    }
                }
                Some(PdaBump::Auto) => {
                    // Reached only when `field.init_config.is_some()`: a fresh
                    // PDA under dynamic (non-literal) seeds, with no
                    // client-supplied bump to (incompletely) trust. This is
                    // the one case naclac's ban on on-chain search doesn't
                    // apply to — no compile-time precomputation is possible
                    // for seeds not known until runtime, so establishing
                    // canonicality here has no alternative to a real search.
                    use_manual_search = true;
                }
                Some(PdaBump::Explicit(parsed_bump_expr)) => {
                    bump_expr = quote! { (#parsed_bump_expr) };
                    bump_verification_logic = quote! {
                        if expected_bump != (#parsed_bump_expr) {
                            return Err(naclac_lang::prelude::NaclacError::ConstraintSeeds.err(#idx));
                        }
                    };
                }
                None if field.init_config.is_some() => {
                    find_pda_error = Some(
                        "Naclac Error: PDA validation seeds constraint requires a 'bump' value or \
                         attribute. This account is being freshly initialized ('init') under seeds \
                         naclac can't resolve at compile time, so add 'bump' to perform a real, \
                         bounded on-chain canonical search — the one case on-chain \
                         'find_program_address' is actually used for — or supply \
                         'bump = <precomputed_bump>' if you already know it off-chain. \
                         Example: #[account(init, seeds = [...], bump)]"
                            .to_string(),
                    );
                }
                None => {
                    if crate::type_classify::is_account_wrapper(&field.ty) {
                        use_hash_only_search = true;
                    } else {
                        find_pda_error = Some(HASH_ONLY_INELIGIBLE_MSG.to_string());
                    }
                }
            }

            if let Some(msg) = &find_pda_error {
                pda_creation_logic = quote! {
                    compile_error!(#msg);
                };
            } else if use_hash_only_search {
                let ty = &field.ty;
                pda_creation_logic = quote! {
                    const _: () = assert!(
                        <#ty as naclac_lang::prelude::NaclacAccount>::OWNER_VALIDATED,
                        "bump discovery skips the curve check, which is sound only for a type whose owner is validated"
                    );
                    #(#seed_bindings)*
                    #pda_program_ref
                    #actual_address_ref
                    let __seeds: &[&[u8]] = &[ #( #seed_refs, )* ];
                    let expected_bump = naclac_lang::prelude::find_bump_for_address(
                        __seeds,
                        __pda_program,
                        __actual_address,
                    )
                    .ok_or(naclac_lang::prelude::NaclacError::ConstraintSeeds.err(#idx))?;
                };
            } else if use_manual_search {
                // Real, bounded on-chain canonical search — the one exception
                // to naclac's ban, scoped to exactly this case (fresh PDA,
                // dynamic seeds, no precomputation possible). Delegates to
                // `naclac_lang::prelude::find_program_address`, the single
                // shared implementation, rather than inlining a copy of the
                // search loop into every `init` field's own codegen.
                pda_creation_logic = quote! {
                    #(#seed_bindings)*
                    #pda_program_ref
                    let __seeds: &[&[u8]] = &[ #( #seed_refs, )* ];
                    let (expected_pda, expected_bump) =
                        naclac_lang::prelude::find_program_address(__seeds, __pda_program)
                            .map_err(|_| naclac_lang::prelude::NaclacError::ConstraintSeeds.err(#idx))?;
                };
            } else {
                // Hash-and-compare optimization (0 CU PDA verify if owned, or ~100 CU hash instead of ~1000 CU on-curve check)
                // — delegates to `naclac_lang::prelude::derive_program_address`,
                // the one shared implementation of this formula, rather than
                // inlining its own copy.
                pda_creation_logic = quote! {
                    #(#seed_bindings)*
                    let expected_bump = #bump_expr;
                    #pda_program_ref
                    let __seeds: &[&[u8]] = &[ #( #seed_refs, )* ];
                    let expected_pda = naclac_lang::prelude::derive_program_address(
                        __seeds,
                        expected_bump,
                        __pda_program,
                    );
                };
            }
        }

        // `find_bump_for_address` only returns a bump whose derived address
        // equals the field's own address, so on that path the address
        // comparison below would be tautological.
        let skip_address_check = use_hash_only_search;
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
                    #[cfg(not(feature = "solana"))]
                    {
                        #actual_address_ref
                        if *__actual_address != expected_pda {
                            return Err(naclac_lang::prelude::NaclacError::ConstraintSeeds.err(#idx));
                        }
                    }
                    #[cfg(feature = "solana")]
                    {
                        if (#field_ident).address() != naclac_lang::prelude::ToAddress::address(&expected_pda) {
                            return Err(naclac_lang::prelude::NaclacError::ConstraintSeeds.err(#idx));
                        }
                    }
                    #bump_verification_logic
                    let #bump_cap = expected_bump;
                });
            }
        } else {
            metadata_checks.push(quote! {
                #pda_creation_logic
                #[cfg(not(feature = "solana"))]
                {
                    #actual_address_ref
                    if *__actual_address != expected_pda {
                        return Err(naclac_lang::prelude::NaclacError::ConstraintSeeds.err(#idx));
                    }
                }
                #[cfg(feature = "solana")]
                {
                    let __key = (info).address();
                    if __key != naclac_lang::prelude::ToAddress::address(&expected_pda) {
                        return Err(naclac_lang::prelude::NaclacError::ConstraintSeeds.err(#idx));
                    }
                }
                #bump_verification_logic
                let #bump_cap = expected_bump;
            });
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
        let mint_key = format_ident!("mint");
        let authority_key = format_ident!("authority");
        let decimals_key = format_ident!("decimals");
        let freeze_authority_key = format_ident!("freeze_authority");

        let mut token_members: Vec<(&Ident, &syn::Expr)> = Vec::new();
        if let Some(value) = &field.token_mint {
            token_members.push((&mint_key, value));
        }
        if let Some(value) = &field.token_authority {
            token_members.push((&authority_key, value));
        }
        if !token_members.is_empty() {
            let namespace = quote! { naclac_lang::token_constraints::token };
            let call = plugin_call_tokens(&namespace, &token_members, field, all_fields);
            constraint_checks.push(quote! { { #call } });
        }

        let mut mint_members: Vec<(&Ident, &syn::Expr)> = Vec::new();
        if let Some(value) = &field.mint_decimals {
            mint_members.push((&decimals_key, value));
        }
        if let Some(value) = &field.mint_authority {
            mint_members.push((&authority_key, value));
        }
        if let Some(value) = &field.mint_freeze_authority {
            mint_members.push((&freeze_authority_key, value));
        }
        if !mint_members.is_empty() {
            let namespace = quote! { naclac_lang::token_constraints::mint };
            let call = plugin_call_tokens(&namespace, &mint_members, field, all_fields);
            constraint_checks.push(quote! { { #call } });
        }
    }

    // 7. Associated Token Validation (for existing accounts)
    //
    // Deliberately narrower than block 6's gate above: an `init_if_needed`
    // associated-token field doesn't need this, since `init_cpi.rs`'s ATA
    // branch calls `create`/`create_idempotent` unconditionally every call
    // (not gated behind `data_is_empty()`), so the real Associated Token
    // Program CPI always runs and self-validates the account's address from
    // `(authority, mint, token_program)` on its own.
    if field.init_config.is_none() {
        let field_ident = &field.ident;
        let data_prelude = borrow_field_data_tokens(field_ident);
        // Entering this branch on EITHER attribute alone (not just both) is
        // deliberate: it's what lets the match arms below emit a clear
        // compile_error! when only one of mint/authority is present,
        // instead of the constraint silently doing nothing — which is
        // exactly the bug this replaces (see
        // token/docs/04-associated-token-existing-account-gap.md).
        if field.associated_token_mint.is_some() || field.associated_token_authority.is_some() {
            match (
                field.associated_token_mint.as_ref(),
                field.associated_token_authority.as_ref(),
                field.associated_token_bump.as_ref(),
            ) {
                (Some(ata_mint_expr), Some(ata_authority_expr), ata_bump_expr) => {
                    let actual_mint_access = resolve_smart_key(ata_mint_expr, all_fields);
                    let actual_authority_access = resolve_smart_key(ata_authority_expr, all_fields);
                    let ata_address_check = match ata_bump_expr {
                        Some(ata_bump_expr) => quote! {
                            let __ata_bump: u8 = #ata_bump_expr;
                            naclac_lang::associated_token::check_associated_token_address(
                                __actual_address,
                                &actual_authority,
                                &__token_program_addr,
                                &actual_mint,
                                __ata_bump,
                            ).map_err(|e| e.err(#idx))?;
                        },
                        None => quote! {
                            naclac_lang::associated_token::find_associated_token_bump(
                                __actual_address,
                                &actual_authority,
                                &__token_program_addr,
                                &actual_mint,
                            )
                            .ok_or(naclac_lang::prelude::NaclacError::ConstraintSeeds.err(#idx))?;
                        },
                    };

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

                                    #actual_address_ref
                                    #ata_address_check
                                }
                            });
                        }
                        Err(err) => constraint_checks.push(err),
                    }
                }
                (None, _, _) => {
                    constraint_checks.push(quote! {
                        compile_error!("Naclac Error: associated_token::mint must be specified alongside associated_token::authority. Example: #[account(associated_token::mint = mint, associated_token::authority = owner)]");
                    });
                }
                (_, None, _) => {
                    constraint_checks.push(quote! {
                        compile_error!("Naclac Error: associated_token::authority must be specified alongside associated_token::mint. Example: #[account(associated_token::mint = mint, associated_token::authority = owner)]");
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

    let core_checks = set_span_recur(quote! { #(#constraint_checks)* }, constraint_checks_span);
    let plugin_checks = generate_plugin_checks(field, all_fields);

    (
        set_span_recur(quote! { #(#metadata_checks)* }, metadata_checks_span),
        quote! { #core_checks #plugin_checks },
    )
}

/// Code for a field's `namespace::key = value` constraints, one group per
/// namespace. It sits after every core check of the field, so a constraint
/// only ever adds a check. The tokens keep the user's own spans so a mistake
/// points at the key.
fn generate_plugin_checks(field: &ParsedField, all_fields: &[ParsedField]) -> TokenStream {
    use quote::ToTokens;

    let mut groups: Vec<(syn::Path, Vec<(&Ident, &syn::Expr)>)> = Vec::new();
    for constraint in &field.plugin_constraints {
        let written = constraint.namespace.to_token_stream().to_string();
        let member = (&constraint.key, &constraint.value);
        match groups
            .iter_mut()
            .find(|(namespace, _)| namespace.to_token_stream().to_string() == written)
        {
            Some((_, members)) => members.push(member),
            None => groups.push((constraint.namespace.clone(), vec![member])),
        }
    }

    let blocks = groups.iter().map(|(namespace, members)| {
        plugin_call_tokens(&namespace.to_token_stream(), members, field, all_fields)
    });
    quote! { #(#blocks)* }
}

/// One namespace's constraint on `field`: the namespace's `RULES` evaluated
/// while compiling (nothing is left in the program), then `Constraint::check`
/// on the loaded field with `Some(value)` for each key written and the
/// namespace's `Args` default for the rest, its error mapped to the field's
/// index. Each value goes through `ConstraintValue`, so its conversion is
/// decided by the type of the key's setter (`#[constraint_args]`). `Args` is
/// built as `<Args as Default>::default().key(value)..`, values in the order
/// written and evaluated in the surrounding function, so `?` in a value
/// works; the fully qualified `Default` cannot be captured by a key named
/// `default`.
pub(crate) fn plugin_call_tokens(
    namespace: &TokenStream,
    members: &[(&Ident, &syn::Expr)],
    field: &ParsedField,
    all_fields: &[ParsedField],
) -> TokenStream {
    let phase = if field.init_config.is_some() {
        quote! { Init }
    } else {
        quote! { Check }
    };
    let field_ident = &field.ident;
    let idx = field.index;
    let key_names = members.iter().map(|(key, _)| key.to_string());
    let keys = members.iter().map(|(key, _)| *key);
    let values = members.iter().map(|(_, value)| {
        let source = value_reference(value, all_fields);
        quote! { naclac_lang::prelude::ConstraintValue::into_value(#source) }
    });
    quote! {
        const _: () = #namespace::RULES.check(
            &[#(#key_names),*],
            naclac_lang::prelude::ConstraintPhase::#phase,
        );
        <#namespace::Constraint as naclac_lang::prelude::AccountConstraint<_>>::check(
            &#field_ident,
            &<#namespace::Args as ::core::default::Default>::default()
                #(.#keys(#values))*,
        )
        .map_err(|e| e.err(#idx))?;
    }
}

/// A reference to a constraint value: the raw account slot or the loaded
/// local of the field it names (`field_reference`), else the expression as
/// written.
fn value_reference(expr: &syn::Expr, all_fields: &[ParsedField]) -> TokenStream {
    if let syn::Expr::Path(expr_path) = expr {
        if let Some(ident) = expr_path.path.get_ident() {
            if let Some(reference) = field_reference(ident, all_fields) {
                return reference;
            }
        }
    }
    quote! { &(#expr) }
}

/// A reference to the field named `ident`, from which its address can be
/// read: the raw `accounts[idx]` slot for an account wrapper or `AccountInfo`
/// (valid wherever the field is declared), else the field's loaded local.
/// `None` when no field has that name.
fn field_reference(ident: &Ident, all_fields: &[ParsedField]) -> Option<TokenStream> {
    let pos = all_fields.iter().position(|f| &f.ident == ident)?;
    let target_field = &all_fields[pos];
    if crate::type_classify::is_account_wrapper(&target_field.ty)
        || crate::type_classify::is_exactly(&target_field.ty, "AccountInfo")
    {
        Some(quote! { &accounts[#pos] })
    } else {
        Some(quote! { &#ident })
    }
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
            if let Some(reference) = field_reference(ident, all_fields) {
                return quote! { naclac_lang::prelude::ToAddress::address(#reference) };
            }
        }
    }
    quote! { naclac_lang::prelude::ToAddress::address(&#expr) }
}

/// PHASE 2: Relational Checks
///
/// Generates validation logic that runs *after* all accounts have been loaded into the instruction struct.
/// This is necessary for `field = target` relations, where one account's data must be compared against
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

        // `token_program` check (`token::`, `mint::` or `associated_token::`)
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

/// The bytes of a seed expression known at macro-expansion time: a string,
/// byte-string or byte literal; a bare constant name; `CONST.as_ref()` or
/// `CONST.as_bytes()` on such a constant; or a `&` reference to any of
/// these. Every form it accepts must yield exactly the bytes the program
/// would hash at runtime, and a subset of the seeds naclac-syn's IDL
/// generator (`parse_single_seed`) records as constants.
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
        syn::Expr::MethodCall(method_call)
            if method_call.args.is_empty()
                && method_call.turbofish.is_none()
                && (method_call.method == "as_ref" || method_call.method == "as_bytes")
                && matches!(&*method_call.receiver, syn::Expr::Path(_)) =>
        {
            resolve_seed_literal(&method_call.receiver)
        }
        syn::Expr::Reference(reference) => resolve_seed_literal(&reference.expr),
        _ => None,
    }
}

/// The literal or constant path inside a seed expression accepted by
/// `resolve_seed_literal`, with the `.as_ref()`/`.as_bytes()` call and `&`
/// wrappers removed.
fn seed_literal_root(expr: &syn::Expr) -> &syn::Expr {
    match expr {
        syn::Expr::MethodCall(method_call) => seed_literal_root(&method_call.receiver),
        syn::Expr::Reference(reference) => seed_literal_root(&reference.expr),
        _ => expr,
    }
}

/// The crate's modules (see `naclac_syn::modules`), read from the crate
/// being compiled.
fn current_crate_modules() -> Vec<naclac_syn::modules::CrateModule> {
    std::env::var("CARGO_MANIFEST_DIR")
        .map(|dir| naclac_syn::modules::crate_modules(std::path::Path::new(&dir)))
        .unwrap_or_default()
}

/// Locates the `const IDENT: T = <expr>;` declaration for `ident_str` in any
/// module of the crate. Must resolve to the same constant initializer
/// expression naclac-syn's IDL generator sees, since the seed/PDA-program-id
/// precomputation path depends on it. Returns `None` when several constants
/// share the name but not the initializer: which one a seed refers to
/// depends on module scope, so the caller must derive that seed at runtime
/// instead of guessing.
fn find_const_expr(ident_str: &str) -> Option<(syn::Expr, String)> {
    use quote::ToTokens;

    let mut found: Option<(syn::Expr, String)> = None;
    for module in current_crate_modules() {
        for item in &module.items {
            if let syn::Item::Const(item_const) = item {
                if item_const.ident == ident_str {
                    match &found {
                        None => {
                            found = Some((
                                (*item_const.expr).clone(),
                                module.file.to_str()?.to_string(),
                            ));
                        }
                        Some((first, _))
                            if first.to_token_stream().to_string()
                                != item_const.expr.to_token_stream().to_string() =>
                        {
                            return None;
                        }
                        Some(_) => {}
                    }
                }
            }
        }
    }
    found
}

/// True if the component struct named `ident_str` (searched in every module
/// of the crate, as `find_const_expr` searches) declares a field literally
/// named `bump`. Used to decide whether an `init`-bump write-back into
/// `.bump` is even possible for a given account type: a component with no
/// `bump` field of its own has nothing to cache the derived bump into, and
/// that's a legitimate, deliberate choice (nothing else in the program
/// ever reads this account's bump back), not an error.
///
/// Several structs of the crate may share the name; the answer is true when
/// any of them declares `bump`. A wrong true is a compile error at the
/// write-back, whereas a wrong false would silently never store the bump.
pub(crate) fn component_declares_bump_field(ident_str: &str) -> bool {
    current_crate_modules()
        .iter()
        .flat_map(|module| module.items.iter())
        .any(|item| {
            matches!(item, syn::Item::Struct(item_struct)
                if item_struct.ident == ident_str
                    && item_struct
                        .fields
                        .iter()
                        .any(|f| f.ident.as_ref().is_some_and(|id| id == "bump")))
        })
}

/// Searched the same way `find_const_expr`/`component_declares_bump_field`
/// find their targets (every module of the crate) — if `ident_str` names a
/// struct with a field whose type is
/// `ZcVec<T>`/`ZcString`, returns that field's name. Used by
/// `program.rs` to give a clear, purpose-built error for an instruction
/// argument whose type is an *opaque* struct name (not itself literally
/// `ZcVec`/`ZcString`, which `program.rs` already recognizes directly)
/// wrapping one of these zero-copy-only view types — the exact same
/// unsound-in-Borsh-mode case `#[instruction_args]`'s own `zc_field` check
/// (macros/src/lib.rs) rejects when expanding that struct itself,
/// just now caught from the *call site* too, so a caller sees one clear
/// error instead of `#[instruction_args]`'s error followed by an unrelated
/// `BorshDeserialize` trait-bound cascade at the point of use.
///
/// When several structs of the crate share the name and disagree, there is no
/// answer (`None`): the diagnostic is skipped rather than naming a field of a
/// struct the argument may not use.
pub(crate) fn dynamic_view_field_of(ident_str: &str) -> Option<String> {
    let modules = current_crate_modules();
    let mut answers = modules
        .iter()
        .flat_map(|module| module.items.iter())
        .filter_map(|item| match item {
            syn::Item::Struct(item_struct) if item_struct.ident == ident_str => {
                Some(item_struct.fields.iter().find_map(|f| {
                    matches!(
                        crate::type_classify::classify_dynamic(&f.ty),
                        crate::type_classify::DynamicKind::ZcVec
                            | crate::type_classify::DynamicKind::ZcString
                    )
                    .then(|| {
                        f.ident
                            .as_ref()
                            .map(|id| id.to_string())
                            .unwrap_or_default()
                    })
                }))
            }
            _ => None,
        });
    let first = answers.next()?;
    let agree = answers.all(|answer| answer == first);
    if agree {
        first
    } else {
        None
    }
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

pub(crate) fn find_program_id(crate_name: &str) -> Option<(String, String)> {
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

pub(crate) fn find_pda_at_compile_time(
    seeds: &[Vec<u8>],
    program_id_str: &str,
) -> Option<(String, u8)> {
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

#[cfg(test)]
mod lookup_tests {
    use super::{component_declares_bump_field, dynamic_view_field_of};

    fn in_crate<T>(source: &str, check: impl FnOnce() -> T) -> T {
        let dir = std::env::temp_dir().join(format!(
            "naclac-macros-security-lookups-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/lib.rs"), source).unwrap();
        let previous = std::env::var_os("CARGO_MANIFEST_DIR");
        std::env::set_var("CARGO_MANIFEST_DIR", &dir);
        let result = check();
        match previous {
            Some(value) => std::env::set_var("CARGO_MANIFEST_DIR", value),
            None => std::env::remove_var("CARGO_MANIFEST_DIR"),
        }
        let _ = std::fs::remove_dir_all(&dir);
        result
    }

    #[test]
    fn same_named_structs_are_resolved_without_picking_one_arbitrarily() {
        in_crate(
            r#"
            pub mod a { pub struct Pool { pub bump: u8 } pub struct Args { pub v: ZcVec<u8> } }
            pub mod b { pub struct Pool { pub x: u8 } pub struct Args { pub v: u8 } }
            pub struct Alone { pub bump: u8, pub s: ZcString }
            pub struct Plain { pub x: u8 }
            pub mod c { pub struct Agreed { pub s: ZcString } }
            pub mod d { pub struct Agreed { pub s: ZcString } }
            "#,
            || {
                assert!(component_declares_bump_field("Pool"));
                assert!(component_declares_bump_field("Alone"));
                assert!(!component_declares_bump_field("Plain"));
                assert!(!component_declares_bump_field("Missing"));

                assert_eq!(dynamic_view_field_of("Args"), None);
                assert_eq!(dynamic_view_field_of("Alone").as_deref(), Some("s"));
                assert_eq!(dynamic_view_field_of("Agreed").as_deref(), Some("s"));
                assert_eq!(dynamic_view_field_of("Plain"), None);
                assert_eq!(dynamic_view_field_of("Missing"), None);
            },
        );
    }
}

#[cfg(test)]
mod plugin_tests {
    use {
        super::{generate_plugin_checks, generate_security_checks},
        crate::instruction::parser::{parse_struct_fields, ParsedField},
    };

    fn parse(fields: &str) -> Vec<ParsedField> {
        let item: syn::ItemStruct = syn::parse_str(&format!("struct S {{ {fields} }}")).unwrap();
        let syn::Fields::Named(named) = &item.fields else {
            panic!("a struct with named fields");
        };
        parse_struct_fields(named).unwrap()
    }

    fn code(fields: &str) -> String {
        let parsed = parse(fields);
        generate_plugin_checks(&parsed[0], &parsed)
            .to_string()
            .replace(' ', "")
    }

    fn constraint_code(fields: &str) -> String {
        let parsed = parse(fields);
        generate_security_checks(&parsed[0], &parsed, false)
            .1
            .to_string()
            .replace(' ', "")
    }

    const INTO_VALUE: &str = "naclac_lang::prelude::ConstraintValue::into_value";

    /// Whether `generated` sets `key` from the value read through `source`.
    fn sets(generated: &str, key: &str, source: &str) -> bool {
        generated.contains(&format!(".{key}({INTO_VALUE}({source}))"))
    }

    #[test]
    fn a_field_without_plugin_keys_generates_nothing() {
        assert_eq!(code("#[account(mut)] v: Account<Vault>"), "");
    }

    #[test]
    fn keys_of_one_namespace_become_one_rules_check_and_one_call() {
        let generated = code(
            "#[account(my_vault::fee_bps = 30, my_vault::admin = admin)] v: Account<Vault>,
             admin: Signer",
        );
        let call = concat!(
            "<my_vault::Constraintasnaclac_lang::prelude::",
            "AccountConstraint<_>>::check(&v,"
        );
        let build = "&<my_vault::Argsas::core::default::Default>::default()";

        assert_eq!(generated.matches("my_vault::RULES.check").count(), 1);
        assert!(generated.contains(r#"&["fee_bps","admin"]"#), "{generated}");
        assert!(generated.contains("ConstraintPhase::Check"), "{generated}");
        assert_eq!(generated.matches(call).count(), 1, "{generated}");
        assert_eq!(generated.matches(build).count(), 1, "{generated}");
        assert!(sets(&generated, "fee_bps", "&(30)"), "{generated}");
        assert!(sets(&generated, "admin", "&admin"), "{generated}");
        assert!(!generated.contains("..::core"), "{generated}");
        assert!(
            generated.contains(".map_err(|e|e.err(0usize))?"),
            "{generated}"
        );
    }

    #[test]
    fn values_are_bound_in_the_order_written() {
        let generated = code("#[account(my_vault::b = 2, my_vault::a = 1)] v: Account<Vault>");
        let first = generated.find(".b(").unwrap();
        let second = generated.find(".a(").unwrap();
        assert!(first < second, "{generated}");
        assert!(sets(&generated, "b", "&(2)"), "{generated}");
        assert!(sets(&generated, "a", "&(1)"), "{generated}");
    }

    #[test]
    fn a_value_naming_an_account_field_reads_its_raw_slot() {
        let generated = code(
            "#[account(my_vault::pool = pool)] v: Account<Vault>,
             pool: Account<Pool>",
        );
        assert!(sets(&generated, "pool", "&accounts[1usize]"), "{generated}");
    }

    #[test]
    fn different_namespaces_are_checked_separately() {
        let generated = code("#[account(a::x = 1, b::y = 2)] v: Account<Vault>");
        assert_eq!(generated.matches("::RULES.check").count(), 2);
        assert!(generated.contains("a::RULES.check"), "{generated}");
        assert!(generated.contains("b::RULES.check"), "{generated}");
    }

    #[test]
    fn a_field_being_created_is_checked_in_the_init_phase() {
        let generated = code(
            "#[account(init, payer = payer, my_vault::fee_bps = 1)] v: Account<Vault>,
             #[account(mut)] payer: Signer",
        );
        assert!(generated.contains("ConstraintPhase::Init"), "{generated}");
    }

    #[test]
    fn token_keys_on_an_existing_account_call_the_token_constraint() {
        let generated = constraint_code(
            "#[account(token::mint = mint, token::authority = owner)] vault: Account<TokenAccount>,
             mint: Account<Mint>,
             owner: Signer",
        );
        let call = concat!(
            "<naclac_lang::token_constraints::token::Constraintas",
            "naclac_lang::prelude::AccountConstraint<_>>::check(&vault,"
        );
        assert_eq!(generated.matches(call).count(), 1, "{generated}");
        assert!(sets(&generated, "mint", "&accounts[1usize]"), "{generated}");
        assert!(sets(&generated, "authority", "&owner"), "{generated}");
        assert!(!generated.contains("check_mint"), "{generated}");
        assert!(!generated.contains("check_authority"), "{generated}");
    }

    #[test]
    fn mint_keys_on_an_existing_mint_call_the_mint_constraint() {
        let generated = constraint_code(
            "#[account(mint::decimals = 6, mint::authority = owner, mint::freeze_authority = owner)]
             mint: Account<Mint>,
             owner: Signer",
        );
        let call = concat!(
            "<naclac_lang::token_constraints::mint::Constraintas",
            "naclac_lang::prelude::AccountConstraint<_>>::check(&mint,"
        );
        assert_eq!(generated.matches(call).count(), 1, "{generated}");
        assert!(sets(&generated, "decimals", "&(6)"), "{generated}");
        assert!(sets(&generated, "authority", "&owner"), "{generated}");
        assert!(
            sets(&generated, "freeze_authority", "&owner"),
            "{generated}"
        );
        assert!(!generated.contains("check_decimals"), "{generated}");
    }

    #[test]
    fn one_token_key_is_passed_alone() {
        let generated = constraint_code(
            "#[account(token::authority = owner)] vault: Account<TokenAccount>,
             owner: Signer",
        );
        assert!(sets(&generated, "authority", "&owner"), "{generated}");
        assert!(!generated.contains(".mint("), "{generated}");
    }

    #[test]
    fn token_keys_are_checked_on_init_if_needed_but_not_on_init() {
        let created = constraint_code(
            "#[account(init, payer = payer, token::mint = mint, token::authority = owner)]
             vault: Account<TokenAccount>,
             mint: Account<Mint>,
             owner: Signer,
             #[account(mut)] payer: Signer",
        );
        assert!(!created.contains("token_constraints"), "{created}");

        let maybe_created = constraint_code(
            "#[account(init_if_needed, payer = payer, token::mint = mint, token::authority = owner)]
             vault: Account<TokenAccount>,
             mint: Account<Mint>,
             owner: Signer,
             #[account(mut)] payer: Signer",
        );
        assert!(
            maybe_created.contains("token_constraints::token::Constraint"),
            "{maybe_created}"
        );
    }
}
