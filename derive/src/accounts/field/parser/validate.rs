//! Validates the combinations of keys read into `RawAttrs` (mutually exclusive
//! creation modes, `payer` with `init`, `realloc` with its payer, and so on),
//! fills the implicit program address, and builds the `ParsedField`.

use {
    super::{attrs::RawAttrs, InitConfig, ParsedField, ReallocConfig, ReallocSpace},
    crate::accounts::field::builtin_namespaces,
    syn::{Ident, Type},
};

/// True if any doc-comment line on this field (once desugared from `///`/`#[doc = "..."]`
/// and trimmed of leading whitespace) starts with `SAFETY:`. Used to require an explicit,
/// human-written justification on every unchecked `AccountInfo` field — mirroring the
/// `// SAFETY:` convention this codebase already uses for `unsafe` blocks.
pub(super) fn has_safety_doc_comment(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("doc") {
            return false;
        }
        let syn::Meta::NameValue(nv) = &attr.meta else {
            return false;
        };
        let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(s),
            ..
        }) = &nv.value
        else {
            return false;
        };
        s.value().trim_start().starts_with("SAFETY:")
    })
}

/// The most seeds a PDA derivation takes; mirrors `naclac_lang`'s
/// `MAX_PDA_SEEDS`, which the derivation functions `assert!` at runtime.
const MAX_PDA_SEEDS: usize = 16;

/// The longest single PDA seed, in bytes.
const MAX_PDA_SEED_LEN: usize = 32;

/// Rejects a `seeds = [...]` list with more than `MAX_PDA_SEEDS` entries or a
/// literal entry (`b"..."` or `"..."`) longer than `MAX_PDA_SEED_LEN` bytes.
/// Non-literal entries can't be measured at expansion time and are not checked.
fn check_seed_limits(seed_array: &syn::ExprArray) -> syn::Result<()> {
    if seed_array.elems.len() > MAX_PDA_SEEDS {
        return Err(syn::Error::new_spanned(
            seed_array,
            format!(
                "Naclac Error: `seeds` lists {} seeds, but a PDA takes at most {MAX_PDA_SEEDS}. \
                 Remove some or combine them into one seed.",
                seed_array.elems.len()
            ),
        ));
    }
    for elem in &seed_array.elems {
        let syn::Expr::Lit(syn::ExprLit { lit, .. }) = elem else {
            continue;
        };
        let len = match lit {
            syn::Lit::ByteStr(bytes) => bytes.value().len(),
            syn::Lit::Str(text) => text.value().len(),
            _ => continue,
        };
        if len > MAX_PDA_SEED_LEN {
            return Err(syn::Error::new_spanned(
                elem,
                format!(
                    "Naclac Error: this seed is {len} bytes, but a PDA seed is at most \
                     {MAX_PDA_SEED_LEN}. Shorten it or hash it down to 32 bytes."
                ),
            ));
        }
    }
    Ok(())
}

pub(super) fn finish(
    raw: RawAttrs,
    idx: usize,
    ident: Ident,
    ty: Type,
    is_optional: bool,
    field_attrs: &[syn::Attribute],
) -> syn::Result<ParsedField> {
    let RawAttrs {
        is_signer,
        is_mut,
        is_alias,
        is_dup,
        is_executable,
        is_rent_exempt,
        pda_program,
        pda_seed,
        pda_bump,
        is_init,
        is_init_if_needed,
        is_init_unchecked,
        close_destination,
        init_payer,
        init_space,
        realloc_space,
        realloc_any_of,
        realloc_grow_only,
        realloc_grow_only_path,
        realloc_payer,
        realloc_zero,
        relations,
        owner,
        address,
        extra_accounts,
        plugin_constraints,
    } = raw;
    let mut init_config = None;
    let mut realloc = None;

    if let Some(super::PdaSeeds::List(seed_array)) = &pda_seed {
        check_seed_limits(seed_array)?;
    }

    if is_dup {
        if is_alias {
            return Err(syn::Error::new_spanned(
                &ident,
                "Naclac Error: `dup` and `unsafe(alias)` are mutually exclusive; `dup` is for \
                 read-only fields, `unsafe(alias)` for `mut` fields.",
            ));
        }
        if is_mut
            || close_destination.is_some()
            || realloc_space.is_some()
            || realloc_any_of.is_some()
        {
            return Err(syn::Error::new_spanned(
                &ident,
                "Naclac Error: a `dup` field is read-only, so it cannot be `mut`, `init`, \
                 `close` or `realloc`. Use `unsafe(alias)` for a mutable duplicate.",
            ));
        }
        if !has_safety_doc_comment(field_attrs) {
            return Err(syn::Error::new_spanned(
                &ident,
                "Naclac Error: `dup` lets this field repeat another account. Add a `/// SAFETY: \
                 ...` doc comment above the field explaining why that is safe.",
            ));
        }
    }

    // The three creation modes are mutually exclusive: combining them leaves
    // unclear which existence check and which CreateAccount CPI applies.
    if [is_init, is_init_if_needed, is_init_unchecked]
        .iter()
        .filter(|b| **b)
        .count()
        > 1
    {
        return Err(syn::Error::new_spanned(
            &ident,
            "Naclac Error: `init`, `init_if_needed` and `init_unchecked` are mutually \
             exclusive; use one.",
        ));
    }

    // A creation mode and `payer =` must appear together: `payer` alone would
    // run a creation CPI on a field left non-mutable, and a creation mode alone
    // would generate no creation logic.
    match (
        is_init || is_init_if_needed || is_init_unchecked,
        init_payer,
    ) {
        (true, Some(payer)) => {
            init_config = Some(InitConfig {
                payer,
                space: init_space,
            });
        }
        (true, None) => {
            return Err(syn::Error::new_spanned(
                &ident,
                "Naclac Error: `init`/`init_if_needed`/`init_unchecked` requires `payer = \
                 <account>` on the same field.",
            ));
        }
        (false, Some(_)) => {
            return Err(syn::Error::new_spanned(
                &ident,
                "Naclac Error: `payer = ...` needs `init`, `init_if_needed` or `init_unchecked` \
                 on the same field.",
            ));
        }
        (false, None) => {}
    }
    if is_init_unchecked {
        if let Some(message) = builtin_namespaces::init_unchecked_error(&plugin_constraints) {
            return Err(syn::Error::new_spanned(&ident, message));
        }
    }
    let realloc_space_variant = match (realloc_space, realloc_any_of) {
        (Some(_), Some(_)) => {
            return Err(syn::Error::new_spanned(
                &ident,
                "Naclac Error: `realloc = <expr>` and `realloc::any_of = [...]` are mutually \
                     exclusive on the same field.",
            ));
        }
        (Some(expr), None) => Some(ReallocSpace::Expr(expr)),
        (None, Some(types)) => Some(ReallocSpace::AnyOf(types)),
        (None, None) => None,
    };
    // `grow_only` only means something with `any_of`, where the size is
    // discriminator-driven; it is rejected elsewhere rather than ignored.
    if let Some(path) = &realloc_grow_only_path {
        if !matches!(realloc_space_variant, Some(ReallocSpace::AnyOf(_))) {
            return Err(syn::Error::new_spanned(
                path,
                "Naclac Error: `realloc::grow_only` only applies with `realloc::any_of = \
                 [...]`; a plain `realloc = <expr>` already controls its direction.",
            ));
        }
    }
    // `realloc` needs an explicit `realloc::payer`: there is no safe default,
    // as a wrong guess would move lamports through an account the author never
    // named.
    match (realloc_space_variant, realloc_payer) {
        (Some(space), Some(payer)) => {
            realloc = Some(ReallocConfig {
                space,
                payer,
                zero: realloc_zero,
                grow_only: realloc_grow_only,
            });
        }
        (Some(_), None) => {
            return Err(syn::Error::new_spanned(
                &ident,
                "Naclac Error: `realloc = <expr>` / `realloc::any_of = [...]` requires \
                 `realloc::payer = <account>` on the same field.",
            ));
        }
        (None, _) => {}
    }

    // The creation CPI always signs as the executing program, so
    // `seeds::program` with a creation mode either can never satisfy the
    // signature or is redundant; it is rejected at compile time.
    if (is_init || is_init_if_needed || is_init_unchecked) && pda_program.is_some() {
        return Err(syn::Error::new_spanned(
            &ident,
            "Naclac Error: `init`/`init_if_needed`/`init_unchecked` can't be combined with \
             `seeds::program`, as creation always signs as this program. Remove \
             `seeds::program`.",
        ));
    }

    let mut final_address = address;
    if final_address.is_none() {
        if crate::type_classify::is_exactly(&ty, "Program") {
            let builtin = builtin_namespaces::PROGRAM_ADDRESSES
                .iter()
                .find(|(marker, _)| crate::type_classify::inner_is(&ty, marker));
            if let Some((_, address)) = builtin {
                final_address = Some(syn::parse_str(address)?);
            } else if crate::type_classify::inner_is(&ty, "System") {
                final_address = Some(syn::parse_str("naclac_lang::prelude::SYSTEM_PROGRAM_ID")?);
            }
        } else if crate::type_classify::is_exactly(&ty, "Rent") {
            final_address = Some(syn::parse_str("naclac_lang::prelude::RENT_SYSVAR_ID")?);
        }
    }

    if crate::type_classify::is_exactly(&ty, "AccountInfo") && !has_safety_doc_comment(field_attrs)
    {
        return Err(syn::Error::new_spanned(
            &ident,
            "Naclac Error: `AccountInfo` fields get no automatic validation. Add a `/// SAFETY: \
             ...` doc comment above the field explaining why that is safe.",
        ));
    }

    Ok(ParsedField {
        ident,
        ty,
        is_optional,
        is_signer,
        is_mut,
        is_alias,
        is_dup,
        is_executable,
        is_rent_exempt,
        pda_program,
        pda_seed,
        pda_bump,
        init_config,
        is_init_if_needed,
        is_init_unchecked,
        close_destination,
        relations,
        owner,
        address: final_address,
        realloc,
        extra_accounts,
        plugin_constraints,
        index: idx,
        has_safety_doc: has_safety_doc_comment(field_attrs),
        parse_error: None,
    })
}
