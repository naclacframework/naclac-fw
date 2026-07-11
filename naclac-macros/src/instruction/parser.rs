//! # Instruction Attribute Parser
//!
//! Parses `#[account(...)]` attributes defined in `#[derive(Accounts)]` structs.
//! It extracts constraints (e.g., `init`, `mut`, `signer`, `has_one`) and builds a structured
//! `ParsedField` representation for the code generators.

use quote::quote;
use syn::{FieldsNamed, Ident, Type};

#[allow(dead_code)]
pub struct ParsedField {
    pub ident: Ident,
    pub ty: Type,
    pub type_str: String,
    pub is_signer: bool,
    pub is_mut: bool,
    pub is_alias: bool,
    pub is_executable: bool,
    pub pda_program: Option<String>,
    pub pda_seed: Option<syn::ExprArray>,
    pub pda_bump: Option<String>,
    pub init_config: Option<InitConfig>,
    pub is_init_if_needed: bool,
    pub close_destination: Option<String>,
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
    pub index: usize,
}

#[derive(Clone)]
pub struct RelationConfig {
    pub field: Ident,
    pub target: syn::Expr,
    pub custom_error: Option<syn::Expr>,
}

pub struct InitConfig {
    pub payer: String,
    pub space: Option<proc_macro2::TokenStream>,
}

pub struct ReallocConfig {
    pub space: proc_macro2::TokenStream,
    pub payer: String,
    pub zero: bool,
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
pub fn parse_struct_fields(fields: &FieldsNamed) -> syn::Result<Vec<ParsedField>> {
    let mut parsed_fields = Vec::new();

    for (idx, field) in fields.named.iter().enumerate() {
        let ident = field.ident.clone().unwrap();
        let ty = field.ty.clone();
        let type_str = quote::quote!(#ty).to_string();

        let mut is_signer = false;
        let mut is_mut = false;
        let mut is_alias = false;
        let mut is_executable = false;
        let mut pda_program = None;
        let mut pda_seed = None;
        let mut pda_bump = None;
        let mut init_config = None;
        let mut is_init_if_needed = false;
        let mut close_destination = None;
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
                        || (path.segments.len() == 2 && path.segments[0].ident == "token")
                        || (path.segments.len() == 2 && path.segments[0].ident == "mint")
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

                        let token_str = tokens.to_string();

                        if let Some(at_idx) = token_str.find('@') {
                            let target_str = token_str[..at_idx].trim().to_string();
                            let target: syn::Expr = syn::parse_str(&target_str)?;
                            let error_str = token_str[at_idx + 1..].trim();
                            let custom_error: syn::Expr = syn::parse_str(error_str)?;

                            relations.push(RelationConfig {
                                field: field_ident,
                                target,
                                custom_error: Some(custom_error),
                            });
                        } else {
                            let target: syn::Expr = syn::parse_str(&token_str)?;
                            relations.push(RelationConfig {
                                field: field_ident,
                                target,
                                custom_error: None,
                            });
                        }
                        return Ok(());
                    }
                    if meta.path.is_ident("owner") {
                        let value = meta.value()?;
                        if let Ok(expr) = value.parse::<syn::Expr>() {
                            owner = Some(expr);
                        }
                        return Ok(());
                    }
                    if meta.path.is_ident("address") {
                        let value = meta.value()?;
                        if let Ok(expr) = value.parse::<syn::Expr>() {
                            address = Some(expr);
                        }
                        return Ok(());
                    }

                    if meta.path.is_ident("seeds") {
                        let value = meta.value()?;
                        if let Ok(expr_array) = value.parse::<syn::ExprArray>() {
                            pda_seed = Some(expr_array);
                        }
                        return Ok(());
                    }

                    if meta.path.segments.len() == 2
                        && meta.path.segments[0].ident == "seeds"
                        && meta.path.segments[1].ident == "program"
                    {
                        let value = meta.value()?;
                        if let Ok(expr) = value.parse::<syn::Expr>() {
                            pda_program = Some(quote::quote!(#expr).to_string());
                        }
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

                    if meta.path.is_ident("bump") {
                        if let Ok(value) = meta.value() {
                            if let Ok(expr) = value.parse::<syn::Expr>() {
                                pda_bump = Some(quote::quote!(#expr).to_string());
                            }
                        } else {
                            pda_bump = Some("__naclac_auto_bump".to_string());
                        }
                        return Ok(());
                    }

                    if meta.path.is_ident("payer")
                        || meta.path.is_ident("space")
                        || meta.path.is_ident("close")
                        || meta.path.is_ident("rent_exempt")
                    {
                        let _ = meta.value()?.parse::<syn::Expr>()?;
                        return Ok(());
                    }

                    Ok(())
                })?;

                let attr_tokens = quote!(#attr);

                if let Ok(Some(parsed_init)) = fully_parse_init(attr_tokens.clone()) {
                    init_config = Some(parsed_init);
                }
                if let Ok(Some(parsed_realloc)) = fully_parse_realloc(attr_tokens.clone()) {
                    realloc = Some(parsed_realloc);
                }
                if let Ok(Some(parsed_close)) = fully_parse_close(attr_tokens.clone()) {
                    close_destination = Some(parsed_close);
                }
            }
        }

        let mut final_address = address;
        if final_address.is_none() {
            if type_str.contains("Program") {
                if type_str.contains("Token2022") {
                    final_address = Some(
                        syn::parse_str("naclac_lang::prelude::TOKEN_2022_PROGRAM_ID").unwrap(),
                    );
                } else if type_str.contains("Token") {
                    final_address =
                        Some(syn::parse_str("naclac_lang::prelude::TOKEN_PROGRAM_ID").unwrap());
                } else if type_str.contains("System") {
                    final_address =
                        Some(syn::parse_str("naclac_lang::prelude::SYSTEM_PROGRAM_ID").unwrap());
                } else if type_str.contains("AssociatedToken") {
                    final_address = Some(
                        syn::parse_str("naclac_lang::prelude::ASSOCIATED_TOKEN_PROGRAM_ID")
                            .unwrap(),
                    );
                }
            } else if type_str.contains("Rent") {
                final_address = Some(syn::parse_str("naclac_lang::prelude::RENT_ID").unwrap());
            }
        }

        if type_str == "AccountInfo" && !has_safety_doc_comment(&field.attrs) {
            return Err(syn::Error::new_spanned(
                &ident,
                "Naclac Error: `AccountInfo` fields receive no automatic validation. \
                 Add a `/// SAFETY: ...` doc comment directly above this field explaining \
                 why skipping validation is safe here.",
            ));
        }

        parsed_fields.push(ParsedField {
            ident,
            ty,
            type_str,
            is_signer,
            is_mut,
            is_alias,
            is_executable,
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
            index: idx,
        });
    }

    Ok(parsed_fields)
}

fn fully_parse_init(tokens: proc_macro2::TokenStream) -> syn::Result<Option<InitConfig>> {
    let token_str = tokens.to_string();
    if !token_str.contains("init") && !token_str.contains("init_if_needed") {
        return Ok(None);
    }

    let mut payer = String::new();
    let mut space = None;

    if let Some(payer_idx) = token_str.find("payer = ") {
        let remainder = &token_str[payer_idx + 8..];
        let end_idx = remainder
            .find(',')
            .unwrap_or(remainder.find(')').unwrap_or(remainder.len()));
        payer = remainder[..end_idx].trim().replace("\"", "");
    }

    if let Some(space_idx) = token_str.find("space = ") {
        let remainder = &token_str[space_idx + 8..];
        let end_idx = remainder
            .find(',')
            .unwrap_or(remainder.find(')').unwrap_or(remainder.len()));
        let space_str = remainder[..end_idx].trim();
        if let Ok(expr) = syn::parse_str::<syn::Expr>(space_str) {
            space = Some(quote::quote!(#expr));
        }
    }

    if payer.is_empty() {
        return Ok(None);
    }
    Ok(Some(InitConfig { payer, space }))
}

fn fully_parse_realloc(tokens: proc_macro2::TokenStream) -> syn::Result<Option<ReallocConfig>> {
    let token_str = tokens.to_string();
    if !token_str.contains("realloc") {
        return Ok(None);
    }

    let mut space = None;
    let mut payer = String::new();
    let mut zero = false;

    if let Some(space_idx) = token_str.find("space = ") {
        let remainder = &token_str[space_idx + 8..];
        let end_idx = remainder
            .find(',')
            .unwrap_or(remainder.find(')').unwrap_or(remainder.len()));
        let space_str = remainder[..end_idx].trim();
        if let Ok(expr) = syn::parse_str::<syn::Expr>(space_str) {
            space = Some(quote::quote!(#expr));
        }
    }

    if let Some(payer_idx) = token_str.find("payer = ") {
        let remainder = &token_str[payer_idx + 8..];
        let end_idx = remainder
            .find(',')
            .unwrap_or(remainder.find(')').unwrap_or(remainder.len()));
        payer = remainder[..end_idx].trim().replace("\"", "");
    }

    if token_str.contains("zero = true") || token_str.contains("zero=true") {
        zero = true;
    }

    if space.is_none() || payer.is_empty() {
        return Ok(None);
    }

    Ok(Some(ReallocConfig {
        space: space.unwrap(),
        payer,
        zero,
    }))
}

fn fully_parse_close(tokens: proc_macro2::TokenStream) -> syn::Result<Option<String>> {
    let token_str = tokens.to_string();
    if let Some(idx) = token_str.find("close = ") {
        let remainder = &token_str[idx + 8..];
        let end_idx = remainder
            .find(',')
            .unwrap_or(remainder.find(')').unwrap_or(remainder.len()));
        let dest = remainder[..end_idx].trim().replace("\"", "");
        return Ok(Some(dest));
    }
    Ok(None)
}
