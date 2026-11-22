//! Relational checks, run once every account of the struct is loaded:
//! `field = target` relations and an `owner = other_field` constraint, each
//! comparing one account against another.

use {crate::accounts::field::parser::ParsedField, proc_macro2::TokenStream, quote::quote};

/// Relational checks for every field of the struct.
///
/// Generates validation logic that runs *after* all accounts have been loaded into the instruction struct.
/// This is necessary for `field = target` relations, where one account's data must be compared against
/// the public key of another account in the struct.
///
/// On an optional field a check runs only when the account is present, like
/// every other constraint on it. A relation or owner whose target is an
/// optional account is a compile error: an absent target has no address to
/// compare against.
pub fn generate_relational_checks(fields: &[ParsedField], prefix: TokenStream) -> Vec<TokenStream> {
    let mut checks = Vec::new();

    for field in fields {
        let field_name = &field.ident;

        for config in &field.relations {
            let relation_field = &config.field;
            let target_expr = &config.target;

            let mut target_name = None;
            if let syn::Expr::Path(expr_path) = target_expr {
                if let Some(ident) = expr_path.path.get_ident() {
                    target_name = Some(ident.to_string());
                }
            }

            if let Some(ref target_str) = target_name {
                match fields.iter().find(|f| f.ident == target_str) {
                    None => {
                        let error_msg = format!(
                            "Naclac Error: relation target '{}' on '{}' is not a field of this \
                             struct.",
                            target_str, field_name
                        );
                        checks.push(quote! {
                            core::compile_error!(#error_msg);
                        });
                        continue;
                    }
                    Some(target) if target.is_optional => {
                        let error_msg = format!(
                            "Naclac Error: relation target '{}' on '{}' is optional, so an absent \
                             target has no address. Make it required, or check the relation in the \
                             handler.",
                            target_str, field_name
                        );
                        checks.push(quote! {
                            core::compile_error!(#error_msg);
                        });
                        continue;
                    }
                    Some(_) => {}
                }
            }

            // We do NOT check relation if the account is being newly initialized!
            if field.init_config.is_none() {
                let idx = field.index;
                let error_logic = if let Some(custom_error) = &config.custom_error {
                    quote! { #custom_error.into() }
                } else {
                    quote! { naclac_lang::prelude::NaclacError::Unauthorized.err(#idx) }
                };

                // Account<T> implements Deref<Target=T> in both its Borsh and
                // zero-copy branches, so fields are reachable via auto-deref
                // regardless of backend.
                let check_logic = if field.is_optional {
                    quote! {
                        if let Some(__account) = &#prefix.#field_name {
                            let __val = __account.#relation_field;
                            let __target_key = (#prefix.#target_expr).address();
                            if __val != __target_key {
                                return Err(#error_logic);
                            }
                        }
                    }
                } else {
                    quote! {
                        let __val = #prefix.#field_name.#relation_field;
                        let __target_key = (#prefix.#target_expr).address();
                        if __val != __target_key {
                            return Err(#error_logic);
                        }
                    }
                };

                checks.push(check_logic);
            }
        }

        // Relational Owner Check
        if let Some(syn::Expr::Path(expr_path)) = &field.owner {
            if let Some(ident) = expr_path.path.get_ident() {
                if let Some(target) = fields.iter().find(|f| &f.ident == ident) {
                    if target.is_optional {
                        let error_msg = format!(
                            "Naclac Error: `owner = {}` on '{}' names an optional account, which \
                             has no address when absent. Make it required, or check the owner in \
                             the handler.",
                            ident, field_name
                        );
                        checks.push(quote! {
                            core::compile_error!(#error_msg);
                        });
                        continue;
                    }
                    let idx = field.index;
                    let owner_check = if field.is_optional {
                        quote! {
                            if let Some(__account) = &#prefix.#field_name {
                                let __expected_owner = naclac_lang::prelude::ToAddress::address(&#prefix.#ident);
                                let __actual_owner = naclac_lang::prelude::Owner::program_owner(__account);
                                if __actual_owner != __expected_owner {
                                    return Err(naclac_lang::prelude::NaclacError::ConstraintOwner.err(#idx));
                                }
                            }
                        }
                    } else {
                        quote! {
                            let __expected_owner = naclac_lang::prelude::ToAddress::address(&#prefix.#ident);
                            let __actual_owner = naclac_lang::prelude::Owner::program_owner(&#prefix.#field_name);
                            if __actual_owner != __expected_owner {
                                return Err(naclac_lang::prelude::NaclacError::ConstraintOwner.err(#idx));
                            }
                        }
                    };
                    checks.push(owner_check);
                }
            }
        }
    }

    checks
}

#[cfg(test)]
mod tests {
    use {
        super::generate_relational_checks, crate::accounts::field::parser::parse_struct_fields,
        quote::quote,
    };

    fn checks(fields: &str) -> String {
        let item: syn::ItemStruct = syn::parse_str(&format!("struct S {{ {fields} }}")).unwrap();
        let syn::Fields::Named(named) = &item.fields else {
            panic!("a struct with named fields");
        };
        let parsed = parse_struct_fields(named).unwrap();
        generate_relational_checks(&parsed, quote! { __me })
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn a_required_relation_reads_the_field_directly() {
        let code = checks("#[account(maker = maker)] a: Account<A>, maker: Signer");
        assert!(code.contains("__me . a . maker"), "{code}");
        assert!(!code.contains("if let Some"), "{code}");
    }

    #[test]
    fn an_optional_relation_runs_only_when_the_account_is_present() {
        let code = checks("#[account(maker = maker)] a: Option<Account<A>>, maker: Signer");
        assert!(
            code.contains("if let Some (__account) = & __me . a"),
            "{code}"
        );
        assert!(code.contains("__account . maker"), "{code}");
    }

    #[test]
    fn an_optional_owner_relation_runs_only_when_the_account_is_present() {
        let code = checks("#[account(owner = o)] a: Option<Account<A>>, o: Signer");
        assert!(
            code.contains("if let Some (__account) = & __me . a"),
            "{code}"
        );
        assert!(code.contains("program_owner (__account)"), "{code}");
    }

    #[test]
    fn a_relation_target_that_is_optional_is_an_error() {
        let code = checks("#[account(maker = maker)] a: Account<A>, maker: Option<Signer>");
        assert!(code.contains("compile_error"), "{code}");
        assert!(code.contains("is optional"), "{code}");
        assert!(!code.contains("__val"), "{code}");
    }

    #[test]
    fn an_owner_target_that_is_optional_is_an_error() {
        let code = checks("#[account(owner = o)] a: Account<A>, o: Option<Signer>");
        assert!(code.contains("compile_error"), "{code}");
        assert!(code.contains("names an optional account"), "{code}");
        assert!(!code.contains("__actual_owner"), "{code}");
    }
}
