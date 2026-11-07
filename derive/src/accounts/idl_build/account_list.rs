//! `impl <Accounts struct> { fn __naclac_idl_accounts() }`: the per-field
//! account metadata (writable, signer, PDA, address) an Accounts struct can
//! resolve on its own at expansion time.

use {crate::accounts::field::parser, quote::quote};

/// Builds `impl #struct_name { pub fn __naclac_idl_accounts() -> Vec<IdlAccountBuild> }`.
///
/// An account seed's subfield type and a spliced-constant seed's value are
/// left for later resolution, by `#[program]`'s assembler and by the calling
/// crate's compiler at `idl-build` runtime respectively. Emits nothing
/// without the `idl-build` feature.
#[cfg(feature = "idl-build")]
pub(crate) fn idl_build_accounts_impl(
    struct_name: &syn::Ident,
    parsed_fields: &[parser::ParsedField],
    fields_named: &syn::FieldsNamed,
    instr_names: &[syn::Ident],
) -> proc_macro2::TokenStream {
    use super::seeds::{build_idl_seed_expr, mentions_any_name, splice_const_fallback};

    let account_names: Vec<String> = parsed_fields.iter().map(|f| f.ident.to_string()).collect();
    let valueless_names: Vec<String> = account_names
        .iter()
        .cloned()
        .chain(instr_names.iter().map(ToString::to_string))
        .collect();

    let mut account_entries = Vec::new();
    for (field, syn_field) in parsed_fields.iter().zip(fields_named.named.iter()) {
        let name = naclac_syn::parser::idl_name(&field.ident.to_string());
        let docs = naclac_syn::parser::extract_docs(&syn_field.attrs);
        let is_writable = field.is_mut;
        let is_signer =
            crate::accounts::signer::signer_flag(field, &syn::Generics::default(), false);
        let is_optional = field.is_optional;

        let pda_tokens = if let Some(seed_array) = &field.pda_seed {
            let seed_tokens: Vec<_> = seed_array
                .elems
                .iter()
                .map(|e| build_idl_seed_expr(e, &account_names, instr_names, parsed_fields))
                .collect();
            let program_tokens = match &field.pda_program {
                Some(prog_expr) => {
                    let t =
                        build_idl_seed_expr(prog_expr, &account_names, instr_names, parsed_fields);
                    quote! { Some(#t) }
                }
                None => quote! { None },
            };
            quote! {
                Some(naclac_lang::naclac_idl::idl_build_accounts::IdlPdaBuild {
                    seeds: vec![#(#seed_tokens),*],
                    program: #program_tokens,
                })
            }
        } else {
            match crate::accounts::field::builtin_namespaces::idl_pda(field, parsed_fields) {
                Err(error) => return error,
                Ok(None) => quote! { None },
                Ok(Some((seeds, program))) => {
                    let seed_tokens: Vec<_> = seeds
                        .iter()
                        .map(|e| build_idl_seed_expr(e, &account_names, instr_names, parsed_fields))
                        .collect();
                    let program_tokens = splice_const_fallback(&program, None);
                    quote! {
                        Some(naclac_lang::naclac_idl::idl_build_accounts::IdlPdaBuild {
                            seeds: vec![#(#seed_tokens),*],
                            program: Some(#program_tokens),
                        })
                    }
                }
            }
        };

        let address_tokens = match &field.address {
            Some(addr_expr) if !mentions_any_name(addr_expr, &valueless_names) => quote! {
                Some(naclac_lang::prelude::base58::encode_address_to_string(
                    &naclac_lang::prelude::ToAddress::address(&(#addr_expr)),
                ))
            },
            _ => quote! { None },
        };

        account_entries.push(quote! {
            naclac_lang::naclac_idl::idl_build_accounts::IdlAccountBuild {
                name: #name.into(),
                docs: vec![#(#docs.into()),*],
                writable: #is_writable,
                signer: #is_signer,
                optional: #is_optional,
                pda: #pda_tokens,
                address: #address_tokens,
            }
        });
    }

    quote! {
        #[cfg(feature = "idl-build")]
        impl #struct_name {
            pub fn __naclac_idl_accounts() -> naclac_lang::prelude::Vec<naclac_lang::naclac_idl::idl_build_accounts::IdlAccountBuild> {
                vec![#(#account_entries),*]
            }
        }
    }
}

#[cfg(not(feature = "idl-build"))]
pub(crate) fn idl_build_accounts_impl(
    _struct_name: &syn::Ident,
    _parsed_fields: &[parser::ParsedField],
    _fields_named: &syn::FieldsNamed,
    _instr_names: &[syn::Ident],
) -> proc_macro2::TokenStream {
    quote! {}
}
