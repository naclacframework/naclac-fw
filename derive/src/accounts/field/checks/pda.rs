//! The `seeds = [...]` / `bump` check of a field: derives the expected PDA
//! (at compile time when every seed and the program id are known, else at
//! runtime) and compares it with the field's account.

use {
    super::{is_zero_copy_account_field, seeds::seed_binding_tokens},
    crate::accounts::field::{
        compile_time::{
            account_declares_bump_field, declared_program_id, find_pda_at_compile_time,
            resolve_program_id_literal, resolve_seed_literal, seed_literal_root,
        },
        parser::{ParsedField, PdaBump},
    },
    proc_macro2::TokenStream,
    quote::{format_ident, quote},
};

const HASH_ONLY_INELIGIBLE_MSG: &str =
    "Naclac Error: this existing PDA field has no bump, and its type can't have one discovered \
     on-chain. Bump discovery skips the curve check, which is only sound when the account type \
     proves the program that derives it created the account: `Account<T>`/`InterfaceAccount<T>` \
     (owner- and discriminator-validated). Use one of those types, or supply the bump explicitly. \
     Example: #[account(seeds = [...], bump = <precomputed_bump>)]";

/// Which phase of a field's checks the PDA check belongs to.
pub(super) enum PdaChecks {
    /// Runs before the account is loaded: the field is being `init`ed.
    Metadata(TokenStream),
    /// Runs after the account is loaded.
    Constraint(TokenStream),
}

/// A PDA whose address is known while the macro expands.
struct Precomputed {
    address: String,
    bump: u8,
    declaring_file: String,
}

/// How the expected PDA and bump are obtained, and how an explicit bump is
/// verified against them.
struct Creation {
    logic: TokenStream,
    bump_verification: TokenStream,
    /// True when the derivation already guarantees the address matches the
    /// field's own account, so comparing the two would be tautological.
    skip_address_check: bool,
}

/// Binds `__actual_address: &Address` to the field's own account address.
/// Pinocchio borrows it from the raw `info`; the other backends copy it.
fn actual_address_ref() -> TokenStream {
    quote! {
        #[cfg(not(feature = "solana"))]
        let __actual_address: &naclac_lang::prelude::Address =
            naclac_lang::prelude::Address::from_address_ref(info.view.address());
        #[cfg(feature = "solana")]
        let __actual_address_owned: naclac_lang::prelude::Address = (info).address();
        #[cfg(feature = "solana")]
        let __actual_address: &naclac_lang::prelude::Address = &__actual_address_owned;
    }
}

/// Binds `__pda_program: &Address`. Pinocchio borrows the instruction's own
/// `program_id` when no foreign `seeds::program` is given; every other case
/// copies it.
fn pda_program_ref(field: &ParsedField) -> TokenStream {
    if let Some(prog_expr) = &field.pda_program {
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
    }
}

/// The PDA when every seed is a literal and the program id is known at
/// expansion time: the crate's own id, or the literal address a
/// `seeds::program = X` resolves to. Anything else is derived at runtime; that
/// never changes what a successful precompute verifies, only whether it
/// happens.
fn precompute(field: &ParsedField, seed_array: &syn::ExprArray) -> Option<Precomputed> {
    let mut literal_seeds = Vec::new();
    for elem in &seed_array.elems {
        literal_seeds.push(resolve_seed_literal(elem)?);
    }

    let resolved_foreign_program_id: Option<(String, String)> = field
        .pda_program
        .as_ref()
        .and_then(resolve_program_id_literal);
    if field.pda_program.is_some() && resolved_foreign_program_id.is_none() {
        return None;
    }
    let (program_id, declaring_file) = match resolved_foreign_program_id {
        Some(resolved) => resolved,
        None => declared_program_id().ok()?,
    };
    let (address, bump) = find_pda_at_compile_time(&literal_seeds, &program_id)?;
    Some(Precomputed {
        address,
        bump,
        declaring_file,
    })
}

fn precomputed_creation(
    field: &ParsedField,
    seed_array: &syn::ExprArray,
    pda: &Precomputed,
) -> Result<Creation, TokenStream> {
    let idx = field.index;
    let pda_bytes: Vec<u8> = bs58::decode(&pda.address).into_vec().map_err(|_| {
        syn::Error::new_spanned(
            &field.ident,
            "Naclac Error: the precomputed PDA address is not valid base58.",
        )
        .to_compile_error()
    })?;
    let pda_byte_lit: Vec<_> = pda_bytes.iter().map(|b| quote! { #b }).collect();
    let bump_val = pda.bump;
    let declaring_file = &pda.declaring_file;

    // The seed bytes came from parsing source text, as a proc macro runs before
    // name resolution. The seed expressions and any foreign `seeds::program`
    // are re-referenced so the user's imports of them do not look unused.
    let literal_seed_exprs: Vec<_> = seed_array.elems.iter().map(seed_literal_root).collect();
    let foreign_program_expr: Vec<&syn::Expr> = field.pda_program.iter().collect();
    let logic = quote! {
        const _: &[u8] = include_bytes!(#declaring_file);
        #(let _ = &#literal_seed_exprs;)*
        #(let _ = &#foreign_program_expr;)*
        let expected_pda = naclac_lang::prelude::Address::new_from_array([ #(#pda_byte_lit),* ]);
        let expected_bump = #bump_val;
    };

    let mut bump_verification = quote! {};
    if let Some(PdaBump::Explicit(bump_expr)) = &field.pda_bump {
        if field.init_config.is_some() {
            return Err(syn::Error::new_spanned(
                &field.ident,
                "Naclac Error: `bump = <expr>` cannot be combined with `init` on a precomputed PDA. \
                 Account creation must use the canonical bump (write `bump` without a value).",
            )
            .to_compile_error());
        }
        bump_verification = quote! {
            if expected_bump != (#bump_expr) {
                return Err(naclac_lang::prelude::NaclacError::ConstraintSeeds.err(#idx));
            }
        };
    }
    Ok(Creation {
        logic,
        bump_verification,
        skip_address_check: false,
    })
}

/// The runtime derivation, used when the PDA cannot be precomputed.
fn dynamic_creation(
    field: &ParsedField,
    seed_array: &syn::ExprArray,
    is_zero_copy: bool,
) -> Creation {
    let idx = field.index;
    let field_ident = &field.ident;
    let pda_program_ref = pda_program_ref(field);
    let actual_address_ref = actual_address_ref();

    // Each seed is bound to a local first to avoid temporary-drop errors (E0716).
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

    let mut bump_expr = quote! {};
    let mut bump_verification = quote! {};
    let mut find_pda_error: Option<String> = None;
    let mut use_manual_search = false;
    let mut use_hash_only_search = false;

    match &field.pda_bump {
        Some(PdaBump::Auto) if field.init_config.is_none() => {
            let stores_bump = crate::type_classify::wrapped_data_ident(&field.ty)
                .is_some_and(|ident| account_declares_bump_field(&ident.to_string()));
            if is_zero_copy_account_field(&field.ty, is_zero_copy) && stores_bump {
                // A zero-copy `Account<T>` derefs to `T`, so `.bump` is direct.
                bump_expr = quote! { #field_ident.bump };
            } else if crate::type_classify::is_exactly(&field.ty, "Account") && stores_bump {
                // A heap-layout `Account<T>` keeps `T` behind `.data`.
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
            // Only reached for a fresh PDA (`init`) under seeds unknown until
            // runtime: no compile-time precompute is possible and no
            // client-supplied bump can be trusted, so a real canonical search
            // is the only way to establish the bump.
            use_manual_search = true;
        }
        Some(PdaBump::Explicit(parsed_bump_expr)) => {
            bump_expr = quote! { (#parsed_bump_expr) };
            bump_verification = quote! {
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

    let logic = if let Some(msg) = &find_pda_error {
        quote! {
            compile_error!(#msg);
        }
    } else if use_hash_only_search {
        let ty = &field.ty;
        quote! {
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
        }
    } else if use_manual_search {
        // The one place a bounded on-chain canonical search is allowed,
        // delegated to the shared `find_program_address`.
        quote! {
            #(#seed_bindings)*
            #pda_program_ref
            let __seeds: &[&[u8]] = &[ #( #seed_refs, )* ];
            let (expected_pda, expected_bump) =
                naclac_lang::prelude::find_program_address(__seeds, __pda_program)
                    .map_err(|_| naclac_lang::prelude::NaclacError::ConstraintSeeds.err(#idx))?;
        }
    } else {
        // Hash-and-compare against the shared `derive_program_address`.
        quote! {
            #(#seed_bindings)*
            let expected_bump = #bump_expr;
            #pda_program_ref
            let __seeds: &[&[u8]] = &[ #( #seed_refs, )* ];
            let expected_pda = naclac_lang::prelude::derive_program_address(
                __seeds,
                expected_bump,
                __pda_program,
            );
        }
    };

    Creation {
        logic,
        bump_verification,
        // `find_bump_for_address` only returns a bump whose derived address
        // equals the field's own address.
        skip_address_check: use_hash_only_search,
    }
}

/// The PDA check for `field`, or `None` when it has neither `seeds` nor
/// `bump`. `Err` is a compile error that replaces the field's checks.
pub(super) fn pda_checks(
    field: &ParsedField,
    is_zero_copy: bool,
) -> Result<Option<PdaChecks>, TokenStream> {
    let Some(seed_array) = &field.pda_seed else {
        if field.pda_bump.is_some() {
            return Ok(Some(PdaChecks::Constraint(quote! {
                compile_error!("Naclac Error: `bump` requires `seeds = [...]` on the same field — a bare `bump` with no `seeds` has no PDA to verify against and would otherwise be silently ignored. Example: #[account(seeds = [...], bump)]");
            })));
        }
        return Ok(None);
    };

    let idx = field.index;
    let field_ident = &field.ident;
    let bump_cap = format_ident!("__bump_{}", field_ident);
    let actual_address_ref = actual_address_ref();

    let creation = match precompute(field, seed_array) {
        Some(pda) => precomputed_creation(field, seed_array, &pda)?,
        None => dynamic_creation(field, seed_array, is_zero_copy),
    };
    let Creation {
        logic,
        bump_verification,
        skip_address_check,
    } = creation;

    if field.init_config.is_none() {
        if skip_address_check {
            Ok(Some(PdaChecks::Constraint(quote! {
                #logic
                #bump_verification
                let #bump_cap = expected_bump;
            })))
        } else {
            Ok(Some(PdaChecks::Constraint(quote! {
                #logic
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
                #bump_verification
                let #bump_cap = expected_bump;
            })))
        }
    } else {
        Ok(Some(PdaChecks::Metadata(quote! {
            #logic
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
            #bump_verification
            let #bump_cap = expected_bump;
        })))
    }
}
