//! Parses the `#[account(...)]` attribute of each field of an Accounts struct
//! into a `ParsedField` for the code generators. `attrs` reads the attribute
//! text, `validate` rejects invalid combinations and fills defaults.

mod attrs;
mod validate;

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
    /// `dup`: this read-only field may repeat an earlier account of the
    /// instruction, unless that account is in a `mut` slot not marked
    /// `unsafe(alias)`. Any other field that repeats an earlier account is
    /// rejected at runtime.
    pub is_dup: bool,
    pub is_executable: bool,
    pub is_rent_exempt: bool,
    pub pda_program: Option<syn::Expr>,
    pub pda_seed: Option<syn::ExprArray>,
    pub pda_bump: Option<PdaBump>,
    pub init_config: Option<InitConfig>,
    pub is_init_if_needed: bool,
    /// `init_unchecked`: creates the account like `init` but through naclac-core's
    /// unchecked `CreateAccount` CPI, with no prefund-awareness (it fails with
    /// `AccountAlreadyInUse` if the target already holds lamports) and, on
    /// pinocchio, no runtime `is_borrowed()` aliasing check. Mutually exclusive
    /// with `init`/`init_if_needed`, and rejected on `associated_token::mint`,
    /// `mint::decimals` and `token::mint` fields, whose creation does not go
    /// through those helpers.
    pub is_init_unchecked: bool,
    pub close_destination: Option<Ident>,
    pub relations: Vec<RelationConfig>,
    pub owner: Option<syn::Expr>,
    pub address: Option<syn::Expr>,
    pub realloc: Option<ReallocConfig>,
    /// `extra_accounts = [...]` on an `init`/`init_if_needed` associated-token
    /// field: other fields (by name) appended to the generated ATA-creation CPI's
    /// account list. Needed when a raw lamport mutation touched the `payer`
    /// right before the `init`, as the runtime only reconciles such a write for
    /// accounts present in the next CPI (see
    /// `naclac_token::associated_token::create_idempotent_with_extra_accounts_signed`).
    pub extra_accounts: Option<syn::ExprArray>,
    /// `namespace::key = value` constraints, builtin (`builtin_namespaces`)
    /// or provided by another crate (see `naclac_core::constraint`), in the
    /// order written.
    pub plugin_constraints: Vec<PluginConstraint>,
    pub index: usize,
    /// True if the field was declared `Option<T>`. `ty` is always the inner
    /// `T`, so code reading `ty` needs no `Option`-awareness; the loader gates
    /// this field behind a sentinel check instead.
    pub is_optional: bool,
    /// True if a doc comment line of the field starts with `SAFETY:`.
    pub has_safety_doc: bool,
    /// Set when this field's own `#[account(...)]` failed to parse. The field
    /// still gets a `ParsedField` (constraint data at safe defaults) so one
    /// field's mistake does not abort the whole struct; the codegen sites that
    /// would use its constraint data emit this error in its place.
    pub parse_error: Option<syn::Error>,
}

/// One `namespace::key = value` constraint. `namespace` is the path as
/// written (everything before the last segment); the crate it names must
/// export `Args`, `Constraint` and `RULES`.
#[derive(Clone)]
pub struct PluginConstraint {
    pub namespace: syn::Path,
    pub key: Ident,
    pub value: syn::Expr,
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
/// of `#[account]` types the field's discriminator is matched against
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
    /// `realloc::grow_only = true`, accepted only on `any_of` fields: clamps
    /// the computed size to never go below the account's current length, so
    /// `any_of` can never shrink an account. See `realloc::generate_any_of_space_expr`.
    pub grow_only: bool,
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
                // `ty` and `is_optional` come from the type alone, so the
                // loader code, which always reads them, still compiles for a
                // field whose attribute failed to parse.
                let is_optional = crate::type_classify::is_option_account(&field.ty);
                let ty = crate::type_classify::option_inner_type(&field.ty)
                    .cloned()
                    .unwrap_or_else(|| field.ty.clone());
                let Some(ident) = field.ident.clone() else {
                    return Err(syn::Error::new_spanned(
                        field,
                        "Naclac Error: Accounts struct fields must be named.",
                    ));
                };
                ParsedField {
                    ident,
                    ty,
                    is_optional,
                    is_signer: false,
                    is_mut: false,
                    is_alias: false,
                    is_dup: false,
                    is_executable: false,
                    is_rent_exempt: false,
                    pda_program: None,
                    pda_seed: None,
                    pda_bump: None,
                    init_config: None,
                    is_init_if_needed: false,
                    is_init_unchecked: false,
                    close_destination: None,
                    relations: Vec::new(),
                    owner: None,
                    address: None,
                    realloc: None,
                    extra_accounts: None,
                    plugin_constraints: Vec::new(),
                    index: idx,
                    has_safety_doc: validate::has_safety_doc_comment(&field.attrs),
                    parse_error: Some(err),
                }
            }
        };
        parsed_fields.push(parsed);
    }

    // `close = dest` and `realloc::payer` write a second field (the
    // destination or payer) as well as the annotated one. Both ends are forced
    // `is_mut` so the writable check and the duplicate-mutable protection
    // cover every account these constraints write.
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

    // A field's mutability is only final here, after the forcing above.
    for field in &mut parsed_fields {
        if field.parse_error.is_some() {
            continue;
        }
        if field.is_dup && field.is_mut {
            field.parse_error = Some(syn::Error::new_spanned(
                &field.ident,
                "Naclac Error: a `dup` field is read-only, but `close` or `realloc::payer` writes \
                 this one. Use `unsafe(alias)` for a mutable duplicate.",
            ));
        }
        if field.is_alias && !field.is_mut {
            field.parse_error = Some(syn::Error::new_spanned(
                &field.ident,
                "Naclac Error: `unsafe(alias)` is for `mut` fields. Use `dup` on a read-only \
                 field that may repeat another account.",
            ));
        }
    }

    Ok(parsed_fields)
}

/// Parses a single field's `#[account(...)]` attribute. `parse_struct_fields`
/// substitutes a safe-defaults `ParsedField` carrying the error in
/// `parse_error` for any field this returns `Err` for, so one field's mistake
/// does not discard the other fields' parsed data.
fn parse_one_field(idx: usize, field: &syn::Field) -> syn::Result<ParsedField> {
    let Some(ident) = field.ident.clone() else {
        return Err(syn::Error::new_spanned(
            field,
            "Naclac Error: Accounts struct fields must be named.",
        ));
    };
    // `Option<>` is unwrapped once, so every later check reads `ty` as the
    // inner type.
    let is_optional = crate::type_classify::is_option_account(&field.ty);
    let ty = crate::type_classify::option_inner_type(&field.ty)
        .cloned()
        .unwrap_or_else(|| field.ty.clone());

    let raw = attrs::RawAttrs::parse(&field.attrs)?;
    validate::finish(raw, idx, ident, ty, is_optional, &field.attrs)
}

#[cfg(test)]
mod tests {
    use {
        super::{parse_struct_fields, ParsedField},
        crate::accounts::field::builtin_namespaces,
    };

    fn parse(fields: &str) -> Vec<ParsedField> {
        let item: syn::ItemStruct = syn::parse_str(&format!("struct S {{ {fields} }}")).unwrap();
        let syn::Fields::Named(named) = &item.fields else {
            panic!("a struct with named fields");
        };
        parse_struct_fields(named).unwrap()
    }

    fn error_of(fields: &str) -> String {
        parse(fields)[0]
            .parse_error
            .as_ref()
            .expect("the attribute must be rejected")
            .to_string()
    }

    #[test]
    fn a_misspelled_key_is_rejected_instead_of_dropping_the_check() {
        let message = error_of("#[account(mut, token::authorty = vault)] a: Account<TokenAccount>");
        assert!(
            message.contains("`token::authorty` is not a constraint"),
            "{message}"
        );
        assert!(message.contains("`token::authority`"), "{message}");
        let message = error_of("#[account(mint::decimal = 6)] m: Account<Mint>");
        assert!(
            message.contains("`mint::decimal` is not a constraint"),
            "{message}"
        );
        let message = error_of("#[account(associated_token::owner = o)] t: Account<TokenAccount>");
        assert!(
            message.contains("`associated_token::owner` is not a constraint"),
            "{message}"
        );
    }

    #[test]
    fn each_namespace_names_its_token_program() {
        for attr in [
            "token::token_program = tp",
            "mint::token_program = tp",
            "associated_token::token_program = tp",
        ] {
            let fields = parse(&format!("#[account({attr})] a: Account<TokenAccount>"));
            assert!(fields[0].parse_error.is_none(), "{attr}");
            let token_program = builtin_namespaces::written_token_program(&fields[0]);
            assert!(token_program.is_some(), "{attr}");
        }
    }

    #[test]
    fn the_bare_program_key_points_to_token_program() {
        let message = error_of("#[account(token::program = tp)] a: Account<TokenAccount>");
        assert!(message.contains("`token::token_program`"), "{message}");
        let message = error_of("#[account(mint::program = tp)] m: Account<Mint>");
        assert!(message.contains("`mint::token_program`"), "{message}");
    }

    fn plugin_keys(field: &ParsedField) -> Vec<(String, String)> {
        use quote::ToTokens;
        field
            .plugin_constraints
            .iter()
            .map(|c| {
                (
                    c.namespace.to_token_stream().to_string().replace(' ', ""),
                    c.key.to_string(),
                )
            })
            .collect()
    }

    #[test]
    fn a_namespace_this_crate_does_not_define_is_a_plugin_constraint() {
        let fields = parse(
            "#[account(mut, my_vault::fee_bps = 30, my_vault::admin = admin)] v: Account<Vault>,
             admin: Signer",
        );
        assert!(fields[0].parse_error.is_none());
        assert_eq!(
            plugin_keys(&fields[0]),
            [
                ("my_vault".to_string(), "fee_bps".to_string()),
                ("my_vault".to_string(), "admin".to_string()),
            ]
        );
        assert!(fields[0].is_mut);
    }

    #[test]
    fn a_qualified_namespace_keeps_its_whole_path() {
        let fields = parse("#[account(defi::vault::fee_bps = 30)] v: Account<Vault>");
        assert!(fields[0].parse_error.is_none());
        assert_eq!(
            plugin_keys(&fields[0]),
            [("defi::vault".to_string(), "fee_bps".to_string())]
        );
    }

    #[test]
    fn a_path_under_a_namespace_the_derive_defines_is_not_a_plugin() {
        let message = error_of("#[account(token::mint::extra = m)] a: Account<TokenAccount>");
        assert!(
            message.contains("`token::mint::extra` is not a constraint"),
            "{message}"
        );
    }

    #[test]
    fn has_one_is_rejected_with_the_naclac_spelling() {
        let message = error_of("#[account(has_one = maker)] a: Account<A>, maker: Signer");
        assert!(message.contains("`has_one` is not supported"), "{message}");
        assert!(message.contains("`#[account(maker = maker)]`"), "{message}");
    }

    #[test]
    fn a_named_field_relation_is_still_a_relation() {
        let fields = parse("#[account(maker = maker)] a: Account<A>, maker: Signer");
        assert!(fields[0].parse_error.is_none());
        assert_eq!(fields[0].relations.len(), 1);
    }

    #[test]
    fn a_field_without_plugin_keys_has_none() {
        let fields = parse("#[account(mut)] a: Account<A>");
        assert!(fields[0].plugin_constraints.is_empty());
    }

    #[test]
    fn a_documented_read_only_dup_is_accepted() {
        let fields = parse("/// SAFETY: read-only.\n#[account(dup)] a: Interface<TokenInterface>");
        assert!(fields[0].parse_error.is_none());
        assert!(fields[0].is_dup);
        assert!(!fields[0].is_mut);
    }

    #[test]
    fn dup_needs_a_safety_comment() {
        let message = error_of("#[account(dup)] a: Interface<TokenInterface>");
        assert!(message.contains("`/// SAFETY:"), "{message}");
        let message = error_of("/// Not a reason.\n#[account(dup)] a: Interface<TokenInterface>");
        assert!(message.contains("`/// SAFETY:"), "{message}");
    }

    #[test]
    fn dup_rejects_everything_that_writes_the_account() {
        for attr in [
            "dup, mut",
            "dup, init, payer = p",
            "dup, init_if_needed, payer = p",
            "dup, init_unchecked, payer = p",
            "dup, close = p",
            "dup, realloc = 8, realloc::payer = p",
        ] {
            let message = error_of(&format!(
                "/// SAFETY: test.\n#[account({attr})] a: Account<A>"
            ));
            assert!(message.contains("is read-only"), "{attr}: {message}");
        }
    }

    #[test]
    fn dup_rejects_being_written_by_another_field() {
        for attr in ["close = a", "realloc = 8, realloc::payer = a"] {
            let fields = parse(&format!(
                "/// SAFETY: test.\n#[account(dup)] a: AccountInfo,
                 #[account({attr})] b: Account<B>"
            ));
            let message = fields[0]
                .parse_error
                .as_ref()
                .expect("the attribute must be rejected")
                .to_string();
            assert!(message.contains("is read-only"), "{attr}: {message}");
        }
    }

    #[test]
    fn dup_and_alias_are_mutually_exclusive() {
        let message = error_of("/// SAFETY: test.\n#[account(dup, unsafe(alias))] a: Account<A>");
        assert!(message.contains("mutually exclusive"), "{message}");
    }

    #[test]
    fn alias_needs_a_mutable_field() {
        let fields = parse("#[account(unsafe(alias))] a: Account<A>");
        let message = fields[0]
            .parse_error
            .as_ref()
            .expect("the attribute must be rejected")
            .to_string();
        assert!(message.contains("Use `dup`"), "{message}");
        let fields = parse(
            "#[account(unsafe(alias))] a: Account<A>,
             #[account(close = a)] b: Account<B>",
        );
        assert!(fields[0].parse_error.is_none());
    }

    #[test]
    fn a_field_names_its_token_program_once() {
        let message = error_of(
            "#[account(token::token_program = a, mint::token_program = b)] t: Account<TokenAccount>",
        );
        assert!(message.contains("more than once"), "{message}");
    }

    #[test]
    fn seed_lists_past_the_pda_limits_are_rejected() {
        let seeds = vec!["b\"s\""; 17].join(", ");
        let message = error_of(&format!(
            "#[account(seeds = [{seeds}], bump)] a: Account<A>"
        ));
        assert!(message.contains("takes at most 16"), "{message}");

        let long = "x".repeat(33);
        let message = error_of(&format!(
            "#[account(seeds = [b\"{long}\"], bump)] a: Account<A>"
        ));
        assert!(message.contains("at most 32"), "{message}");
        let message = error_of(&format!(
            "#[account(seeds = [\"{long}\"], bump)] a: Account<A>"
        ));
        assert!(message.contains("at most 32"), "{message}");

        let seeds = vec!["b\"s\""; 16].join(", ");
        let fields = parse(&format!(
            "#[account(seeds = [{seeds}], bump)] a: Account<A>"
        ));
        assert!(fields[0].parse_error.is_none());
        let fields = parse(
            "#[account(seeds = [b\"01234567890123456789012345678901\"], bump)] a: Account<A>",
        );
        assert!(fields[0].parse_error.is_none());
    }

    #[test]
    fn a_safety_doc_is_recorded_whatever_the_type_is_called() {
        let fields = parse(
            "/// SAFETY: only the address is read.
             raw: Raw,
             plain: Raw",
        );
        assert!(fields[0].has_safety_doc);
        assert!(!fields[1].has_safety_doc);
    }
}
