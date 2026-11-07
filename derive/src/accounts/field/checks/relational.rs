//! Relational checks, run once every account of the struct is loaded:
//! `field = target` relations, an `owner = other_field` constraint and a
//! written token program, each comparing one account against another.

use {
    crate::accounts::field::{builtin_namespaces, parser::ParsedField},
    proc_macro2::TokenStream,
    quote::quote,
};

/// Relational checks for every field of the struct.
///
/// Generates validation logic that runs *after* all accounts have been loaded into the instruction struct.
/// This is necessary for `field = target` relations, where one account's data must be compared against
/// the public key of another account in the struct.
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
                if !fields.iter().any(|f| f.ident == target_str) {
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
                let check_logic = quote! {
                    let __val = #prefix.#field_name.#relation_field;
                    let __target_key = (#prefix.#target_expr).address();
                    if __val != __target_key {
                        return Err(#error_logic);
                    }
                };

                checks.push(check_logic);
            }
        }

        // Relational Owner Check
        if let Some(syn::Expr::Path(expr_path)) = &field.owner {
            if let Some(ident) = expr_path.path.get_ident() {
                if fields.iter().any(|f| &f.ident == ident) {
                    let idx = field.index;
                    checks.push(quote! {
                        let __expected_owner = naclac_lang::prelude::ToAddress::address(&#prefix.#ident);
                        let __actual_owner = naclac_lang::prelude::Owner::program_owner(&#prefix.#field_name);
                        if __actual_owner != __expected_owner {
                            return Err(naclac_lang::prelude::NaclacError::ConstraintOwner.err(#idx));
                        }
                    });
                }
            }
        }

        // A written token program must own the account once it is loaded.
        if let Some(prog_expr) = builtin_namespaces::written_token_program(field) {
            let idx = field.index;
            checks.push(quote! {
                let __expected_prog = naclac_lang::prelude::ToAddress::address(&#prefix.#prog_expr);
                let __actual_owner = naclac_lang::prelude::Owner::program_owner(&#prefix.#field_name);
                if __actual_owner != __expected_prog {
                    return Err(naclac_lang::prelude::NaclacError::ProgramIdMismatch.err(#idx));
                }
            });
        }
    }

    checks
}
