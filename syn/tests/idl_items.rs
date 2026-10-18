//! Pins the rules `naclac_syn::idl_items` enforces on a crate's IDL-visible
//! declarations: unique names per namespace, unique error codes, and
//! existence that depends only on features.

use {
    naclac_syn::{
        idl_items::{declarations, violations},
        modules::crate_modules,
    },
    std::fs,
};

fn messages(tag: &str, lib_rs: &str) -> Vec<String> {
    let dir = std::env::temp_dir().join(format!(
        "naclac-syn-idl-items-{tag}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(dir.join("src/lib.rs"), lib_rs).unwrap();
    let modules = crate_modules(&dir);
    let found: Vec<String> = violations(&declarations(&modules).unwrap())
        .into_iter()
        .map(|v| v.message)
        .collect();
    let _ = fs::remove_dir_all(&dir);
    found
}

#[test]
fn a_clean_crate_has_no_violations() {
    let found = messages(
        "clean",
        r#"
        #[component] pub struct Vault { pub a: u64 }
        #[event] pub struct Moved { pub a: u64 }
        #[defined_type] pub struct Pair { pub a: u64 }
        #[constant] pub const LIMIT: u64 = 1;
        #[error_code] pub enum First { A, B }
        #[error_code(offset = 6100)] pub enum Second { C }
        "#,
    );
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn two_errors_with_one_name_clash_even_with_different_codes() {
    let found = messages(
        "error-name-clash",
        r#"
        #[error_code] pub enum First { A }
        #[error_code(offset = 6100)] pub enum Second { A }
        "#,
    );
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("`A`") && found[0].contains("crate::Second::A"));
}

#[test]
fn two_types_with_one_name_clash_across_kinds_and_modules() {
    let found = messages(
        "type-clash",
        r#"
        #[component] pub struct Same { pub a: u64 }
        pub mod inner { #[defined_type] pub struct Same { pub a: u64 } }
        "#,
    );
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("`Same`"));
    assert!(found[0].contains("crate::Same") && found[0].contains("crate::inner::Same"));
}

#[test]
fn two_accounts_structs_with_one_name_clash_but_an_accounts_struct_and_a_type_do_not() {
    let found = messages(
        "accounts-clash",
        r#"
        #[derive(Accounts)] pub struct Init { pub a: AccountInfo }
        pub mod inner { #[derive(Accounts)] pub struct Init { pub b: AccountInfo } }
        #[component] pub struct Shared { pub a: u64 }
        #[derive(Accounts)] pub struct Shared2 { pub a: AccountInfo }
        #[derive(Accounts)] pub struct Shared { pub a: AccountInfo }
        "#,
    );
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("accounts struct") && found[0].contains("`Init`"));
}

#[test]
fn two_constants_with_one_name_clash_but_a_constant_and_a_type_do_not() {
    let found = messages(
        "const-clash",
        r#"
        #[constant] pub const LIMIT: u64 = 1;
        pub mod inner { #[constant] pub const LIMIT: u64 = 2; }
        #[defined_type] pub struct LIMIT2 { pub a: u64 }
        #[constant] pub const Pair: u64 = 3;
        #[defined_type] pub struct Pair { pub a: u64 }
        "#,
    );
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("constant"));
}

#[test]
fn two_errors_with_one_code_clash_and_an_offset_resolves_it() {
    let clash = messages(
        "code-clash",
        r#"
        #[error_code] pub enum First { A }
        #[error_code] pub enum Second { B }
        "#,
    );
    assert_eq!(clash.len(), 1, "{clash:?}");
    assert!(clash[0].contains("6000") && clash[0].contains("offset"));

    let resolved = messages(
        "code-offset",
        r#"
        #[error_code] pub enum First { A }
        #[error_code(offset = 6001)] pub enum Second { B }
        "#,
    );
    assert!(resolved.is_empty(), "{resolved:?}");

    let overlapping_explicit = messages(
        "code-explicit",
        r#"
        #[error_code] pub enum First { A, B = 40 }
        #[error_code(offset = 6000)] pub enum Second { C = 40 }
        "#,
    );
    assert!(overlapping_explicit.iter().any(|m| m.contains("6040")));
}

#[test]
fn clashing_declarations_that_never_coexist_carry_their_cfgs() {
    let dir = std::env::temp_dir().join(format!("naclac-syn-idl-items-cfg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(
        dir.join("src/lib.rs"),
        r#"
        #[cfg(feature = "a")] #[component] pub struct Same { pub a: u64 }
        #[cfg(not(feature = "a"))] #[component] pub struct Same { pub a: u64 }
        "#,
    )
    .unwrap();
    let found = violations(&declarations(&crate_modules(&dir)).unwrap());
    let _ = fs::remove_dir_all(&dir);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].cfgs.len(), 2);
}

#[test]
fn only_feature_and_test_cfgs_are_allowed_on_idl_items() {
    let ok = messages(
        "cfg-ok",
        r#"
        #[cfg(feature = "x")] #[event] pub struct A { pub a: u64 }
        #[cfg(all(feature = "x", not(test)))] #[event] pub struct B { pub a: u64 }
        #[cfg(any(feature = "x", feature = "y"))] #[event] pub struct C { pub a: u64 }
        "#,
    );
    assert!(ok.is_empty(), "{ok:?}");

    let bad = messages(
        "cfg-bad",
        r#"
        #[cfg(target_os = "solana")] #[event] pub struct A { pub a: u64 }
        #[cfg(not(target_arch = "bpf"))] #[event] pub struct B { pub a: u64 }
        #[cfg(debug_assertions)] #[event] pub struct C { pub a: u64 }
        "#,
    );
    assert_eq!(bad.len(), 3, "{bad:?}");
    assert!(bad.iter().all(|m| m.contains("only be gated by features")));
}

#[test]
fn error_code_arguments_other_than_offset_are_rejected() {
    use {naclac_syn::idl_items::error_code_offset_tokens, quote::quote};
    assert_eq!(error_code_offset_tokens(quote! {}).unwrap(), 6000);
    assert_eq!(
        error_code_offset_tokens(quote! { offset = 7000 }).unwrap(),
        7000
    );
    assert!(error_code_offset_tokens(quote! { start = 7000 }).is_err());
    assert!(error_code_offset_tokens(quote! { offset = "7000" }).is_err());
    assert!(error_code_offset_tokens(quote! { offset = 7000, extra }).is_err());
}
