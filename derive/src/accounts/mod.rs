//! `#[derive(Accounts)]`: validates and loads an instruction's accounts.
//!
//! `expand_derive_accounts` parses the struct, then builds the layout-dependent
//! pieces once per layout (`variant`) and the shared pieces (`bumps`,
//! `loadable`, `pinocchio`, `teardown`, `account_metas`) and assembles them.
//! `idl_build` adds the `idl-build` output.

mod account_metas;
mod bumps;
pub(crate) mod field;
mod field_loaders;
mod idl_build;
mod ix_args;
mod layout;
mod loadable;
mod pinocchio;
mod signer;
mod stack_guard;
mod teardown;
mod variant;

use {
    crate::{
        accounts::field::{checks, load_order, parser},
        type_classify,
    },
    bumps::Bumps,
    ix_args::InstructionArgs,
    layout::Layout,
    proc_macro::TokenStream,
    proc_macro2::TokenStream as TokenStream2,
    quote::quote,
    syn::{parse_macro_input, Data, DeriveInput, Fields},
    variant::{Variant, Variants},
};

/// What the per-concern generators share: the struct, its parsed fields, its
/// `Bumps` companion and the relational checks run once all fields are loaded.
struct Shared<'a> {
    struct_name: &'a syn::Ident,
    generics: &'a syn::Generics,
    fields: &'a [parser::ParsedField],
    bumps: &'a Bumps,
    relational_checks: Vec<TokenStream2>,
}

impl Shared<'_> {
    /// The field names, used to build `Self { .. }` from the loaded locals.
    fn struct_instantiation(&self) -> Vec<TokenStream2> {
        self.fields
            .iter()
            .map(|field| {
                let field_name = &field.ident;
                quote! { #field_name }
            })
            .collect()
    }
}

const PUBKEY_DEPRECATED: &str = "Naclac Error: 'Pubkey' is deprecated in Solana v3; use 'Address'.";

/// Entrypoint for `#[derive(Accounts)]` expansion.
///
/// Cargo gives a proc macro no reliable way to see which features the calling
/// crate activated, so the layout cannot be chosen here: everything that
/// depends on it is generated for each layout and emitted behind that layout's
/// own `#[cfg(...)]`, resolved by the calling crate's compiler. The layout is
/// the feature the crate enables (`bytemuck`, `borsh` or `wincode`),
/// independent of the runtime (`solana` or `pinocchio`).
pub fn expand_derive_accounts(item: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(item as DeriveInput);
    let struct_name = &ast.ident;

    let fields_named = match &ast.data {
        Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(fields_named) => fields_named,
            _ => {
                return syn::Error::new_spanned(
                    struct_name,
                    "Naclac Error: #[derive(Accounts)] must have named fields.",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(
                struct_name,
                "Naclac Error: #[derive(Accounts)] can only be used on structs.",
            )
            .to_compile_error()
            .into();
        }
    };

    for field in &fields_named.named {
        let ty = &field.ty;
        if type_classify::is_deprecated_pubkey(ty) {
            return syn::Error::new_spanned(ty, PUBKEY_DEPRECATED)
                .to_compile_error()
                .into();
        }
    }

    let parsed_fields = match parser::parse_struct_fields(fields_named) {
        Ok(fields) => fields,
        Err(err) => return err.to_compile_error().into(),
    };
    let args = InstructionArgs::parse(&ast.attrs);

    for ty in &args.types {
        if type_classify::is_deprecated_pubkey(ty) {
            return syn::Error::new_spanned(ty, PUBKEY_DEPRECATED)
                .to_compile_error()
                .into();
        }
    }

    let load_order = match load_order::load_order(&parsed_fields) {
        Ok(order) => order,
        Err(err) => return err.to_compile_error().into(),
    };

    let variants = Variants {
        zero_copy: Variant::build(Layout::ZeroCopy, &parsed_fields, &args, &load_order),
        borsh: Variant::build(Layout::Borsh, &parsed_fields, &args, &load_order),
        wincode: Variant::build(Layout::Wincode, &parsed_fields, &args, &load_order),
    };

    let bumps = bumps::build(struct_name, &parsed_fields);
    let shared = Shared {
        struct_name,
        generics: &ast.generics,
        fields: &parsed_fields,
        bumps: &bumps,
        relational_checks: checks::generate_relational_checks(&parsed_fields, quote! { __me }),
    };

    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();
    let bumps_struct_name = &bumps.struct_name;
    let bumps_struct_def = &bumps.struct_def;
    let mut_mask_body = loadable::mut_mask_body(&shared);
    let pinocchio_fns = pinocchio::load_and_validate_fns(&shared, &variants);
    let teardown_fn = teardown::teardown_fn(&parsed_fields, &bumps, &variants);
    let loadable_impls = loadable::loadable_impls(&shared, &variants);
    let account_helper_impls = account_metas::account_helper_impls(&shared);

    let zc_size_guard_consts = &variants.zero_copy.size_guard_consts;
    let borsh_size_guard_consts = &variants.borsh.size_guard_consts;
    let wincode_size_guard_consts = &variants.wincode.size_guard_consts;

    let idl_build_accounts_tokens =
        idl_build::idl_build_accounts_impl(struct_name, &parsed_fields, fields_named, &args.names);
    let idl_insert_account_types_tokens =
        idl_build::idl_insert_account_types_impl(struct_name, &parsed_fields);

    let needs_event_cpi = ast
        .attrs
        .iter()
        .any(|attr| attr.path().is_ident("__naclac_event_cpi"));

    let expanded = quote! {
        #bumps_struct_def

        impl #impl_generics naclac_lang::prelude::EventCpiAccounts for #struct_name #ty_generics #where_clause {
            const NEEDS_EVENT_CPI: bool = #needs_event_cpi;
        }

        #account_helper_impls

        impl #impl_generics naclac_lang::prelude::Bumps for #struct_name #ty_generics #where_clause {
            type BumpsStruct = #bumps_struct_name;
        }

        impl #impl_generics #struct_name #ty_generics #where_clause {
            pub const MUT_MASK: [u64; 4] = #mut_mask_body;
            #pinocchio_fns
            #teardown_fn
        }

        #zc_size_guard_consts
        #borsh_size_guard_consts
        #wincode_size_guard_consts

        #loadable_impls

        #idl_build_accounts_tokens
        #idl_insert_account_types_tokens
    };
    expanded.into()
}
