//! Code generated for the `init` / `init_if_needed` constraint: the CPI that
//! creates a field's account. `generate_init_cpi` validates the field, then
//! hands over to the constraint namespace that creates it
//! (`namespace_create`, `plugin_init`) or, with none, to the plain system
//! program `create_account` path (`system_create`).

mod namespace_create;
mod plugin_init;
mod signer_seeds;
mod system_create;

use {
    crate::accounts::field::{
        builtin_namespaces,
        parser::{InitConfig, ParsedField},
    },
    proc_macro2::TokenStream,
    quote::quote,
    syn::Type,
};

/// Returns the marker ident (e.g. `System`, `Token`, `Token2022`,
/// `AssociatedToken`, `TokenInterface`) for a field typed `Program<Marker>`
/// or `Interface<Marker>`, or `None` for any other type.
pub(crate) fn program_marker_ident(ty: &Type) -> Option<String> {
    if let Type::Path(type_path) = ty {
        let segment = type_path.path.segments.last()?;
        if segment.ident == "Program" || segment.ident == "Interface" {
            if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                for arg in &args.args {
                    if let syn::GenericArgument::Type(Type::Path(inner_path)) = arg {
                        return inner_path.path.segments.last().map(|s| s.ident.to_string());
                    }
                }
            }
        }
    }
    None
}

/// A reference to the account `expr` names: its raw slot in `accounts` when it is
/// a field of the struct, else the expression as written.
fn get_account_reference(expr: &syn::Expr, all_fields: &[ParsedField]) -> TokenStream {
    if let syn::Expr::Path(expr_path) = expr {
        if let Some(ident) = expr_path.path.get_ident() {
            if let Some(pos) = all_fields.iter().position(|f| &f.ident == ident) {
                return quote! { &accounts[#pos] };
            }
        }
    }
    quote! { &#expr }
}

/// What every way of creating an account needs: the field, the struct's other
/// fields, the `init` options, the payer, the PDA signer seeds and the index of
/// the system program account.
struct InitCtx<'a> {
    field: &'a ParsedField,
    all_fields: &'a [ParsedField],
    init_config: &'a InitConfig,
    payer_ref: TokenStream,
    signer_seeds_logic: TokenStream,
    system_program_idx: usize,
}

/// Generates the creation logic for `#[account(init, ...)]` and
/// `#[account(init_if_needed, ...)]`, empty for a field with neither.
///
/// It delegates to `pinocchio_system` or the standard Solana program
/// depending on the active compilation target, handles space calculation and
/// the lamport transfer, and injects the zero-copy discriminator.
pub fn generate_init_cpi(field: &ParsedField, all_fields: &[ParsedField]) -> TokenStream {
    let init_config = match &field.init_config {
        Some(config) => config,
        None => return quote! {},
    };

    let creator_accepts_extra_accounts = builtin_namespaces::creator(field)
        .is_some_and(|(_, create)| create.extra_accounts.is_some());
    if field.extra_accounts.is_some() && !creator_accepts_extra_accounts {
        return syn::Error::new_spanned(&field.ident, builtin_namespaces::EXTRA_ACCOUNTS_ELSEWHERE)
            .to_compile_error();
    }

    // The payer is resolved by raw index into the accounts slice, not by a
    // typed local, so it can be declared anywhere in the struct.
    let payer_ref = get_account_reference(&init_config.payer, all_fields);
    let signer_seeds_logic = signer_seeds::signer_seeds_tokens(field);

    let system_program_idx = match all_fields
        .iter()
        .position(|f| program_marker_ident(&f.ty).as_deref() == Some("System"))
    {
        Some(idx) => idx,
        None => {
            return syn::Error::new_spanned(
                &field.ident,
                "Naclac Error: `init` needs a `Program<System>` field in the Accounts struct.",
            )
            .to_compile_error();
        }
    };

    let ctx = InitCtx {
        field,
        all_fields,
        init_config,
        payer_ref,
        signer_seeds_logic,
        system_program_idx,
    };
    match builtin_namespaces::creator(field) {
        Some((namespace, create)) => namespace_create::tokens(&ctx, namespace, create),
        None => system_create::tokens(&ctx),
    }
}

#[cfg(test)]
mod tests {
    use {
        super::generate_init_cpi,
        crate::accounts::field::parser::{parse_struct_fields, ParsedField},
    };

    const INTO_VALUE: &str = "naclac_lang::prelude::ConstraintValue::into_value";

    const MINT_INIT: &str = concat!(
        "<naclac_lang::token_constraints::mint::Constraintas",
        "naclac_lang::prelude::AccountConstraintInit<Account<Mint>>>"
    );

    const TOKEN_INIT: &str = concat!(
        "<naclac_lang::token_constraints::token::Constraintas",
        "naclac_lang::prelude::AccountConstraintInit<Account<TokenAccount>>>"
    );

    const MINT_FIELDS: &str = "
        mint: Account<Mint>,
        owner: Signer,
        #[account(mut)] payer: Signer,
        token_program: Program<Token>,
        system_program: Program<System>";

    fn parse(fields: &str) -> Vec<ParsedField> {
        let item: syn::ItemStruct = syn::parse_str(&format!("struct S {{ {fields} }}")).unwrap();
        let syn::Fields::Named(named) = &item.fields else {
            panic!("a struct with named fields");
        };
        parse_struct_fields(named).unwrap()
    }

    fn init_code(fields: &str) -> String {
        let parsed = parse(fields);
        generate_init_cpi(&parsed[0], &parsed)
            .to_string()
            .replace(' ', "")
    }

    fn mint_init_code(kind: &str) -> String {
        let attribute = "payer = payer, mint::decimals = 6, mint::authority = owner";
        init_code(&format!("#[account({kind}, {attribute})] {MINT_FIELDS}"))
    }

    #[test]
    fn a_mint_is_created_through_the_mint_constraint() {
        let generated = mint_init_code("init");
        let owner = format!("{MINT_INIT}::owner(&__init_args)");
        let space = format!("{MINT_INIT}::space(&__init_args)");
        let initialize = format!("{MINT_INIT}::initialize(info,0usize,");
        let decimals = format!(".decimals({INTO_VALUE}(&(6)))");
        let program = format!(".token_program({INTO_VALUE}(&(accounts[3usize])))");

        for expected in [owner, space, initialize, decimals, program] {
            assert!(generated.contains(&expected), "{generated}");
        }
        for removed in ["initialize_mint_signed", "TOKEN_PROGRAM_ID"] {
            assert!(!generated.contains(removed), "{generated}");
        }
    }

    #[test]
    fn a_token_account_is_created_through_the_token_constraint() {
        let generated = init_code(
            "#[account(init, payer = payer, token::mint = mint, token::authority = owner)]
             vault: Account<TokenAccount>,
             mint: Account<Mint>,
             owner: Signer,
             #[account(mut)] payer: Signer,
             token_program: Program<Token>,
             system_program: Program<System>",
        );
        let initialize = format!("{TOKEN_INIT}::initialize(");
        let mint = format!(".mint({INTO_VALUE}(&accounts[1usize]))");
        let authority = format!(".authority({INTO_VALUE}(&owner))");

        for expected in [initialize, mint, authority] {
            assert!(generated.contains(&expected), "{generated}");
        }
        let removed = "initialize_account_signed";
        assert!(!generated.contains(removed), "{generated}");
    }

    #[test]
    fn an_associated_token_account_is_created_through_its_constraint() {
        let generated = init_code(
            "#[account(init_if_needed, payer = payer, associated_token::mint = mint, associated_token::authority = owner)]
             ata: Account<TokenAccount>,
             mint: Account<Mint>,
             owner: Signer,
             #[account(mut)] payer: Signer,
             token_program: Program<Token>,
             associated_token_program: Program<AssociatedToken>,
             system_program: Program<System>",
        );
        let create = concat!(
            "<naclac_lang::token_constraints::associated_token::Constraintas",
            "naclac_lang::prelude::AccountConstraintCreate<Account<TokenAccount>>>::create("
        );
        let program = format!(".associated_token_program({INTO_VALUE}(&(accounts[5usize])))");
        let expected = [
            create.to_string(),
            program,
            "idempotent:true".to_string(),
            "extra_accounts:::core::option::Option::None".to_string(),
        ];

        for expected in expected {
            assert!(generated.contains(&expected), "{generated}");
        }
        for removed in ["AccountAlreadyInitialized", "create_idempotent"] {
            assert!(!generated.contains(removed), "{generated}");
        }
    }

    #[test]
    fn an_init_field_must_be_empty_and_an_init_if_needed_field_need_not() {
        let created = mint_init_code("init");
        let maybe_created = mint_init_code("init_if_needed");
        assert!(created.contains("AccountAlreadyInitialized"), "{created}");
        assert!(!maybe_created.contains("AccountAlreadyInitialized"));
    }
}
