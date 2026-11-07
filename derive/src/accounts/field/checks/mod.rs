//! Enforcement of a field's `#[account(...)]` constraints in two phases:
//! `generate_security_checks` runs as each account is loaded (flags, address,
//! owner, PDA seeds, namespace constraints), `generate_relational_checks` runs
//! once every account of the struct is loaded. One file per kind of check.

mod address_owner;
mod header;
mod namespaces;
mod pda;
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
    plugin::plugin_args_tokens, relational::generate_relational_checks, seeds::seed_binding_tokens,
};

/// True if this field's declared type is an account wrapper
/// (`Account<T>`/`InterfaceAccount<T>`) *and* the whole crate is in the
/// `bytemuck` layout, the condition both wrapper types route on internally
/// (see `core/src/wrappers/accounts/`). `program_is_zero_copy` is computed once
/// by the caller and passed in.
pub(crate) fn is_zero_copy_account_field(ty: &syn::Type, program_is_zero_copy: bool) -> bool {
    crate::type_classify::is_account_wrapper(ty) && program_is_zero_copy
}

/// The checks that run as `field`'s account is loaded: `(metadata_checks,
/// constraint_checks)`. The metadata checks run before the account is loaded
/// and the constraint checks after. The tokens carry spans pointing at the
/// attribute that produced them.
pub fn generate_security_checks(
    field: &ParsedField,
    all_fields: &[ParsedField],
    is_zero_copy: bool,
) -> (TokenStream, TokenStream) {
    let mut metadata_checks = Vec::new();
    let mut constraint_checks = Vec::new();

    metadata_checks.extend(header::header_checks(field));
    metadata_checks.extend(address_owner::address_checks(field, all_fields));
    metadata_checks.extend(address_owner::owner_checks(field, all_fields));

    match pda::pda_checks(field, is_zero_copy) {
        Err(error) => return (error.clone(), error),
        Ok(Some(PdaChecks::Metadata(tokens))) => metadata_checks.push(tokens),
        Ok(Some(PdaChecks::Constraint(tokens))) => constraint_checks.push(tokens),
        Ok(None) => {}
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
