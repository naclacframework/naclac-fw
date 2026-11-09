//! The `LoadableAccounts` impls of the solana runtime (one per layout) and the
//! `MUT_MASK` and `UNIQUE_MASK` constants that mark which declared slots must
//! not be duplicated.

use {
    super::{
        field::parser::ParsedField,
        variant::{Variant, Variants},
        Shared,
    },
    proc_macro2::TokenStream,
    quote::quote,
};

/// A `[u64; 4]` slot mask with a bit per declared field accepted by `selected`.
fn slot_mask_body(shared: &Shared, selected: fn(&ParsedField) -> bool) -> TokenStream {
    let mask_steps: Vec<_> = shared
        .fields
        .iter()
        .enumerate()
        .filter_map(|(i, field)| {
            if selected(field) {
                Some(quote! {
                    __mask = naclac_lang::prelude::mut_mask_set_bit(__mask, #i);
                })
            } else {
                None
            }
        })
        .collect();

    if mask_steps.is_empty() {
        quote! { [0u64; 4] }
    } else {
        quote! {{
            let mut __mask = [0u64; 4];
            #(#mask_steps)*
            __mask
        }}
    }
}

/// The `MUT_MASK` value: a bit per declared slot that is mutable and not an
/// `alias`. No account in such a slot may appear in any other slot.
pub(super) fn mut_mask_body(shared: &Shared) -> TokenStream {
    slot_mask_body(shared, |field| field.is_mut && !field.is_alias)
}

/// The `UNIQUE_MASK` value: a bit per declared slot that is neither `dup` nor
/// an `alias`. Such a slot may not repeat an earlier account.
pub(super) fn unique_mask_body(shared: &Shared) -> TokenStream {
    slot_mask_body(shared, |field| !field.is_dup && !field.is_alias)
}

/// The check that no trailing account repeats a declared mutable one. An
/// optional field is compared only when present. The pinocchio runtime needs no
/// address compare: it serializes a repeated address as a duplicate marker,
/// which the cursor records and the `MUT_MASK` check rejects.
fn remaining_dup_check(shared: &Shared) -> TokenStream {
    let n_declared_accounts = shared.fields.len();
    let items: Vec<TokenStream> = shared
        .fields
        .iter()
        .filter(|f| f.is_mut && !f.is_alias)
        .map(|f| {
            let name = &f.ident;
            let compare = quote! {
                let __rem_addr = naclac_lang::prelude::ToAddress::address(__rem_acc);
                if naclac_lang::prelude::ToAddress::address(__decl) == __rem_addr {
                    return Err(naclac_lang::prelude::NaclacError::ConstraintDuplicateMutableAccount.err(__rem_check_idx));
                }
            };
            if f.is_optional {
                quote! {
                    if let Some(__decl) = &__me.#name {
                        #compare
                    }
                }
            } else {
                quote! {
                    {
                        let __decl = &__me.#name;
                        #compare
                    }
                }
            }
        })
        .collect();
    if items.is_empty() {
        quote! {}
    } else {
        quote! {
            {
                let mut __rem_check_idx = #n_declared_accounts;
                for __rem_acc in remaining_accounts {
                    #(#items)*
                    __rem_check_idx += 1;
                }
            }
        }
    }
}

fn loadable_impl(shared: &Shared, cfg: &TokenStream, variant: &Variant) -> TokenStream {
    let struct_name = shared.struct_name;
    let (_, ty_generics, where_clause) = shared.generics.split_for_impl();
    let n_declared_accounts = shared.fields.len();
    let bumps_struct_name = &shared.bumps.struct_name;
    let bumps_instantiation = &shared.bumps.instantiation;
    let ix_deserializer = &variant.ix_deserializer;
    let field_loaders = &variant.field_loaders;
    let struct_instantiation = shared.struct_instantiation();
    let relational_checks = &shared.relational_checks;
    let remaining_dup_check = remaining_dup_check(shared);
    quote! {
        #cfg
        impl<'a> naclac_lang::prelude::LoadableAccounts<'a> for #struct_name #ty_generics #where_clause {
            type Payload = Self;

            fn load_and_validate<const __NACLAC_IX_DISC_LEN: usize>(
                program_id: &naclac_lang::prelude::Address,
                accounts: &'a [naclac_lang::prelude::AccountType],
                instruction_data: &[u8],
                __duplicates: Option<&naclac_lang::prelude::DuplicateSlots>,
            ) -> naclac_lang::prelude::ValidationResult<'a, Self::Payload, #bumps_struct_name> {
                // Single upfront bounds check.
                if accounts.len() < #n_declared_accounts {
                    return Err(naclac_lang::prelude::NaclacError::NotEnoughAccountKeys.err(0));
                }
                if let Some(__dups) = __duplicates {
                    __dups.check(&Self::MUT_MASK, &Self::UNIQUE_MASK)?;
                }
                let __views = accounts;
                #ix_deserializer
                let __naclac_rent = naclac_lang::prelude::LazyRent::default();
                #(#field_loaders)*
                let remaining_accounts = &accounts[#n_declared_accounts..];
                let mut __me = Self { #(#struct_instantiation),* };
                #remaining_dup_check
                #(#relational_checks)*
                Ok((__me, remaining_accounts, #bumps_instantiation))
            }

            #[inline(always)]
            fn as_mut_payload(payload: &mut Self::Payload) -> &mut Self {
                payload
            }

            #[inline(always)]
            fn teardown_payload(payload: &mut Self::Payload, program_id: &naclac_lang::prelude::Address, bumps: &#bumps_struct_name) -> Result<(), naclac_lang::prelude::ProgramError> {
                payload.teardown(program_id, bumps)
            }
        }
    }
}

/// The three `LoadableAccounts` impls: zero-copy and Wincode are gated to the
/// solana runtime (pinocchio has its own `load_and_validate`), Borsh exists
/// only there.
pub(super) fn loadable_impls(shared: &Shared, variants: &Variants) -> TokenStream {
    let zero_copy = loadable_impl(
        shared,
        &quote! { #[cfg(all(feature = "solana", feature = "bytemuck"))] },
        &variants.zero_copy,
    );
    let borsh = loadable_impl(
        shared,
        &quote! { #[cfg(feature = "borsh")] },
        &variants.borsh,
    );
    let wincode = loadable_impl(
        shared,
        &quote! { #[cfg(all(feature = "solana", feature = "wincode"))] },
        &variants.wincode,
    );
    quote! {
        #zero_copy
        #borsh
        #wincode
    }
}
