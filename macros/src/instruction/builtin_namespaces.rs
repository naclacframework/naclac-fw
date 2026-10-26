//! The constraint namespaces naclac ships, which programs use without
//! importing them (`#[account(token::mint = ..)]`). Everything the derive
//! knows about them is here; their checks and account creation live in the
//! crate the table points to. A namespace not listed here is resolved in the
//! program's own scope instead.

use {
    crate::instruction::parser::{ParsedField, PluginConstraint},
    proc_macro2::{Ident, TokenStream},
    quote::quote,
};

pub(crate) struct Namespace {
    pub name: &'static str,
    /// Where its `Args`, `Constraint` and `RULES` are.
    pub path: &'static str,
    pub keys: &'static [&'static str],
    /// A key that is not a constraint but a likely slip, with the hint added
    /// to the error.
    pub key_hint: Option<(&'static str, &'static str)>,
    pub check: Check,
    pub create: Option<Create>,
}

/// When and with what a namespace checks a field after it is loaded.
pub(crate) struct Check {
    pub when: CheckWhen,
    /// The check runs only when one of these keys is written.
    pub trigger: &'static [&'static str],
    /// Keys an existing account needs together: the first one missing is a
    /// compile error with its message, and no check is emitted.
    pub together: &'static [Required],
    /// Keys given to the check from another field when not written.
    pub filled: &'static [&'static Filled],
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) enum CheckWhen {
    /// On an existing account and on an `init_if_needed` one.
    UnlessCreated,
    /// Only on an account that is not `init` / `init_if_needed`.
    OnlyExisting,
}

/// How a namespace creates an `init` / `init_if_needed` field's account.
pub(crate) struct Create {
    pub by: CreatedBy,
    /// The namespace creates the account only when this key is written.
    pub trigger: &'static str,
    /// Checked in order; the first one missing is a compile error.
    pub required: &'static [Required],
    pub filled: &'static [&'static Filled],
    /// Keys that make no sense on a created account: a compile error at the
    /// key's value, `{field}` replaced by the field's name.
    pub forbidden: &'static [Required],
    /// `Some(message)`: `extra_accounts` is accepted, but only on an
    /// `init_if_needed` field; `message` otherwise.
    pub extra_accounts: Option<&'static str>,
    /// `Some(message)`: `init_unchecked` is rejected with `message`.
    pub rejects_init_unchecked: Option<&'static str>,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) enum CreatedBy {
    /// The derive creates the account through the system program
    /// (`AccountConstraintInit`).
    Derive,
    /// The program the namespace calls creates it (`AccountConstraintCreate`).
    Program,
}

pub(crate) struct Required {
    pub key: &'static str,
    pub message: &'static str,
}

/// A key filled from another field of the struct when it is not written: the
/// written value when it names a field, else the first field typed
/// `Program<T>` / `Interface<T>` with `T` in `from`.
pub(crate) struct Filled {
    pub key: &'static str,
    pub from: &'static [&'static str],
    /// The error when no field fills it.
    pub missing: &'static str,
}

/// One field names its token program at most once, in whichever namespace.
pub(crate) const TOKEN_PROGRAM_KEY: &str = "token_program";

const TOKEN_PROGRAM_TWICE: &str = "Naclac Error: this field names its token program more than \
     once; keep a single `token_program` key.";

const TOKEN_PROGRAM: Filled = Filled {
    key: TOKEN_PROGRAM_KEY,
    from: &["Token", "Token2022", "TokenInterface"],
    missing: "Naclac Error: token_program must be defined in the Accounts struct for account \
              initialization (or point at one explicitly via `token::token_program`, \
              `mint::token_program` or `associated_token::token_program`).",
};

const ASSOCIATED_TOKEN_PROGRAM: Filled = Filled {
    key: "associated_token_program",
    from: &["AssociatedToken"],
    missing: "Naclac Error: associated_token_program (Program<AssociatedToken>) must be \
              defined in the Accounts struct for associated token account initialization.",
};

const TOKEN_PROGRAM_HINT: Option<(&str, &str)> =
    Some(("program", "token_program` to name the token program."));

pub(crate) const NAMESPACES: &[Namespace] = &[
    Namespace {
        name: "token",
        path: "naclac_lang::token_constraints::token",
        keys: &["mint", "authority", "token_program"],
        key_hint: TOKEN_PROGRAM_HINT,
        check: Check {
            when: CheckWhen::UnlessCreated,
            trigger: &["mint", "authority"],
            together: &[],
            filled: &[],
        },
        create: Some(Create {
            by: CreatedBy::Derive,
            trigger: "mint",
            required: &[Required {
                key: "authority",
                message: "Naclac Error: `token::authority` must be specified alongside \
                          `token::mint`.",
            }],
            filled: &[&TOKEN_PROGRAM],
            forbidden: &[],
            extra_accounts: None,
            rejects_init_unchecked: None,
        }),
    },
    Namespace {
        name: "mint",
        path: "naclac_lang::token_constraints::mint",
        keys: &["decimals", "authority", "freeze_authority", "token_program"],
        key_hint: TOKEN_PROGRAM_HINT,
        check: Check {
            when: CheckWhen::UnlessCreated,
            trigger: &["decimals", "authority", "freeze_authority"],
            together: &[],
            filled: &[],
        },
        create: Some(Create {
            by: CreatedBy::Derive,
            trigger: "decimals",
            required: &[Required {
                key: "authority",
                message: "Naclac Error: `mint::authority` must be specified alongside \
                          `mint::decimals`.",
            }],
            filled: &[&TOKEN_PROGRAM],
            forbidden: &[],
            extra_accounts: None,
            rejects_init_unchecked: None,
        }),
    },
    Namespace {
        name: "associated_token",
        path: "naclac_lang::token_constraints::associated_token",
        keys: &["mint", "authority", "bump", "token_program"],
        key_hint: TOKEN_PROGRAM_HINT,
        check: Check {
            when: CheckWhen::OnlyExisting,
            trigger: &["mint", "authority"],
            together: &[
                Required {
                    key: "mint",
                    message: "Naclac Error: associated_token::mint must be specified alongside \
                              associated_token::authority. Example: \
                              #[account(associated_token::mint = mint, \
                              associated_token::authority = owner)]",
                },
                Required {
                    key: "authority",
                    message: "Naclac Error: associated_token::authority must be specified \
                              alongside associated_token::mint. Example: \
                              #[account(associated_token::mint = mint, \
                              associated_token::authority = owner)]",
                },
            ],
            filled: &[&TOKEN_PROGRAM],
        },
        create: Some(Create {
            by: CreatedBy::Program,
            trigger: "mint",
            required: &[Required {
                key: "authority",
                message: "Naclac Error: `associated_token::authority` must be specified \
                          alongside `associated_token::mint`.",
            }],
            filled: &[&TOKEN_PROGRAM, &ASSOCIATED_TOKEN_PROGRAM],
            forbidden: &[Required {
                key: "bump",
                message: "Naclac Error: `associated_token::bump` on field `{field}` is never \
                          read here -- `init`/`init_if_needed` associated-token accounts are \
                          verified by the real Associated Token Program's own CPI (it \
                          re-derives and checks the canonical address internally on every \
                          call), so naclac never reads this value for them. Remove the \
                          `associated_token::bump = ...` line; it only matters on an existing \
                          (non-init) associated_token field.",
            }],
            extra_accounts: Some(
                "Naclac Error: `extra_accounts` on an associated-token `init` field requires \
                 `init_if_needed` (not plain `init`) — only \
                 `create_idempotent_with_extra_accounts_signed` exists, since nothing in this \
                 codebase needs the strict, non-idempotent variant padded.",
            ),
            rejects_init_unchecked: Some(
                "Naclac Error: `init_unchecked` is not supported on an `associated_token::mint` \
                 field — ATA creation CPIs into the Associated Token Program, which handles its \
                 own account creation internally; there's no naclac-core `create_account_*` \
                 call here to redirect. Use `init`/`init_if_needed` instead.",
            ),
        }),
    },
];

/// When a field's account is created and two namespaces could create it,
/// the first one here does.
const CREATION_ORDER: [&str; 3] = ["mint", "token", "associated_token"];

/// The error for `extra_accounts` on an `init` field that no namespace which
/// accepts them creates.
pub(crate) const EXTRA_ACCOUNTS_ELSEWHERE: &str =
    "Naclac Error: `extra_accounts` is only supported on an `init_if_needed` \
     associated-token field (one with `associated_token::mint = ...`) — every other \
     `init` path (`mint::`, `token::`, plain PDA `init`) doesn't wire it in yet.";

/// The program id a `Program<T>` field must have, for the `T` naclac's
/// token crate defines.
pub(crate) const PROGRAM_ADDRESSES: &[(&str, &str)] = &[
    ("Token2022", "naclac_lang::prelude::TOKEN_2022_PROGRAM_ID"),
    ("Token", "naclac_lang::prelude::TOKEN_PROGRAM_ID"),
    (
        "AssociatedToken",
        "naclac_lang::prelude::ASSOCIATED_TOKEN_PROGRAM_ID",
    ),
];

pub(crate) fn namespace(name: &str) -> Option<&'static Namespace> {
    NAMESPACES.iter().find(|namespace| namespace.name == name)
}

/// The builtin namespace a constraint was written in, if any: a single
/// segment naming one of `NAMESPACES`.
pub(crate) fn of(constraint: &PluginConstraint) -> Option<&'static Namespace> {
    let segments = &constraint.namespace.segments;
    if segments.len() != 1 {
        return None;
    }
    namespace(&segments[0].ident.to_string())
}

/// The keys written in `namespace` on `field`, in the order written, except
/// the token program (which reaches a namespace only through `filled`).
pub(crate) fn written<'a>(
    field: &'a ParsedField,
    namespace: &Namespace,
) -> Vec<(&'a Ident, &'a syn::Expr)> {
    field
        .plugin_constraints
        .iter()
        .filter(|constraint| of(constraint).is_some_and(|n| n.name == namespace.name))
        .filter(|constraint| constraint.key != TOKEN_PROGRAM_KEY)
        .map(|constraint| (&constraint.key, &constraint.value))
        .collect()
}

fn has(written: &[(&Ident, &syn::Expr)], key: &str) -> bool {
    written.iter().any(|(written_key, _)| *written_key == key)
}

/// The token program written on `field` in any builtin namespace.
pub(crate) fn written_token_program(field: &ParsedField) -> Option<&syn::Expr> {
    written_token_program_in(&field.plugin_constraints)
}

fn written_token_program_in(constraints: &[PluginConstraint]) -> Option<&syn::Expr> {
    constraints
        .iter()
        .find(|constraint| of(constraint).is_some() && constraint.key == TOKEN_PROGRAM_KEY)
        .map(|constraint| &constraint.value)
}

/// Rejects a key `namespace` does not define, and a second token program
/// on the same field, before `constraint` is recorded.
pub(crate) fn validate_key(
    namespace: &Namespace,
    constraint: &PluginConstraint,
    recorded: &[PluginConstraint],
    path: &syn::Path,
) -> syn::Result<()> {
    let key = constraint.key.to_string();
    if !namespace.keys.contains(&key.as_str()) {
        return Err(unknown_key_error(path, namespace, &key));
    }
    if key == TOKEN_PROGRAM_KEY && written_token_program_in(recorded).is_some() {
        return Err(syn::Error::new_spanned(path, TOKEN_PROGRAM_TWICE));
    }
    Ok(())
}

/// Every unknown key is rejected, so a misspelled constraint can never
/// compile into a missing check.
fn unknown_key_error(path: &syn::Path, namespace: &Namespace, key: &str) -> syn::Error {
    let name = namespace.name;
    let list = namespace
        .keys
        .iter()
        .map(|k| format!("`{name}::{k}`"))
        .collect::<Vec<_>>()
        .join(", ");
    let hint = match namespace.key_hint {
        Some((slip, suggestion)) if slip == key => format!(" Write `{name}::{suggestion}"),
        _ => String::new(),
    };
    syn::Error::new_spanned(
        path,
        format!("Naclac Error: `{name}::{key}` is not a constraint. Valid keys: {list}.{hint}"),
    )
}

/// The error for `init_unchecked` on a field a namespace creates that does
/// not support it.
pub(crate) fn init_unchecked_error(constraints: &[PluginConstraint]) -> Option<&'static str> {
    NAMESPACES.iter().find_map(|namespace| {
        let create = namespace.create.as_ref()?;
        let triggered = constraints.iter().any(|constraint| {
            of(constraint).is_some_and(|n| n.name == namespace.name)
                && constraint.key == create.trigger
        });
        if triggered {
            create.rejects_init_unchecked
        } else {
            None
        }
    })
}

/// The namespace that creates `field`'s account, if any.
pub(crate) fn creator(field: &ParsedField) -> Option<(&'static Namespace, &'static Create)> {
    CREATION_ORDER.iter().find_map(|name| {
        let namespace = namespace(name)?;
        let create = namespace.create.as_ref()?;
        has(&written(field, namespace), create.trigger).then_some((namespace, create))
    })
}

/// The index of the field that fills `filled` for `field`.
pub(crate) fn resolve_filled(
    filled: &Filled,
    field: &ParsedField,
    all_fields: &[ParsedField],
) -> Result<usize, TokenStream> {
    if filled.key == TOKEN_PROGRAM_KEY {
        if let Some(syn::Expr::Path(expr_path)) = written_token_program(field) {
            if let Some(ident) = expr_path.path.get_ident() {
                if let Some(pos) = all_fields.iter().position(|f| &f.ident == ident) {
                    return Ok(pos);
                }
            }
        }
    }
    all_fields
        .iter()
        .position(|f| {
            crate::instruction::init_cpi::program_marker_ident(&f.ty)
                .is_some_and(|marker| filled.from.contains(&marker.as_str()))
        })
        .ok_or_else(|| syn::Error::new_spanned(&field.ident, filled.missing).to_compile_error())
}

/// The first of `required` not written, as a compile error at the field.
pub(crate) fn first_missing(
    required: &[Required],
    written: &[(&Ident, &syn::Expr)],
    field: &ParsedField,
) -> Option<TokenStream> {
    required
        .iter()
        .find(|required| !has(written, required.key))
        .map(|required| syn::Error::new_spanned(&field.ident, required.message).to_compile_error())
}

/// The first of `forbidden` written, as a compile error at its value.
pub(crate) fn first_forbidden(
    forbidden: &[Required],
    written: &[(&Ident, &syn::Expr)],
    field: &ParsedField,
) -> Option<TokenStream> {
    forbidden.iter().find_map(|forbidden| {
        let (_, value) = written.iter().find(|(key, _)| *key == forbidden.key)?;
        let message = forbidden
            .message
            .replace("{field}", &field.ident.to_string());
        Some(syn::Error::new_spanned(value, message).to_compile_error())
    })
}

/// Where errors in a field's generated constraint checks point when it has no
/// seeds: its `token::mint` value, if written.
pub(crate) fn check_span(field: &ParsedField) -> Option<proc_macro2::Span> {
    use syn::spanned::Spanned;
    let token = namespace("token")?;
    written(field, token)
        .into_iter()
        .find(|(key, _)| *key == "mint")
        .map(|(_, value)| value.span())
}

/// The namespace's path as tokens.
pub(crate) fn path_tokens(namespace: &Namespace) -> TokenStream {
    let path: syn::Path = syn::parse_str(namespace.path)
        .unwrap_or_else(|_| panic!("builtin namespace paths are valid Rust paths"));
    quote! { #path }
}

#[cfg(test)]
mod tests {
    use {
        super::{namespace, CREATION_ORDER, NAMESPACES},
        std::path::{Path, PathBuf},
    };

    /// Names that tie derive code to naclac's token crate. Outside this file
    /// they may appear only in test code.
    const TOKEN_NAMES: &[&str] = &[
        "\"Token\"",
        "\"Token2022\"",
        "\"TokenInterface\"",
        "\"AssociatedToken\"",
        "token_constraints",
        "TOKEN_PROGRAM_ID",
        "TOKEN_2022_PROGRAM_ID",
        "ASSOCIATED_TOKEN_PROGRAM_ID",
        "\"token\"",
        "\"mint\"",
        "\"associated_token\"",
        "\"token_program\"",
        "\"decimals\"",
    ];

    fn rust_files(dir: &Path, files: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                rust_files(&path, files);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                files.push(path);
            }
        }
    }

    #[test]
    fn token_names_appear_only_in_this_file() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        rust_files(&src, &mut files);

        let mut found = Vec::new();
        for file in files {
            if file.ends_with("builtin_namespaces.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&file).unwrap();
            let code = text.split("#[cfg(test)]").next().unwrap_or_default();
            for name in TOKEN_NAMES {
                if code.contains(name) {
                    found.push(format!("{}: {name}", file.display()));
                }
            }
        }
        assert!(found.is_empty(), "{found:#?}");
    }

    #[test]
    fn every_key_the_table_uses_is_a_key_of_its_namespace() {
        for ns in NAMESPACES {
            let used = ns
                .check
                .trigger
                .iter()
                .copied()
                .chain(ns.check.together.iter().map(|required| required.key));
            let created = ns.create.iter().flat_map(|create| {
                std::iter::once(create.trigger)
                    .chain(create.required.iter().map(|required| required.key))
                    .chain(create.forbidden.iter().map(|forbidden| forbidden.key))
            });
            for key in used.chain(created) {
                assert!(ns.keys.contains(&key), "`{}::{key}`", ns.name);
            }
        }
    }

    #[test]
    fn every_creating_namespace_is_in_the_creation_order() {
        for name in CREATION_ORDER {
            assert!(
                namespace(name).is_some_and(|ns| ns.create.is_some()),
                "{name}"
            );
        }
        let creating = NAMESPACES.iter().filter(|ns| ns.create.is_some()).count();
        assert_eq!(creating, CREATION_ORDER.len());
    }
}
