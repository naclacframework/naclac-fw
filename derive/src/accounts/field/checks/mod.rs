//! Enforcement of a field's `#[account(...)]` constraints in two phases:
//! `generate_security_checks` runs as each account is loaded (flags, address,
//! owner, PDA seeds, namespace constraints), `generate_relational_checks` runs
//! once every account of the struct is loaded. One file per kind of check.

mod address_owner;
mod header;
mod namespaces;
pub(in crate::accounts) mod pda;
mod plugin;
mod relational;
mod seeds;

use {
    crate::accounts::field::{builtin_namespaces, parser::ParsedField},
    pda::PdaChecks,
    proc_macro2::TokenStream,
    quote::quote,
    syn::spanned::Spanned,
};

pub(crate) use {
    plugin::plugin_args_tokens,
    relational::generate_relational_checks,
    seeds::{seed_binding_tokens, typed_seeds_call},
};

#[cfg(feature = "idl-build")]
pub(crate) use seeds::mentions_any_name;

/// The names an Accounts struct's seeds can use that have no value until a
/// transaction arrives: its account fields, its `#[instruction(..)]`
/// arguments and `program_id`. A seed naming none of them is computed by the
/// compiler.
pub(crate) fn runtime_names(
    fields: &[ParsedField],
    instruction_args: &[syn::Ident],
) -> Vec<String> {
    fields
        .iter()
        .map(|field| field.ident.to_string())
        .chain(instruction_args.iter().map(ToString::to_string))
        .chain(std::iter::once("program_id".to_string()))
        .collect()
}

/// The checks that run as `field`'s account is loaded: `(metadata_checks,
/// constraint_checks)`. The metadata checks run before the account is loaded
/// and the constraint checks after. The tokens carry spans pointing at the
/// attribute that produced them. `runtime_names` is `runtime_names(..)` of
/// the struct.
pub fn generate_security_checks(
    field: &ParsedField,
    all_fields: &[ParsedField],
    runtime_names: &[String],
) -> (TokenStream, TokenStream) {
    let mut metadata_checks = Vec::new();
    let mut constraint_checks = Vec::new();

    metadata_checks.extend(header::header_checks(field));
    metadata_checks.extend(address_owner::address_checks(field, all_fields));
    metadata_checks.extend(address_owner::owner_checks(field, all_fields));

    match pda::pda_checks(field, runtime_names) {
        Some(PdaChecks::Metadata(tokens)) => metadata_checks.push(tokens),
        Some(PdaChecks::Constraint(tokens)) => constraint_checks.push(tokens),
        None => {}
    }

    constraint_checks.extend(namespaces::namespace_checks(field, all_fields));

    let field_span = field.ident.span();
    let metadata_checks_span = field
        .address
        .as_ref()
        .map(|a| a.span())
        .or_else(|| field.owner.as_ref().map(|o| o.span()))
        .unwrap_or(field_span);
    let constraint_checks_span = field
        .pda_seed
        .as_ref()
        .map(|s| s.span())
        .or_else(|| builtin_namespaces::check_span(field))
        .unwrap_or(field_span);

    let core_checks = set_span_recur(quote! { #(#constraint_checks)* }, constraint_checks_span);
    let plugin_checks = plugin::generate_plugin_checks(field, all_fields);

    (
        set_span_recur(quote! { #(#metadata_checks)* }, metadata_checks_span),
        quote! { #core_checks #plugin_checks },
    )
}

fn set_span_recur(tokens: TokenStream, span: proc_macro2::Span) -> TokenStream {
    tokens
        .into_iter()
        .map(|mut tree| match &mut tree {
            proc_macro2::TokenTree::Group(group) => {
                let delim = group.delimiter();
                let stream = set_span_recur(group.stream(), span);
                let mut new_group = proc_macro2::Group::new(delim, stream);
                new_group.set_span(span);
                proc_macro2::TokenTree::Group(new_group)
            }
            _ => {
                tree.set_span(span);
                tree
            }
        })
        .collect()
}
