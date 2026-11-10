//! The `seeds = [...]` / `bump` check of a field: derives the expected PDA
//! (in a `const` the compiler evaluates when every seed and the program id
//! are constants, else at runtime) and compares it with the field's account.

use {
    super::seeds::seed_binding_tokens,
    crate::accounts::field::parser::{ParsedField, PdaBump},
    proc_macro2::TokenStream,
    quote::{format_ident, quote},
};

const HASH_ONLY_INELIGIBLE_MSG: &str =
    "Naclac Error: this existing PDA field has no bump, and its type can't discover one. Use \
     Account<T>/InterfaceAccount<T>, or write `bump = <value>` on the field.";

/// Whether `ty` is written as `AccountInfo` or `AccountView`, types that never
/// validate their owner. Bump discovery is refused for them here, with the
/// field's span; every other spelling, an alias included, is refused by the
/// `OWNER_VALIDATED` assertion the discovery code carries.
fn names_unchecked_type(ty: &syn::Type) -> bool {
    crate::type_classify::is_exactly(ty, "AccountInfo")
        || crate::type_classify::is_exactly(ty, "AccountView")
}

/// Which phase of a field's checks the PDA check belongs to.
pub(super) enum PdaChecks {
    /// Runs before the account is loaded: the field is being `init`ed.
    Metadata(TokenStream),
    /// Runs after the account is loaded.
    Constraint(TokenStream),
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

/// Whether `expr` is a path written in ALL_CAPS, the spelling of a constant. A
/// proc macro runs before name resolution and cannot ask the compiler whether
/// a name is a `const`, so this relies on the naming rule that
/// `non_upper_case_globals` and `non_snake_case` enforce. A name that breaks
/// the rule fails to compile, in the `const` the macro generates.
fn is_const_path(expr: &syn::Expr) -> bool {
    let syn::Expr::Path(path) = expr else {
        return false;
    };
    let Some(last) = path.path.segments.last() else {
        return false;
    };
    let name = last.ident.to_string();
    path.qself.is_none()
        && last.arguments.is_empty()
        && name.chars().any(|c| c.is_ascii_uppercase())
        && !name.chars().any(|c| c.is_ascii_lowercase())
}

/// The literal or ALL_CAPS constant inside a seed expression, with any `&`,
/// `.as_ref()` and `.as_bytes()` wrapper removed, or `None` when the seed
/// depends on runtime values.
pub(in crate::accounts) fn const_seed_root(expr: &syn::Expr) -> Option<&syn::Expr> {
    match expr {
        syn::Expr::Reference(reference) => const_seed_root(&reference.expr),
        syn::Expr::MethodCall(call)
            if call.args.is_empty()
                && call.turbofish.is_none()
                && (call.method == "as_ref" || call.method == "as_bytes") =>
        {
            const_seed_root(&call.receiver)
        }
        syn::Expr::Lit(literal)
            if matches!(
                literal.lit,
                syn::Lit::Str(_) | syn::Lit::ByteStr(_) | syn::Lit::Byte(_)
            ) =>
        {
            Some(expr)
        }
        _ if is_const_path(expr) => Some(expr),
        _ => None,
    }
}

/// The seeds, as `&[u8]` const expressions, and the program id expression of
/// a PDA the compiler can evaluate: every seed a literal or constant, and the
/// program the crate's own `crate::ID` or an ALL_CAPS `seeds::program`.
/// `None` when any part depends on runtime values.
fn const_pda(
    field: &ParsedField,
    seed_array: &syn::ExprArray,
) -> Option<(Vec<TokenStream>, TokenStream)> {
    let seeds = seed_array
        .elems
        .iter()
        .map(|elem| {
            const_seed_root(elem)
                .map(|root| quote! { naclac_lang::prelude::ConstSeed(#root).bytes() })
        })
        .collect::<Option<Vec<_>>>()?;
    let program = match &field.pda_program {
        None => quote! { crate::ID },
        Some(expr) if is_const_path(expr) => quote! { #expr },
        Some(_) => return None,
    };
    Some((seeds, program))
}

fn precomputed_creation(
    field: &ParsedField,
    seeds: &[TokenStream],
    program: &TokenStream,
) -> Creation {
    let idx = field.index;
    let pda_const = format_ident!("__NACLAC_PDA_{}", idx);
    let logic = quote! {
        const #pda_const: (naclac_lang::prelude::Address, u8) =
            naclac_lang::prelude::find_program_address_const(&[ #(#seeds),* ], &#program);
        let expected_pda = #pda_const.0;
        let expected_bump = #pda_const.1;
    };

    let mut bump_verification = quote! {};
    if let Some(PdaBump::Explicit(bump_expr)) = &field.pda_bump {
        bump_verification = quote! {
            if expected_bump != (#bump_expr) {
                return Err(naclac_lang::prelude::NaclacError::ConstraintSeeds.err(#idx));
            }
        };
    }
    Creation {
        logic,
        bump_verification,
        skip_address_check: false,
    }
}

/// The runtime derivation, used when the PDA cannot be precomputed.
fn dynamic_creation(field: &ParsedField, seed_array: &syn::ExprArray) -> Creation {
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
    let mut use_stored_or_searched_bump = false;

    match &field.pda_bump {
        Some(PdaBump::Auto) if field.init_config.is_none() => {
            if names_unchecked_type(&field.ty) {
                find_pda_error = Some(HASH_ONLY_INELIGIBLE_MSG.to_string());
            } else {
                use_stored_or_searched_bump = true;
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
                "Naclac Error: `init` under seeds unknown at compile time needs `bump` \
                 (canonical search) or `bump = <value>`. Example: #[account(init, seeds = \
                 [...], bump)]"
                    .to_string(),
            );
        }
        None => {
            if names_unchecked_type(&field.ty) {
                find_pda_error = Some(HASH_ONLY_INELIGIBLE_MSG.to_string());
            } else {
                use_hash_only_search = true;
            }
        }
    }

    let logic = if let Some(msg) = &find_pda_error {
        quote! {
            compile_error!(#msg);
        }
    } else if use_stored_or_searched_bump {
        let ty = &field.ty;
        quote! {
            const _: () = assert!(
                <#ty as naclac_lang::prelude::NaclacAccount>::OWNER_VALIDATED,
                #HASH_ONLY_INELIGIBLE_MSG
            );
            #(#seed_bindings)*
            #pda_program_ref
            #actual_address_ref
            let __seeds: &[&[u8]] = &[ #( #seed_refs, )* ];
            let (expected_pda, expected_bump) =
                match naclac_lang::prelude::NaclacAccount::stored_bump(&#field_ident) {
                    Some(__stored) => (
                        naclac_lang::prelude::derive_program_address(
                            __seeds,
                            __stored,
                            __pda_program,
                        ),
                        __stored,
                    ),
                    None => (
                        *__actual_address,
                        naclac_lang::prelude::find_bump_for_address(
                            __seeds,
                            __pda_program,
                            __actual_address,
                        )
                        .ok_or(naclac_lang::prelude::NaclacError::ConstraintSeeds.err(#idx))?,
                    ),
                };
        }
    } else if use_hash_only_search {
        let ty = &field.ty;
        quote! {
            const _: () = assert!(
                <#ty as naclac_lang::prelude::NaclacAccount>::OWNER_VALIDATED,
                #HASH_ONLY_INELIGIBLE_MSG
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
/// `bump`. A field that creates its account and writes `bump = <expr>` gets a
/// compile error plus placeholder `expected_bump` and `__bump_<field>` locals,
/// so the code generated after it still compiles and the error stands alone.
pub(super) fn pda_checks(field: &ParsedField) -> Option<PdaChecks> {
    let Some(seed_array) = &field.pda_seed else {
        if field.pda_bump.is_some() {
            return Some(PdaChecks::Constraint(quote! {
                compile_error!("Naclac Error: `bump` needs `seeds = [...]` on the same field, \
                                as there is no PDA to verify without them. Example: \
                                #[account(seeds = [...], bump)]");
            }));
        }
        return None;
    };

    if field.init_config.is_some() && matches!(field.pda_bump, Some(PdaBump::Explicit(_))) {
        let error = syn::Error::new_spanned(
            &field.ident,
            "Naclac Error: `init` takes a bare `bump`, not `bump = <expr>`.",
        )
        .to_compile_error();
        let bump_cap = format_ident!("__bump_{}", field.ident);
        return Some(PdaChecks::Metadata(quote! {
            #error
            let expected_bump = 0u8;
            let #bump_cap = expected_bump;
        }));
    }

    let idx = field.index;
    let field_ident = &field.ident;
    let bump_cap = format_ident!("__bump_{}", field_ident);
    let actual_address_ref = actual_address_ref();

    let creation = match const_pda(field, seed_array) {
        Some((seeds, program)) => precomputed_creation(field, &seeds, &program),
        None => dynamic_creation(field, seed_array),
    };
    let Creation {
        logic,
        bump_verification,
        skip_address_check,
    } = creation;

    if field.init_config.is_none() {
        if skip_address_check {
            Some(PdaChecks::Constraint(quote! {
                #logic
                #bump_verification
                let #bump_cap = expected_bump;
            }))
        } else {
            Some(PdaChecks::Constraint(quote! {
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
            }))
        }
    } else {
        Some(PdaChecks::Metadata(quote! {
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
        }))
    }
}

#[cfg(test)]
mod const_pda_tests {
    use super::{const_pda, const_seed_root, is_const_path, pda_checks, PdaChecks};
    use crate::accounts::field::parser::parse_struct_fields;

    fn expr(source: &str) -> syn::Expr {
        syn::parse_str(source).unwrap()
    }

    #[test]
    fn only_all_caps_paths_are_constants() {
        for name in [
            "SEED",
            "SEED_VAULT",
            "crate::SEED",
            "module::GLOBAL_SEED",
            "SEED2",
        ] {
            assert!(is_const_path(&expr(name)), "{name}");
        }
        for name in [
            "seed",
            "Seed",
            "authority",
            "args.id",
            "self::seed",
            "_",
            "X.y",
        ] {
            assert!(!is_const_path(&expr(name)), "{name}");
        }
    }

    #[test]
    fn a_seed_is_const_when_its_root_is_a_literal_or_constant() {
        for seed in [
            "b\"vault\"",
            "\"vault\"",
            "b'x'",
            "SEED",
            "&SEED",
            "SEED.as_ref()",
            "SEED.as_bytes()",
            "&SEED.as_ref()",
            "\"vault\".as_bytes()",
        ] {
            assert!(const_seed_root(&expr(seed)).is_some(), "{seed}");
        }
        for seed in [
            "mint.address().as_ref()",
            "&id.to_le_bytes()",
            "authority",
            "SEED.to_vec()",
            "SEED.as_ref(1)",
            "1u8",
        ] {
            assert!(const_seed_root(&expr(seed)).is_none(), "{seed}");
        }
    }

    fn field(source: &str) -> crate::accounts::field::parser::ParsedField {
        let item: syn::ItemStruct = syn::parse_str(&format!("struct S {{ {source} }}")).unwrap();
        let syn::Fields::Named(named) = &item.fields else {
            panic!("a struct with named fields");
        };
        parse_struct_fields(named).unwrap().remove(0)
    }

    #[test]
    fn a_pda_is_const_only_when_every_part_is() {
        let all_const = field("#[account(seeds = [SEED_A, b\"x\"], bump)] a: Account<A>");
        let seeds = all_const.pda_seed.clone().unwrap();
        assert!(const_pda(&all_const, &seeds).is_some());

        let runtime_seed =
            field("#[account(seeds = [SEED_A, user.address().as_ref()], bump)] a: Account<A>");
        let seeds = runtime_seed.pda_seed.clone().unwrap();
        assert!(const_pda(&runtime_seed, &seeds).is_none());

        let const_program = field(
            "#[account(seeds = [SEED_A], bump, seeds::program = OTHER_PROGRAM)] a: Account<A>",
        );
        let seeds = const_program.pda_seed.clone().unwrap();
        assert!(const_pda(&const_program, &seeds).is_some());

        let runtime_program = field(
            "#[account(seeds = [SEED_A], bump, seeds::program = other.address())] a: Account<A>",
        );
        let seeds = runtime_program.pda_seed.clone().unwrap();
        assert!(const_pda(&runtime_program, &seeds).is_none());
    }

    fn rendered(field: &crate::accounts::field::parser::ParsedField) -> String {
        match pda_checks(field) {
            Some(PdaChecks::Metadata(tokens) | PdaChecks::Constraint(tokens)) => {
                tokens.to_string()
            }
            None => String::new(),
        }
    }

    #[test]
    fn every_creation_takes_a_bare_bump_for_every_kind_of_seed() {
        for seeds in ["SEED_A", "SEED_A, user.address().as_ref()"] {
            for creation in ["init", "init_if_needed", "init_unchecked"] {
                let explicit = field(&format!(
                    "#[account({creation}, payer = payer, seeds = [{seeds}], bump = args.bump)] \
                     a: Account<A>"
                ));
                assert!(
                    rendered(&explicit).contains("takes a bare"),
                    "{creation} with {seeds}"
                );
                let bare = field(&format!(
                    "#[account({creation}, payer = payer, seeds = [{seeds}], bump)] a: Account<A>"
                ));
                assert!(
                    !rendered(&bare).contains("compile_error"),
                    "{creation} with {seeds}"
                );
            }
        }
    }

    #[test]
    fn an_existing_account_may_still_check_an_explicit_bump() {
        let existing = field(
            "#[account(seeds = [SEED_A, user.address().as_ref()], bump = args.bump)] \
             a: Account<A>",
        );
        assert!(!rendered(&existing).contains("compile_error"));
    }
}
