//! Constraints written as `namespace::key = value` on a field: one `check`
//! call per namespace, plus the argument and value-reference builders that
//! `init_cpi` shares for the creation path.

use {
    crate::accounts::field::{builtin_namespaces, parser::ParsedField},
    proc_macro2::{Ident, TokenStream},
    quote::quote,
};

/// Code for a field's `namespace::key = value` constraints, one group per
/// namespace. It sits after every core check of the field, so a constraint
/// only ever adds a check. The tokens keep the user's own spans so a mistake
/// points at the key.
pub(super) fn generate_plugin_checks(
    field: &ParsedField,
    all_fields: &[ParsedField],
) -> TokenStream {
    use quote::ToTokens;

    let mut groups: Vec<(syn::Path, Vec<(&Ident, &syn::Expr)>)> = Vec::new();
    for constraint in &field.plugin_constraints {
        if builtin_namespaces::of(constraint).is_some() {
            continue;
        }
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
    let args = plugin_args_tokens(namespace, members, all_fields);
    quote! {
        const _: () = #namespace::RULES.check(
            &[#(#key_names),*],
            naclac_lang::prelude::ConstraintPhase::#phase,
        );
        <#namespace::Constraint as naclac_lang::prelude::AccountConstraint<_>>::check(
            &#field_ident,
            &#args,
        )
        .map_err(|e| e.err(#idx))?;
    }
}

/// A namespace's `Args` built from `members`:
/// `<Args as Default>::default().key(value)..`, values in the order written,
/// each converted by `ConstraintValue` to the type its setter takes.
pub(crate) fn plugin_args_tokens(
    namespace: &TokenStream,
    members: &[(&Ident, &syn::Expr)],
    all_fields: &[ParsedField],
) -> TokenStream {
    let keys = members.iter().map(|(key, _)| *key);
    let values = members.iter().map(|(_, value)| {
        let source = value_reference(value, all_fields);
        quote! { naclac_lang::prelude::ConstraintValue::into_value(#source) }
    });
    quote! {
        <#namespace::Args as ::core::default::Default>::default() #(.#keys(#values))*
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

#[cfg(test)]
mod plugin_tests {
    use {
        super::generate_plugin_checks,
        crate::accounts::field::{
            checks::generate_security_checks,
            parser::{parse_struct_fields, ParsedField},
        },
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
    fn associated_token_keys_on_an_existing_account_call_its_constraint() {
        let generated = constraint_code(
            "#[account(associated_token::mint = mint, associated_token::authority = owner, associated_token::bump = 7)]
             ata: Account<TokenAccount>,
             mint: Account<Mint>,
             owner: Signer,
             token_program: Program<Token>",
        );
        let call = concat!(
            "<naclac_lang::token_constraints::associated_token::Constraintas",
            "naclac_lang::prelude::AccountConstraint<_>>::check(&ata,"
        );
        let removed = "find_associated_token_bump";

        assert_eq!(generated.matches(call).count(), 1, "{generated}");
        assert!(sets(&generated, "mint", "&accounts[1usize]"), "{generated}");
        assert!(sets(&generated, "authority", "&owner"), "{generated}");
        assert!(sets(&generated, "bump", "&(7)"), "{generated}");
        assert!(
            sets(&generated, "token_program", "&(accounts[3usize])"),
            "{generated}"
        );
        assert!(!generated.contains(removed), "{generated}");
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
