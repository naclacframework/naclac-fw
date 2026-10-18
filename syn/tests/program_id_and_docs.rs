//! Source-parsing IDL route: an account pinned with `address = crate::ID` (or
//! a bare `ID`) is marked as the program's own id, a bare `signer` keyword
//! counts as a signer, and an instruction's docs come from the `lib.rs`
//! wrapper when it has any, otherwise from the implementation elsewhere in the
//! crate.

use {naclac_syn::parse_workspace_program, std::fs};

const LIB_RS: &str = r#"
use naclac_lang::prelude::*;

pub mod handlers;

#[derive(Accounts)]
pub struct Pinned {
    #[account(address = crate::ID)]
    pub via_crate_path: AccountInfo,
    #[account(address = ID)]
    pub via_bare_ident: AccountInfo,
    #[account(signer)]
    pub attribute_signer: AccountInfo,
    pub plain: AccountInfo,
    #[account(address = expected_address)]
    pub dynamic: AccountInfo,
    #[account(address = MPL_TOKEN_METADATA_PROGRAM_ID)]
    pub metadata_program: AccountInfo,
    #[account(address = naclac_lang::prelude::SYSTEM_PROGRAM_ID)]
    pub system: AccountInfo,
    #[account(address = TOKEN_PROGRAM_ID)]
    pub token: AccountInfo,
    pub system_program: Program<Custom>,
    pub anything: Program<System>,
}

#[program]
pub mod sample {
    /// Wrapper docs.
    pub fn documented_on_both(ctx: Context<Pinned>) -> Result {
        handlers::documented_on_both(ctx)
    }

    pub fn documented_on_handler_only(ctx: Context<Pinned>) -> Result {
        handlers::documented_on_handler_only(ctx)
    }

    pub fn undocumented(ctx: Context<Pinned>) -> Result {
        handlers::undocumented(ctx)
    }
}
"#;

const HANDLERS_RS: &str = r#"
/// Handler docs that must lose to the wrapper's.
pub fn documented_on_both(ctx: Context<Pinned>) -> Result {
    Ok(())
}

/// Handler-only docs.
pub fn documented_on_handler_only(ctx: Context<Pinned>) -> Result {
    Ok(())
}

pub fn undocumented(ctx: Context<Pinned>) -> Result {
    Ok(())
}
"#;

fn sample_program() -> naclac_syn::types::NaclacProgram {
    let dir = std::env::temp_dir().join(format!(
        "naclac-syn-program-id-and-docs-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(dir.join("src/lib.rs"), LIB_RS).unwrap();
    fs::write(dir.join("src/handlers.rs"), HANDLERS_RS).unwrap();
    let program = parse_workspace_program(&dir, "sample");
    fs::remove_dir_all(&dir).unwrap();
    program
}

#[test]
fn accounts_pinned_to_the_program_id_are_marked_and_others_are_not() {
    let program = sample_program();
    let accounts = &program.instructions[0].accounts;
    let by_name = |name: &str| accounts.iter().find(|a| a.name == name).unwrap();

    for name in ["via_crate_path", "via_bare_ident"] {
        assert!(by_name(name).address_is_program_id, "{name}");
        assert_eq!(by_name(name).address, None, "{name}");
    }
    assert!(!by_name("attribute_signer").address_is_program_id);
    assert!(!by_name("plain").address_is_program_id);
}

#[test]
fn address_paths_resolve_by_exact_name_and_unknown_ones_report_no_address() {
    let program = sample_program();
    let accounts = &program.instructions[0].accounts;
    let address = |name: &str| {
        accounts
            .iter()
            .find(|a| a.name == name)
            .unwrap()
            .address
            .clone()
    };

    assert_eq!(
        address("system").as_deref(),
        Some("11111111111111111111111111111111")
    );
    assert_eq!(
        address("token").as_deref(),
        Some("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA")
    );
    // A variable, and a name that merely contains "token", must not become an address.
    assert_eq!(address("dynamic"), None);
    assert_eq!(address("metadata_program"), None);
}

#[test]
fn a_program_account_address_comes_from_its_type_never_from_its_field_name() {
    let program = sample_program();
    let accounts = &program.instructions[0].accounts;
    let address = |name: &str| {
        accounts
            .iter()
            .find(|a| a.name == name)
            .unwrap()
            .address
            .clone()
    };

    assert_eq!(address("system_program"), None);
    assert_eq!(
        address("anything").as_deref(),
        Some("11111111111111111111111111111111")
    );
}

#[test]
fn a_bare_signer_keyword_marks_a_non_signer_typed_field_as_signer() {
    let program = sample_program();
    let accounts = &program.instructions[0].accounts;
    let signer = |name: &str| accounts.iter().find(|a| a.name == name).unwrap().signer;

    assert!(signer("attribute_signer"));
    assert!(!signer("plain"));
}

#[test]
fn instruction_docs_prefer_the_wrapper_then_fall_back_to_the_implementation() {
    let program = sample_program();
    let docs = |name: &str| {
        program
            .instructions
            .iter()
            .find(|ix| ix.name == name)
            .unwrap()
            .docs
            .clone()
    };

    assert_eq!(docs("documented_on_both"), vec!["Wrapper docs."]);
    assert_eq!(
        docs("documented_on_handler_only"),
        vec!["Handler-only docs."]
    );
    assert!(docs("undocumented").is_empty());
}
