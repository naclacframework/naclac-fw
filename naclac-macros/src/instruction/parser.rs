//! # Instruction Attribute Parser
//!
//! Parses `#[account(...)]` attributes defined in `#[derive(Accounts)]` structs.
//! It extracts constraints (e.g., `init`, `mut`, `signer`, `has_one`) and builds a structured
//! `ParsedField` representation for the code generators.

use syn::{FieldsNamed, Ident, Type};

/// A field's `bump = ...` constraint: either bare `bump` (auto-derive/
/// auto-verify the canonical bump) or an explicit `bump = <expr>`.
pub enum PdaBump {
    Auto,
    Explicit(syn::Expr),
}

pub struct ParsedField {
    pub ident: Ident,
    pub ty: Type,
    pub is_signer: bool,
    pub is_mut: bool,
    pub is_alias: bool,
    pub is_executable: bool,
    pub is_rent_exempt: bool,
    pub pda_program: Option<syn::Expr>,
    pub pda_seed: Option<syn::ExprArray>,
    pub pda_bump: Option<PdaBump>,
    pub init_config: Option<InitConfig>,
    pub is_init_if_needed: bool,
    pub close_destination: Option<Ident>,
    pub relations: Vec<RelationConfig>,
    pub owner: Option<syn::Expr>,
    pub address: Option<syn::Expr>,
    pub realloc: Option<ReallocConfig>,
    pub token_mint: Option<syn::Expr>,
    pub token_authority: Option<syn::Expr>,
    pub token_program: Option<syn::Expr>,
    pub mint_decimals: Option<syn::Expr>,
    pub mint_authority: Option<syn::Expr>,
    pub mint_freeze_authority: Option<syn::Expr>,
    pub associated_token_mint: Option<syn::Expr>,
    pub associated_token_authority: Option<syn::Expr>,
    pub associated_token_bump: Option<syn::Expr>,
    /// `extra_accounts = [...]` on an `init`/`init_if_needed` associated-token
    /// field — other fields (by name) to pad, as harmless pass-through
    /// entries, onto the generated ATA-creation CPI's own account list.
    /// Needed whenever a raw (`sub_lamports`/`add_lamports`) lamport
    /// mutation touched the `payer` account (or its debit source) right
    /// before this `init`, since Solana's runtime only reconciles a raw
    /// lamport write into its tracked bookkeeping for accounts present in
    /// whatever CPI is entered next — see
    /// `naclac_token::associated_token::create_idempotent_with_extra_accounts_signed`.
    pub extra_accounts: Option<syn::ExprArray>,
    pub index: usize,
    /// True if this field was declared `Option<T>` — `ty` above is always
    /// `T` (the `Option<>` layer is unwrapped in `parse_one_field` before
    /// any other field is set), so every other piece of code that reads
    /// `ty` needs no `Option`-awareness of its own. Codegen (`accounts.rs`)
    /// gates this field's load behind a sentinel check instead of loading
    /// unconditionally — see `naclac-macros/docs/optional-accounts-plan.md`.
    pub is_optional: bool,
    /// Set when this field's own `#[account(...)]` attribute failed to
    /// parse. Every other field on the struct — and every constraint-
    /// independent generated item (the account loader call, `Bumps`/
    /// `LoadableAccounts`/`teardown`, other fields' checks) — is unaffected
    /// by one field's parse failure, so `parse_struct_fields` still returns
    /// a `ParsedField` for this field (all constraint data at safe/absent
    /// defaults) rather than aborting the whole struct. The codegen sites
    /// that would otherwise use this field's (unparseable) constraint data
    /// check this first and emit a single `compile_error!` in its place —
    /// see `accounts.rs`'s main per-field loop.
    pub parse_error: Option<syn::Error>,
}

#[derive(Clone)]
pub struct RelationConfig {
    pub field: Ident,
    pub target: syn::Expr,
    pub custom_error: Option<syn::Expr>,
}

pub struct InitConfig {
    pub payer: syn::Expr,
    pub space: Option<syn::Expr>,
}

/// Either a caller-supplied size expression (`realloc = <expr>`) or a list
/// of component types the field's discriminator is matched against
/// (`realloc::any_of = [Type1, Type2, ...]`), sized to whichever type's
/// `DISCRIMINATOR` the account's raw data actually starts with.
pub enum ReallocSpace {
    Expr(syn::Expr),
    AnyOf(Vec<syn::Type>),
}

pub struct ReallocConfig {
    pub space: ReallocSpace,
    pub payer: Ident,
    pub zero: bool,
    /// `realloc::grow_only = true`. Only meaningful (and only accepted) on
    /// `any_of` fields, where the target size is discriminator-driven
    /// rather than author-chosen — clamps the computed size to never go
    /// below the account's current length, so `any_of` can never shrink an
    /// account it doesn't own out from under its rightful holder. See
    /// `realloc::generate_any_of_space_expr`.
    pub grow_only: bool,
}

/// True if any doc-comment line on this field (once desugared from `///`/`#[doc = "..."]`
/// and trimmed of leading whitespace) starts with `SAFETY:`. Used to require an explicit,
/// human-written justification on every unchecked `AccountInfo` field — mirroring the
/// `// SAFETY:` convention this codebase already uses for `unsafe` blocks.
fn has_safety_doc_comment(attrs: &[syn::Attribute]) -> bool {
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

/// Parses the named fields of an `Accounts` struct.
///
/// Iterates over all fields and their attached `#[account(...)]` attributes to extract
/// and configure security checks, PDA seeds, initialization parameters, and program constraints.
/// A single field whose `#[account(...)]` fails to parse does not abort parsing for the
/// rest of the struct — see `parse_one_field`'s doc and `ParsedField::parse_error`.
pub fn parse_struct_fields(fields: &FieldsNamed) -> syn::Result<Vec<ParsedField>> {
    let mut parsed_fields = Vec::new();

    for (idx, field) in fields.named.iter().enumerate() {
        let parsed = match parse_one_field(idx, field) {
            Ok(parsed) => parsed,
            Err(err) => {
                // Type-level Option<> unwrapping is independent of whether
                // the field's #[account(...)] attribute content parsed —
                // still resolved here so the loader codegen in accounts.rs
                // (which always reads `ty`/`is_optional`, even for a
                // poisoned field) doesn't hit a second, unrelated "trait not
                // implemented" error on top of the real one.
                let raw_ty = field.ty.clone();
                let is_optional = crate::type_classify::is_option_account(&raw_ty);
                let ty = crate::type_classify::option_inner_type(&raw_ty)
                    .cloned()
                    .unwrap_or(raw_ty);
                ParsedField {
                    ident: field.ident.clone().unwrap(),
                    ty,
                    is_optional,
                    is_signer: false,
                    is_mut: false,
                    is_alias: false,
                    is_executable: false,
                    is_rent_exempt: false,
                    pda_program: None,
                    pda_seed: None,
                    pda_bump: None,
                    init_config: None,
                    is_init_if_needed: false,
                    close_destination: None,
                    relations: Vec::new(),
                    owner: None,
                    address: None,
                    realloc: None,
                    token_mint: None,
                    token_authority: None,
                    token_program: None,
                    mint_decimals: None,
                    mint_authority: None,
                    mint_freeze_authority: None,
                    associated_token_mint: None,
                    associated_token_authority: None,
                    associated_token_bump: None,
                    extra_accounts: None,
                    index: idx,
                    parse_error: Some(err),
                }
            }
        };
        parsed_fields.push(parsed);
    }

    // `close = dest` and `realloc::payer = payer` both move lamports into a
    // *second* field beyond the one carrying the constraint (the
    // destination/payer), and the annotated field itself is always written
    // too (zeroed+reassigned for close, resized for realloc). Force
    // `is_mut` on both ends here so the existing per-field `ConstraintMut`
    // check (`security.rs`) and duplicate-mutable-account protection
    // (`accounts.rs`'s `mut_mask_steps`) cover every account these
    // constraints write to, rather than relying on the user to also write
    // `mut` on the target and separately on the referenced field.
    let mut extra_mut_targets: Vec<Ident> = Vec::new();
    for field in &parsed_fields {
        if let Some(dest) = &field.close_destination {
            extra_mut_targets.push(dest.clone());
        }
        if let Some(realloc_config) = &field.realloc {
            extra_mut_targets.push(realloc_config.payer.clone());
        }
    }
    for field in &mut parsed_fields {
        if field.close_destination.is_some() || field.realloc.is_some() {
            field.is_mut = true;
        }
        if extra_mut_targets.contains(&field.ident) {
            field.is_mut = true;
        }
    }

    Ok(parsed_fields)
}

/// Parses a single field's `#[account(...)]` attribute. Called once per field
/// by `parse_struct_fields`, which substitutes a safe-defaults `ParsedField`
/// (carrying the error in `parse_error`) for any field this returns `Err`
/// for, rather than letting one field's mistake abort every other field's
/// already-successfully-parsed data.
fn parse_one_field(idx: usize, field: &syn::Field) -> syn::Result<ParsedField> {
    let ident = field.ident.clone().unwrap();
    let raw_ty = field.ty.clone();
    // `Option<>` is unwrapped once, up front — every check/constraint below
    // reads `ty` as the *inner* type, so `is_option_account` need not be
    // threaded through the rest of this function.
    let is_optional = crate::type_classify::is_option_account(&raw_ty);
    let ty = crate::type_classify::option_inner_type(&raw_ty)
        .cloned()
        .unwrap_or(raw_ty);

    let mut is_signer = false;
    let mut is_mut = false;
    let mut is_alias = false;
    let mut is_executable = false;
    let mut is_rent_exempt = false;
    let mut pda_program = None;
    let mut pda_seed = None;
    let mut pda_bump = None;
    let mut init_config = None;
    let mut is_init = false;
    let mut is_init_if_needed = false;
    let mut close_destination = None;
    let mut init_payer: Option<syn::Expr> = None;
    let mut init_space: Option<syn::Expr> = None;
    let mut realloc_space: Option<syn::Expr> = None;
    let mut realloc_any_of: Option<Vec<syn::Type>> = None;
    let mut realloc_grow_only = false;
    let mut realloc_grow_only_path: Option<syn::Path> = None;
    let mut realloc_payer: Option<Ident> = None;
    let mut realloc_zero = false;
    let mut relations = Vec::new();
    let mut owner = None;
    let mut address = None;
    let mut realloc = None;
    let mut token_mint: Option<syn::Expr> = None;
    let mut token_authority: Option<syn::Expr> = None;
    let mut token_program: Option<syn::Expr> = None;
    let mut mint_decimals: Option<syn::Expr> = None;
    let mut mint_authority: Option<syn::Expr> = None;
    let mut mint_freeze_authority: Option<syn::Expr> = None;
    let mut associated_token_mint: Option<syn::Expr> = None;
    let mut associated_token_authority: Option<syn::Expr> = None;
    let mut associated_token_bump: Option<syn::Expr> = None;
    let mut extra_accounts: Option<syn::ExprArray> = None;

    for attr in &field.attrs {
        if attr.path().is_ident("account") {
            attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("signer") {
                        is_signer = true;
                        return Ok(());
                    }
                    if meta.path.is_ident("mut") {
                        is_mut = true;
                        return Ok(());
                    }
                    if meta.path.is_ident("executable") {
                        is_executable = true;
                        return Ok(());
                    }
                    if meta.path.is_ident("rent_exempt") {
                        is_rent_exempt = true;
                        return Ok(());
                    }
                    if meta.path.is_ident("unsafe") {
                        let content;
                        syn::parenthesized!(content in meta.input);
                        let inner: syn::Ident = content.parse()?;
                        if inner == "alias" {
                            is_alias = true;
                        }
                        return Ok(());
                    }
                    if meta.path.is_ident("alias") {
                        return Err(syn::Error::new_spanned(
                            &meta.path,
                            "Naclac Error: bare `alias` is rejected for safety. \
                             Use `unsafe(alias)` to explicitly opt out of duplicate mutable account protection.",
                        ));
                    }
                    if meta.path.is_ident("init") {
                        is_mut = true;
                        is_init = true;
                        return Ok(());
                    }
                    if meta.path.is_ident("init_if_needed") {
                        is_mut = true;
                        is_init_if_needed = true;
                        return Ok(());
                    }

                    let path = &meta.path;
                    let is_reserved = path.is_ident("signer")
                        || path.is_ident("mut")
                        || path.is_ident("init")
                        || path.is_ident("init_if_needed")
                        || path.is_ident("owner")
                        || path.is_ident("address")
                        || path.is_ident("seeds")
                        || path.is_ident("bump")
                        || path.is_ident("executable")
                        || path.is_ident("unsafe")
                        || path.is_ident("alias")
                        || path.is_ident("payer")
                        || path.is_ident("space")
                        || path.is_ident("close")
                        || path.is_ident("rent_exempt")
                        || path.is_ident("realloc")
                        || path.is_ident("extra_accounts")
                        || (path.segments.len() == 2 && path.segments[0].ident == "token")
                        || (path.segments.len() == 2 && path.segments[0].ident == "mint")
                        || (path.segments.len() == 2 && path.segments[0].ident == "associated_token")
                        || (path.segments.len() == 2 && path.segments[0].ident == "realloc")
                        || (path.segments.len() == 2 && path.segments[0].ident == "seeds" && path.segments[1].ident == "program");

                    if !is_reserved {
                        let field_ident = path.get_ident().cloned().ok_or_else(|| {
                            syn::Error::new_spanned(path, "Expected relation field name")
                        })?;
                        let value = meta.value()?;

                        // Manually parse tokens until we hit a comma or the end of the attribute
                        let mut tokens = proc_macro2::TokenStream::new();
                        while !value.is_empty() && !value.peek(syn::Token![,]) {
                            tokens
                                .extend(core::iter::once(value.parse::<proc_macro2::TokenTree>()?));
                        }

                        // Split at a top-level `@` (Naclac's `field = target @ CustomError`
                        // syntax) by walking the real token tree, not by stringifying and
                        // slicing text — a stringified round-trip can corrupt re-parsing of
                        // complex expressions and can't distinguish a separator `@` from one
                        // appearing inside a legitimate sub-expression.
                        let mut target_tokens = proc_macro2::TokenStream::new();
                        let mut error_tokens: Option<proc_macro2::TokenStream> = None;
                        for tt in tokens {
                            if let Some(et) = error_tokens.as_mut() {
                                et.extend(core::iter::once(tt));
                                continue;
                            }
                            if let proc_macro2::TokenTree::Punct(p) = &tt {
                                if p.as_char() == '@' {
                                    error_tokens = Some(proc_macro2::TokenStream::new());
                                    continue;
                                }
                            }
                            target_tokens.extend(core::iter::once(tt));
                        }

                        let target: syn::Expr = syn::parse2(target_tokens)?;
                        let custom_error = match error_tokens {
                            Some(et) => Some(syn::parse2::<syn::Expr>(et)?),
                            None => None,
                        };
                        relations.push(RelationConfig {
                            field: field_ident,
                            target,
                            custom_error,
                        });
                        return Ok(());
                    }
                    if meta.path.is_ident("owner") {
                        owner = Some(meta.value()?.parse::<syn::Expr>()?);
                        return Ok(());
                    }
                    if meta.path.is_ident("address") {
                        address = Some(meta.value()?.parse::<syn::Expr>()?);
                        return Ok(());
                    }

                    if meta.path.is_ident("seeds") {
                        pda_seed = Some(meta.value()?.parse::<syn::ExprArray>()?);
                        return Ok(());
                    }

                    if meta.path.is_ident("extra_accounts") {
                        extra_accounts = Some(meta.value()?.parse::<syn::ExprArray>()?);
                        return Ok(());
                    }

                    if meta.path.segments.len() == 2
                        && meta.path.segments[0].ident == "seeds"
                        && meta.path.segments[1].ident == "program"
                    {
                        pda_program = Some(meta.value()?.parse::<syn::Expr>()?);
                        return Ok(());
                    }

                    if meta.path.segments.len() == 2 && meta.path.segments[0].ident == "token" {
                        let key = meta.path.segments[1].ident.to_string();
                        let value = meta.value()?;
                        let expr = value.parse::<syn::Expr>()?;
                        if key == "mint" {
                            token_mint = Some(expr);
                        } else if key == "authority" {
                            token_authority = Some(expr);
                        } else if key == "program" {
                            token_program = Some(expr);
                        }
                        return Ok(());
                    }

                    if meta.path.segments.len() == 2 && meta.path.segments[0].ident == "mint" {
                        let key = meta.path.segments[1].ident.to_string();
                        let value = meta.value()?;
                        let expr = value.parse::<syn::Expr>()?;
                        if key == "decimals" {
                            mint_decimals = Some(expr);
                        } else if key == "authority" {
                            mint_authority = Some(expr);
                        } else if key == "freeze_authority" {
                            mint_freeze_authority = Some(expr);
                        }
                        return Ok(());
                    }

                    if meta.path.segments.len() == 2 && meta.path.segments[0].ident == "associated_token" {
                        let key = meta.path.segments[1].ident.to_string();
                        let value = meta.value()?;
                        let expr = value.parse::<syn::Expr>()?;
                        if key == "mint" {
                            associated_token_mint = Some(expr);
                        } else if key == "authority" {
                            associated_token_authority = Some(expr);
                        } else if key == "bump" {
                            associated_token_bump = Some(expr);
                        }
                        return Ok(());
                    }

                    if meta.path.is_ident("bump") {
                        if let Ok(value) = meta.value() {
                            pda_bump = Some(PdaBump::Explicit(value.parse::<syn::Expr>()?));
                        } else {
                            pda_bump = Some(PdaBump::Auto);
                        }
                        return Ok(());
                    }

                    if meta.path.is_ident("payer") {
                        init_payer = Some(meta.value()?.parse::<syn::Expr>()?);
                        return Ok(());
                    }
                    if meta.path.is_ident("space") {
                        init_space = Some(meta.value()?.parse::<syn::Expr>()?);
                        return Ok(());
                    }
                    if meta.path.is_ident("close") {
                        close_destination = Some(meta.value()?.parse::<Ident>()?);
                        return Ok(());
                    }
                    if meta.path.is_ident("realloc") {
                        realloc_space = Some(meta.value()?.parse::<syn::Expr>()?);
                        return Ok(());
                    }
                    if meta.path.segments.len() == 2 && meta.path.segments[0].ident == "realloc" {
                        let key = meta.path.segments[1].ident.to_string();
                        if key == "payer" {
                            realloc_payer = Some(meta.value()?.parse::<Ident>()?);
                        } else if key == "zero" {
                            let expr = meta.value()?.parse::<syn::Expr>()?;
                            realloc_zero = matches!(
                                &expr,
                                syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Bool(b), .. }) if b.value
                            );
                        } else if key == "grow_only" {
                            let expr = meta.value()?.parse::<syn::Expr>()?;
                            realloc_grow_only = matches!(
                                &expr,
                                syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Bool(b), .. }) if b.value
                            );
                            realloc_grow_only_path = Some(meta.path.clone());
                        } else if key == "any_of" {
                            let array = meta.value()?.parse::<syn::ExprArray>()?;
                            let mut types = Vec::with_capacity(array.elems.len());
                            for elem in array.elems {
                                match elem {
                                    syn::Expr::Path(expr_path) => {
                                        let ty: syn::Type = syn::parse2(
                                            quote::quote! { #expr_path },
                                        )?;
                                        types.push(ty);
                                    }
                                    other => {
                                        return Err(syn::Error::new_spanned(
                                            other,
                                            "Naclac Error: `realloc::any_of = [...]` entries must be bare type paths (e.g. `Global`).",
                                        ));
                                    }
                                }
                            }
                            realloc_any_of = Some(types);
                        }
                        return Ok(());
                    }

                    Ok(())
                })?;
        }
    }

    // `init`/`init_if_needed` and `payer =` must appear together or not
    // at all — previously independent, which let `payer = wallet` alone
    // (no `init` keyword) silently run a full account-creation CPI with
    // `is_mut` left false (excluding the field from duplicate-mutable-
    // account protection even though it's being written via CPI this
    // call), and let `init` alone (no `payer =`) silently generate zero
    // creation logic at all (`generate_init_cpi` returns `quote!{}`
    // when `init_config` is `None`) with no compile-time signal that
    // `init` did nothing. See
    // `naclac-macros/docs/derive-accounts-gaps-audit.md`'s gap #2.
    match (is_init || is_init_if_needed, init_payer) {
        (true, Some(payer)) => {
            init_config = Some(InitConfig {
                payer,
                space: init_space,
            });
        }
        (true, None) => {
            return Err(syn::Error::new_spanned(
                &ident,
                "Naclac Error: `init`/`init_if_needed` requires `payer = <account>` on the \
                     same field — without it, no account-creation CPI is generated at all, and \
                     this field would silently behave as an ordinary (non-created) account.",
            ));
        }
        (false, Some(_)) => {
            return Err(syn::Error::new_spanned(
                &ident,
                "Naclac Error: `payer = ...` has no effect without `init` or \
                     `init_if_needed` on the same field. Add `init` (or `init_if_needed`) \
                     explicitly.",
            ));
        }
        (false, None) => {}
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
    // `grow_only` only means something when the target size is
    // discriminator-driven (`any_of`) rather than author-chosen
    // (`realloc = <expr>`, where the author already fully controls
    // direction by hand) — reject it elsewhere instead of silently
    // ignoring a flag that looks like it should do something.
    if let Some(path) = &realloc_grow_only_path {
        if !matches!(realloc_space_variant, Some(ReallocSpace::AnyOf(_))) {
            return Err(syn::Error::new_spanned(
                path,
                "Naclac Error: `realloc::grow_only` is only meaningful alongside \
                     `realloc::any_of = [...]` — a plain `realloc = <expr>` already fully \
                     controls its own size and direction.",
            ));
        }
    }
    // `realloc = <expr>` / `realloc::any_of = [...]` without `realloc::payer`
    // used to silently construct no `ReallocConfig` at all — the space
    // expression, `grow_only`, everything, just dropped with no compile
    // error and no runtime effect. Same class of gap `init` had before its
    // own `payer`-required guard (`derive-accounts-gaps-audit.md` gap #2):
    // there's no safe default payer to fall back to, since guessing wrong
    // means moving real lamports through an account the author never named.
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
                     `realloc::payer = <account>` on the same field — without it, no resize \
                     logic is generated at all, and this field would silently behave as an \
                     ordinary (non-reallocating) account.",
            ));
        }
        (None, _) => {}
    }

    // `init`/`init_if_needed` combined with `seeds::program = X` has no
    // valid use: the account-creation CPI's `invoke_signed` always signs
    // under the currently executing program (that's inherent to Solana's
    // CPI signing model — there is no way to sign as a different program
    // X), so if X differs from the current program this can never
    // satisfy the signature check and fails at runtime every time; if X
    // happens to equal the current program, `seeds::program` is entirely
    // redundant (the default already uses the current program). Either
    // way, writing it alongside `init`/`init_if_needed` is always a
    // mistake, so it's rejected at compile time here rather than
    // failing opaquely at runtime (or silently succeeding-by-redundancy
    // and misleading a reader into thinking it does something).
    if (is_init || is_init_if_needed) && pda_program.is_some() {
        return Err(syn::Error::new_spanned(
            &ident,
            "Naclac Error: `init`/`init_if_needed` cannot be combined with `seeds::program = \
                 ...` on the same field — the account-creation CPI always signs under the \
                 currently executing program, so a `seeds::program` pointing anywhere else can \
                 never satisfy the signature check, and pointing at the current program is \
                 redundant. Remove `seeds::program` here.",
        ));
    }

    let mut final_address = address;
    if final_address.is_none() {
        if crate::type_classify::is_exactly(&ty, "Program") {
            if crate::type_classify::inner_is(&ty, "Token2022") {
                final_address =
                    Some(syn::parse_str("naclac_lang::prelude::TOKEN_2022_PROGRAM_ID").unwrap());
            } else if crate::type_classify::inner_is(&ty, "Token") {
                final_address =
                    Some(syn::parse_str("naclac_lang::prelude::TOKEN_PROGRAM_ID").unwrap());
            } else if crate::type_classify::inner_is(&ty, "System") {
                final_address =
                    Some(syn::parse_str("naclac_lang::prelude::SYSTEM_PROGRAM_ID").unwrap());
            } else if crate::type_classify::inner_is(&ty, "AssociatedToken") {
                final_address = Some(
                    syn::parse_str("naclac_lang::prelude::ASSOCIATED_TOKEN_PROGRAM_ID").unwrap(),
                );
            }
        } else if crate::type_classify::is_exactly(&ty, "Rent") {
            final_address = Some(syn::parse_str("naclac_lang::prelude::RENT_SYSVAR_ID").unwrap());
        }
    }

    if crate::type_classify::is_exactly(&ty, "AccountInfo") && !has_safety_doc_comment(&field.attrs)
    {
        return Err(syn::Error::new_spanned(
            &ident,
            "Naclac Error: `AccountInfo` fields receive no automatic validation. \
                 Add a `/// SAFETY: ...` doc comment directly above this field explaining \
                 why skipping validation is safe here.",
        ));
    }

    Ok(ParsedField {
        ident,
        ty,
        is_optional,
        is_signer,
        is_mut,
        is_alias,
        is_executable,
        is_rent_exempt,
        pda_program,
        pda_seed,
        pda_bump,
        init_config,
        is_init_if_needed,
        close_destination,
        relations,
        owner,
        address: final_address,
        realloc,
        token_mint,
        token_authority,
        token_program,
        mint_decimals,
        mint_authority,
        mint_freeze_authority,
        associated_token_mint,
        associated_token_authority,
        associated_token_bump,
        extra_accounts,
        index: idx,
        parse_error: None,
    })
}
