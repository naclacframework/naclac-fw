//! `naclac_syn::parser::derives` matches a derive by its exact last path segment.

use naclac_syn::parser::derives;

fn attrs(source: &str) -> Vec<syn::Attribute> {
    syn::parse_str::<syn::ItemStruct>(source)
        .expect("fixture parses")
        .attrs
}

#[test]
fn matches_a_bare_and_a_qualified_derive() {
    assert!(derives(&attrs("#[derive(Accounts)] struct S;"), "Accounts"));
    assert!(derives(
        &attrs("#[derive(Debug, naclac_lang::prelude::Accounts)] struct S;"),
        "Accounts"
    ));
}

#[test]
fn does_not_match_a_name_that_only_contains_it() {
    assert!(!derives(
        &attrs("#[derive(MyAccountsHelper)] struct S;"),
        "Accounts"
    ));
    assert!(!derives(
        &attrs("#[derive(AccountsLoader)] struct S;"),
        "Accounts"
    ));
    assert!(!derives(&attrs("struct S;"), "Accounts"));
}
