//! The two array helpers on an Accounts struct that expose its accounts:
//! `to_account_metas` (off-chain, for building transactions) and
//! `to_account_infos` (on-chain and off-chain, e.g. for manual CPI).

use {
    super::{signer::signer_flag, Shared},
    proc_macro2::TokenStream,
    quote::quote,
};

/// Both helper impls.
///
/// A declared slot is always present in the array: an absent optional account
/// is sentinel-filled with the program's own address (`ID`), never omitted, so
/// the arrays stay fixed-size on every backend with no allocation.
/// `to_account_metas` is gated off the SBF target because it is dead code
/// there and would only grow the `.so`.
pub(super) fn account_helper_impls(shared: &Shared) -> TokenStream {
    let (impl_generics, ty_generics, where_clause) = shared.generics.split_for_impl();
    let struct_name = shared.struct_name;
    let n_fields = shared.fields.len();
    let is_signers: Vec<_> = shared
        .fields
        .iter()
        .map(|f| signer_flag(f, shared.generics, false))
        .collect();
    let is_writables: Vec<_> = shared.fields.iter().map(|f| f.is_mut).collect();

    let account_meta_address_exprs: Vec<_> = shared
        .fields
        .iter()
        .map(|field| {
            let field_name = &field.ident;
            if field.is_optional {
                quote! {
                    match &self.#field_name {
                        Some(__inner) => naclac_lang::prelude::ToAddress::address(__inner),
                        None => crate::ID,
                    }
                }
            } else {
                quote! { (&self.#field_name).address() }
            }
        })
        .collect();

    let account_info_exprs: Vec<_> = shared
        .fields
        .iter()
        .map(|field| {
            let field_name = &field.ident;
            if field.is_optional {
                quote! {
                    match &self.#field_name {
                        Some(__inner) => Some(naclac_lang::prelude::ToAccountInfo::to_account_info(__inner)),
                        None => None,
                    }
                }
            } else {
                quote! { Some(naclac_lang::prelude::ToAccountInfo::to_account_info(&self.#field_name)) }
            }
        })
        .collect();

    quote! {
        #[cfg(not(target_os = "solana"))]
        impl #impl_generics #struct_name #ty_generics #where_clause {
            pub fn to_account_metas(&self) -> [naclac_lang::prelude::AccountMeta; #n_fields] {
                [
                    #(
                        naclac_lang::prelude::AccountMeta {
                            address: #account_meta_address_exprs,
                            is_signer: #is_signers,
                            is_writable: #is_writables,
                        },
                    )*
                ]
            }
        }

        impl #impl_generics #struct_name #ty_generics #where_clause {
            pub fn to_account_infos(&self) -> [Option<naclac_lang::prelude::AccountInfo>; #n_fields] {
                [
                    #(
                        #account_info_exprs,
                    )*
                ]
            }
        }
    }
}
