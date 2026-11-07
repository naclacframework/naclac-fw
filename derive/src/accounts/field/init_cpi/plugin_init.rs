//! Creating an account through a namespace that implements
//! `AccountConstraintInit`: the namespace's `owner` and `space` decide what the
//! system program creates, then its `initialize` writes the initial state.

use {
    crate::accounts::field::{checks::plugin_args_tokens, parser::ParsedField},
    proc_macro2::{Ident, TokenStream},
    quote::quote,
};

/// What `generate_init_cpi` needs to create a field's account through a
/// constraint namespace that implements `AccountConstraintInit`.
pub(super) struct PluginInit<'a> {
    pub(super) namespace: TokenStream,
    pub(super) members: &'a [(&'a Ident, &'a syn::Expr)],
    pub(super) payer_ref: &'a TokenStream,
    pub(super) signer_seeds_logic: &'a TokenStream,
    pub(super) system_program_idx: usize,
}

impl PluginInit<'_> {
    /// Creates `field`'s account when it is empty (an `init` field must be
    /// empty, an `init_if_needed` one is left as it is otherwise): the
    /// namespace's `owner` and `space` decide what the system program creates,
    /// funded by the payer and signed with the field's own PDA seeds, then
    /// its `initialize` writes the initial state. Errors from the programs
    /// called keep their own codes; `owner` errors carry the field's index.
    pub(super) fn tokens(&self, field: &ParsedField, all_fields: &[ParsedField]) -> TokenStream {
        let PluginInit {
            namespace,
            members,
            payer_ref,
            signer_seeds_logic,
            system_program_idx,
        } = self;
        let idx = field.index;
        let ty = &field.ty;
        let args = plugin_args_tokens(namespace, members, all_fields);
        let init = quote! {
            <#namespace::Constraint as naclac_lang::prelude::AccountConstraintInit<#ty>>
        };
        let existence_check = if field.is_init_if_needed {
            quote! {}
        } else {
            quote! {
                if !info.data_is_empty() {
                    return Err(naclac_lang::prelude::NaclacError::AccountAlreadyInitialized.err(#idx));
                }
            }
        };
        let create_call_solana = if field.is_init_unchecked {
            quote! { naclac_lang::prelude::system_program::create_account_signed_unchecked }
        } else {
            quote! { naclac_lang::prelude::system_program::create_account_signed }
        };
        let create_call_pinocchio = if field.is_init_unchecked {
            quote! { naclac_lang::prelude::system_program::create_account_unchecked_for_init }
        } else {
            quote! { naclac_lang::prelude::system_program::create_account_checked_for_init }
        };

        quote! {
            #existence_check
            if info.data_is_empty() {
                #signer_seeds_logic
                #[cfg(not(feature = "solana"))]
                let signer_seeds: &[&[&[u8]]] = &[];

                let __sys_info = &accounts[#system_program_idx];
                if (__sys_info).address() != naclac_lang::prelude::SYSTEM_PROGRAM_ID {
                    return Err(naclac_lang::prelude::NaclacError::ProgramIdMismatch.err(#system_program_idx));
                }

                let __init_args = #args;
                let __owner = #init::owner(&__init_args).map_err(|e| e.err(#idx))?;
                let __space = #init::space(&__init_args);
                let __payer_info = naclac_lang::prelude::ToAccountInfo::to_account_info(#payer_ref);

                #[cfg(feature = "solana")]
                {
                    let lamports = __naclac_rent.minimum_balance(__space)?;
                    #create_call_solana(
                        naclac_lang::prelude::CpiHandleMut { info: __payer_info.clone(), _phantom: core::marker::PhantomData },
                        naclac_lang::prelude::CpiHandleMut { info: info.clone(), _phantom: core::marker::PhantomData },
                        naclac_lang::prelude::CpiHandle { info: __sys_info.clone(), _phantom: core::marker::PhantomData },
                        lamports,
                        __space as u64,
                        &__owner,
                        signer_seeds,
                    )?;
                }
                #[cfg(not(feature = "solana"))]
                {
                    let __lamports = __naclac_rent.minimum_balance(__space)?;
                    #create_call_pinocchio(
                        &__payer_info.view,
                        &info.view,
                        __lamports,
                        __space as u64,
                        &__owner,
                        pinocchio_signer_seeds,
                    )?;
                }

                #init::initialize(info, #idx, &__init_args, signer_seeds)?;
            }
        }
    }
}
