//! The checks generated from the builtin constraint namespaces
//! (`builtin_namespaces::NAMESPACES`), in table order.

use {
    super::plugin::plugin_call_tokens,
    crate::accounts::field::{
        builtin_namespaces::{self, CheckWhen},
        parser::ParsedField,
    },
    proc_macro2::{Ident, TokenStream},
    quote::{format_ident, quote},
};

/// One check per namespace whose trigger keys are written on `field`. A
/// namespace missing a key it needs together with another, or a value it
/// cannot fill from another field, yields that namespace's compile error in
/// place of its check.
pub(super) fn namespace_checks(
    field: &ParsedField,
    all_fields: &[ParsedField],
) -> Vec<TokenStream> {
    let mut checks = Vec::new();
    for namespace in builtin_namespaces::NAMESPACES {
        let runs = match namespace.check.when {
            CheckWhen::UnlessCreated => field.init_config.is_none() || field.is_init_if_needed,
            CheckWhen::OnlyExisting => field.init_config.is_none(),
        };
        if !runs {
            continue;
        }
        let written = builtin_namespaces::written(field, namespace);
        let program_index = if namespace.check.written_token_program {
            builtin_namespaces::written_token_program_index(field, all_fields)
        } else {
            None
        };
        let triggered = program_index.is_some()
            || written
                .iter()
                .any(|(key, _)| namespace.check.trigger.iter().any(|t| *key == t));
        if !triggered {
            continue;
        }
        if let Some(missing) = namespace
            .check
            .together
            .iter()
            .find(|required| !written.iter().any(|(key, _)| *key == required.key))
        {
            let message = missing.message;
            checks.push(quote! { compile_error!(#message); });
            continue;
        }

        let mut filled_values: Vec<(Ident, syn::Expr)> = Vec::new();
        let mut unresolved = None;
        match program_index {
            Some(Ok(idx)) => filled_values.push((
                format_ident!("{}", builtin_namespaces::TOKEN_PROGRAM_KEY),
                syn::parse_quote! { accounts[#idx] },
            )),
            Some(Err(err)) => unresolved = Some(err),
            None => {}
        }
        for filled in namespace.check.filled {
            match builtin_namespaces::resolve_filled(filled, field, all_fields) {
                Ok(idx) => filled_values.push((
                    format_ident!("{}", filled.key),
                    syn::parse_quote! { accounts[#idx] },
                )),
                Err(err) => {
                    unresolved = Some(err);
                    break;
                }
            }
        }
        if let Some(err) = unresolved {
            checks.push(err);
            continue;
        }

        let mut members = written;
        members.extend(filled_values.iter().map(|(key, value)| (key, value)));
        let path = builtin_namespaces::path_tokens(namespace);
        let call = plugin_call_tokens(&path, &members, field, all_fields);
        checks.push(quote! { { #call } });
    }
    checks
}
