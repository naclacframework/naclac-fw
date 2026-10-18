//! The IDL-visible declarations of a crate, gathered from its module tree,
//! and the rules they must satisfy for an IDL to describe the program
//! unambiguously: one declaration per IDL name in each namespace, one error
//! per error code, and existence that depends only on `feature` cfgs. Both
//! IDL generators and `#[program]` enforce these rules from this one source.

use crate::modules::CrateModule;

/// The on-chain code of an `#[error_code]` enum's first possible variant
/// (discriminant 0) when the enum sets no `offset`. Must equal
/// `naclac_core::error::USER_ERROR_BASE`; `#[error_code]` asserts it at
/// compile time.
pub const DEFAULT_ERROR_OFFSET: u32 = 6000;

/// Which IDL names an item competes with. Components, defined types and
/// events all become named types in generated clients, so they share one.
/// Accounts structs are found by name from an instruction's `Context<Name>`,
/// so they form a namespace of their own.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Namespace {
    Type,
    Error,
    Constant,
    Accounts,
}

impl Namespace {
    fn label(self) -> &'static str {
        match self {
            Namespace::Type => "type",
            Namespace::Error => "error",
            Namespace::Constant => "constant",
            Namespace::Accounts => "accounts struct",
        }
    }
}

/// One declaration the IDL depends on.
pub struct Declaration {
    /// `None` for a declaration the IDL reads but does not name (an accounts
    /// struct).
    pub namespace: Option<Namespace>,
    pub kind: &'static str,
    pub name: String,
    /// Its path from the crate root, e.g. `crate::state::Vault` or
    /// `crate::errors::VaultError::NotAdmin`.
    pub location: String,
    /// The `#[cfg(..)]` attributes it exists under: its modules' then its own.
    pub cfgs: Vec<syn::Attribute>,
    /// The on-chain error code, for an error.
    pub code: Option<u32>,
}

/// A broken rule. It applies only when every `cfgs` predicate holds, so two
/// declarations that never exist in the same build never conflict.
pub struct Violation {
    pub message: String,
    pub cfgs: Vec<syn::Attribute>,
}

fn has_attr(attrs: &[syn::Attribute], name: &str) -> Option<syn::Attribute> {
    attrs.iter().find(|a| a.path().is_ident(name)).cloned()
}

/// The `offset` of an `#[error_code]` / `#[error_code(offset = N)]`
/// attribute: the code of discriminant 0. Any other argument is an error.
pub fn error_code_offset(attr: &syn::Attribute) -> syn::Result<u32> {
    let syn::Meta::List(list) = &attr.meta else {
        return Ok(DEFAULT_ERROR_OFFSET);
    };
    error_code_offset_tokens(list.tokens.clone())
}

/// [`error_code_offset`] for the argument tokens of `#[error_code(..)]`.
pub fn error_code_offset_tokens(tokens: proc_macro2::TokenStream) -> syn::Result<u32> {
    if tokens.is_empty() {
        return Ok(DEFAULT_ERROR_OFFSET);
    }
    let nv: syn::MetaNameValue = syn::parse2(tokens)?;
    if !nv.path.is_ident("offset") {
        return Err(syn::Error::new_spanned(
            &nv.path,
            "Naclac Error: `#[error_code]` accepts only `offset = N`.",
        ));
    }
    let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Int(int),
        ..
    }) = &nv.value
    else {
        return Err(syn::Error::new_spanned(
            &nv.value,
            "Naclac Error: `offset` must be an integer literal.",
        ));
    };
    int.base10_parse::<u32>()
}

fn cfgs_of(module: &CrateModule, attrs: &[syn::Attribute]) -> Vec<syn::Attribute> {
    let mut cfgs = module.cfgs.clone();
    cfgs.extend(crate::modules::cfg_attrs(attrs));
    cfgs
}

/// Every declaration of `modules` that an IDL describes or reads.
pub fn declarations(modules: &[CrateModule]) -> syn::Result<Vec<Declaration>> {
    let mut out = Vec::new();
    for module in modules {
        for item in &module.items {
            let attrs = crate::modules::item_attrs(item);
            let location = |name: &str| format!("{}::{name}", module.path);
            match item {
                syn::Item::Struct(s) => {
                    let name = s.ident.to_string();
                    let entry = if has_attr(attrs, "component").is_some() {
                        Some((Some(Namespace::Type), "component"))
                    } else if has_attr(attrs, "event").is_some() {
                        Some((Some(Namespace::Type), "event"))
                    } else if has_attr(attrs, "defined_type").is_some() {
                        Some((Some(Namespace::Type), "defined type"))
                    } else if is_accounts_struct(attrs) {
                        Some((Some(Namespace::Accounts), "accounts struct"))
                    } else {
                        None
                    };
                    if let Some((namespace, kind)) = entry {
                        out.push(Declaration {
                            namespace,
                            kind,
                            location: location(&name),
                            name,
                            cfgs: cfgs_of(module, attrs),
                            code: None,
                        });
                    }
                }
                syn::Item::Enum(e) => {
                    let name = e.ident.to_string();
                    if let Some(attr) = has_attr(attrs, "error_code") {
                        let offset = error_code_offset(&attr)?;
                        let discriminants =
                            crate::discriminator::variant_discriminants(e.variants.iter());
                        for (variant, discriminant) in e.variants.iter().zip(discriminants) {
                            let variant_name = variant.ident.to_string();
                            out.push(Declaration {
                                namespace: Some(Namespace::Error),
                                kind: "error",
                                location: format!("{}::{name}::{variant_name}", module.path),
                                name: variant_name,
                                cfgs: cfgs_of(module, attrs),
                                code: u32::try_from(i128::from(offset) + discriminant).ok(),
                            });
                        }
                    } else if has_attr(attrs, "defined_type").is_some() {
                        out.push(Declaration {
                            namespace: Some(Namespace::Type),
                            kind: "defined type",
                            location: location(&name),
                            name,
                            cfgs: cfgs_of(module, attrs),
                            code: None,
                        });
                    }
                }
                syn::Item::Const(c) if has_attr(attrs, "constant").is_some() => {
                    let name = c.ident.to_string();
                    out.push(Declaration {
                        namespace: Some(Namespace::Constant),
                        kind: "constant",
                        location: location(&name),
                        name,
                        cfgs: cfgs_of(module, attrs),
                        code: None,
                    });
                }
                _ => {}
            }
        }
    }
    Ok(out)
}

fn is_accounts_struct(attrs: &[syn::Attribute]) -> bool {
    crate::parser::derives(attrs, "Accounts")
}

fn joined_cfgs(a: &Declaration, b: &Declaration) -> Vec<syn::Attribute> {
    let mut cfgs = a.cfgs.clone();
    cfgs.extend(b.cfgs.iter().cloned());
    cfgs
}

/// Every rule `declarations` breaks.
pub fn violations(declarations: &[Declaration]) -> Vec<Violation> {
    let mut out = Vec::new();

    for decl in declarations {
        if !crate::modules::cfg_is_decidable(&decl.cfgs) {
            out.push(Violation {
                message: format!(
                    "Naclac Error: {} `{}` exists only under a `#[cfg]` other than `feature = \"..\"` \
                     (or `test`). An IDL describes one program for every target, so IDL items may \
                     only be gated by features.",
                    decl.kind, decl.location
                ),
                cfgs: Vec::new(),
            });
        }
    }

    for (i, a) in declarations.iter().enumerate() {
        for b in &declarations[i + 1..] {
            if let (Some(ns_a), Some(ns_b)) = (a.namespace, b.namespace) {
                if ns_a == ns_b && a.name == b.name {
                    out.push(Violation {
                        message: format!(
                            "Naclac Error: two IDL {} items are both named `{}`: {} `{}` and {} `{}`. \
                             The IDL identifies them by name only; rename one.",
                            ns_a.label(),
                            a.name,
                            a.kind,
                            a.location,
                            b.kind,
                            b.location
                        ),
                        cfgs: joined_cfgs(a, b),
                    });
                }
            }
            if let (Some(code_a), Some(code_b)) = (a.code, b.code) {
                if code_a == code_b {
                    out.push(Violation {
                        message: format!(
                            "Naclac Error: error code {code_a} is used by both `{}` and `{}`. Give \
                             one `#[error_code]` enum its own range with `#[error_code(offset = N)]`.",
                            a.location, b.location
                        ),
                        cfgs: joined_cfgs(a, b),
                    });
                }
            }
        }
    }

    out
}
