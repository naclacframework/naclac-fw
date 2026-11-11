//! Reads the raw `#[account(...)]` keys of one field into `RawAttrs`, before
//! any combination is validated.

use {
    super::{PdaBump, PdaSeeds, PluginConstraint, RelationConfig},
    crate::accounts::field::builtin_namespaces,
    syn::Ident,
};

/// Namespaces of the derive's own constraints. A path that starts with one of
/// these, or with a builtin namespace, but is not one of its keys is a
/// mistake, never a plugin constraint.
const CORE_NAMESPACES: [&str; 2] = ["realloc", "seeds"];

/// The `seeds = ..` value: a list `[..]`, or a call of a path ending in
/// `seeds`, like `PoolState::seeds(a, b)`. Anything else is an error.
fn parse_pda_seeds(expr: syn::Expr) -> syn::Result<PdaSeeds> {
    match expr {
        syn::Expr::Array(array) => Ok(PdaSeeds::List(array)),
        syn::Expr::Call(call) if is_seeds_function(&call.func) => Ok(PdaSeeds::Typed(call)),
        other => Err(syn::Error::new_spanned(
            other,
            "Naclac Error: `seeds` takes a list like `seeds = [SEED, owner.address().as_ref()]` \
             or a call like `seeds = Vault::seeds(owner)` of an `#[account]` with `#[seeds(..)]`.",
        )),
    }
}

/// True for a path of at least two segments whose last is `seeds`.
fn is_seeds_function(func: &syn::Expr) -> bool {
    let syn::Expr::Path(path) = func else {
        return false;
    };
    path.path.segments.len() >= 2
        && path
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "seeds")
}

fn is_builtin_namespace(segment: &syn::PathSegment) -> bool {
    builtin_namespaces::namespace(&segment.ident.to_string()).is_some()
}

/// Every key a field's `#[account(...)]` attributes can set, exactly as
/// written.
pub(super) struct RawAttrs {
    pub(super) is_signer: bool,
    pub(super) is_mut: bool,
    pub(super) is_alias: bool,
    pub(super) is_dup: bool,
    pub(super) is_executable: bool,
    pub(super) is_rent_exempt: bool,
    pub(super) pda_program: Option<syn::Expr>,
    pub(super) pda_seed: Option<super::PdaSeeds>,
    pub(super) pda_bump: Option<PdaBump>,
    pub(super) is_init: bool,
    pub(super) is_init_if_needed: bool,
    pub(super) is_init_unchecked: bool,
    pub(super) close_destination: Option<Ident>,
    pub(super) init_payer: Option<syn::Expr>,
    pub(super) init_space: Option<syn::Expr>,
    pub(super) realloc_space: Option<syn::Expr>,
    pub(super) realloc_any_of: Option<Vec<syn::Type>>,
    pub(super) realloc_grow_only: bool,
    pub(super) realloc_grow_only_path: Option<syn::Path>,
    pub(super) realloc_payer: Option<Ident>,
    pub(super) realloc_zero: bool,
    pub(super) relations: Vec<RelationConfig>,
    pub(super) owner: Option<syn::Expr>,
    pub(super) address: Option<syn::Expr>,
    pub(super) extra_accounts: Option<syn::ExprArray>,
    pub(super) plugin_constraints: Vec<PluginConstraint>,
}

impl RawAttrs {
    pub(super) fn parse(field_attrs: &[syn::Attribute]) -> syn::Result<RawAttrs> {
        let mut is_signer = false;
        let mut is_mut = false;
        let mut is_alias = false;
        let mut is_dup = false;
        let mut is_executable = false;
        let mut is_rent_exempt = false;
        let mut pda_program = None;
        let mut pda_seed = None;
        let mut pda_bump = None;
        let mut is_init = false;
        let mut is_init_if_needed = false;
        let mut is_init_unchecked = false;
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
        let mut extra_accounts: Option<syn::ExprArray> = None;
        let mut plugin_constraints: Vec<PluginConstraint> = Vec::new();

        for attr in field_attrs {
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
                        if meta.path.is_ident("dup") {
                            is_dup = true;
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
                                "Naclac Error: bare `alias` is rejected; write `unsafe(alias)` \
                                 to opt out of duplicate-mutable-account protection.",
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
                        if meta.path.is_ident("init_unchecked") {
                            is_mut = true;
                            is_init_unchecked = true;
                            return Ok(());
                        }

                        let path = &meta.path;
                        let is_reserved = path.is_ident("signer")
                            || path.is_ident("mut")
                            || path.is_ident("init")
                            || path.is_ident("init_if_needed")
                            || path.is_ident("init_unchecked")
                            || path.is_ident("owner")
                            || path.is_ident("address")
                            || path.is_ident("seeds")
                            || path.is_ident("bump")
                            || path.is_ident("executable")
                            || path.is_ident("unsafe")
                            || path.is_ident("alias")
                            || path.is_ident("dup")
                            || path.is_ident("payer")
                            || path.is_ident("space")
                            || path.is_ident("close")
                            || path.is_ident("rent_exempt")
                            || path.is_ident("realloc")
                            || path.is_ident("extra_accounts")
                            || (path.segments.len() == 2 && is_builtin_namespace(&path.segments[0]))
                            || (path.segments.len() == 2 && path.segments[0].ident == "realloc")
                            || (path.segments.len() == 2 && path.segments[0].ident == "seeds" && path.segments[1].ident == "program");

                        if !is_reserved && path.segments.len() >= 2 {
                            let mut namespace = path.clone();
                            let Some(last) = namespace.segments.pop() else {
                                return Err(meta.error("expected a constraint name"));
                            };
                            let key = last.ident;
                            namespace.segments.pop_punct();
                            if CORE_NAMESPACES.iter().any(|n| namespace.segments[0].ident == n)
                                || is_builtin_namespace(&namespace.segments[0])
                            {
                                let written = path
                                    .segments
                                    .iter()
                                    .map(|segment| segment.ident.to_string())
                                    .collect::<Vec<_>>()
                                    .join("::");
                                return Err(syn::Error::new_spanned(
                                    path,
                                    format!("Naclac Error: `{written}` is not a constraint."),
                                ));
                            }
                            let value = meta.value()?.parse::<syn::Expr>()?;
                            plugin_constraints.push(PluginConstraint {
                                namespace,
                                key,
                                value,
                            });
                            return Ok(());
                        }

                        if !is_reserved {
                            let field_ident = path.get_ident().cloned().ok_or_else(|| {
                                syn::Error::new_spanned(path, "Naclac Error: expected a field \
                                                               name for this relation \
                                                               constraint.")
                            })?;
                            if field_ident == "has_one" {
                                return Err(syn::Error::new_spanned(
                                    path,
                                    "Naclac Error: `has_one` is not supported; name the field \
                                     instead, e.g. `#[account(maker = maker)]`.",
                                ));
                            }
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
                            pda_seed = Some(parse_pda_seeds(meta.value()?.parse::<syn::Expr>()?)?);
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

                        if meta.path.segments.len() == 2 {
                            let name = meta.path.segments[0].ident.to_string();
                            if let Some(namespace) = builtin_namespaces::namespace(&name) {
                                let mut namespace_path = meta.path.clone();
                                let Some(last) = namespace_path.segments.pop() else {
                                    return Err(meta.error("expected a constraint name"));
                                };
                                namespace_path.segments.pop_punct();
                                let constraint = PluginConstraint {
                                    namespace: namespace_path,
                                    key: last.ident,
                                    value: meta.value()?.parse::<syn::Expr>()?,
                                };
                                builtin_namespaces::validate_key(
                                    namespace,
                                    &constraint,
                                    &plugin_constraints,
                                    &meta.path,
                                )?;
                                plugin_constraints.push(constraint);
                                return Ok(());
                            }
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

        Ok(RawAttrs {
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
        })
    }
}
