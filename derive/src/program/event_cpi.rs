//! The event-CPI receive side of `#[program]`: the dispatch arm and the
//! `__naclac_receive_event_cpi` function that accept an `emit_cpi!`
//! self-invocation, live only when some handler's accounts struct carries
//! `#[event_cpi]`.

use {
    super::{discriminators::disc_arm_literal, handler::ProgramCtx},
    proc_macro2::TokenStream,
    quote::quote,
    syn::Item,
};

pub(super) struct EventCpi {
    /// The reserved dispatch tag, which also lands in `__NACLAC_EVENT_DISPATCH_TAG`.
    pub(super) event_tag_bytes: Vec<u8>,
    /// Items added to the program module.
    pub(super) items: Vec<Item>,
    /// The `(pattern, body)` dispatch arm for the event tag.
    pub(super) dispatch_arm: (TokenStream, TokenStream),
    /// The `__naclac_receive_event_cpi` functions, at crate-root scope.
    pub(super) receive_fns: TokenStream,
}

/// Builds the event-CPI receive code. It is live only when some handler's
/// accounts struct carries `#[event_cpi]`, decided by a const-evaluated OR over
/// `EventCpiAccounts::NEEDS_EVENT_CPI`. The tag takes part in dispatch like a
/// real discriminator of the same width, so a handler using the same bytes is a
/// compile error in a program that uses `#[event_cpi]`.
pub(super) fn expand(
    ctx: &ProgramCtx,
    event_cpi_ctx_structs: &[syn::Path],
    declared_address: &str,
    declaring_file: &str,
) -> syn::Result<EventCpi> {
    let ix_disc_len = ctx.ix_disc_len;
    let mod_name = ctx.mod_name;
    let ix_disc_by_name = ctx.ix_disc_by_name;
    let mut items: Vec<Item> = Vec::new();

    let event_tag_bytes = naclac_syn::discriminator::event_dispatch_tag(ix_disc_len);
    let colliding_handlers: Vec<&String> = ix_disc_by_name
        .iter()
        .filter(|(_, bytes)| **bytes == event_tag_bytes)
        .map(|(name, _)| name)
        .collect();
    items.push(syn::parse_quote! {
        pub(super) const __NACLAC_NEEDS_EVENT_CPI: bool =
            false #(|| <#event_cpi_ctx_structs as naclac_lang::prelude::EventCpiAccounts>::NEEDS_EVENT_CPI)*;
    });
    let mut tagged_ctx_structs: Vec<&syn::Path> = Vec::new();
    for ctx_struct in event_cpi_ctx_structs {
        let already_tagged = tagged_ctx_structs
            .iter()
            .any(|seen| quote!(#seen).to_string() == quote!(#ctx_struct).to_string());
        if !already_tagged {
            tagged_ctx_structs.push(ctx_struct);
        }
    }
    for ctx_struct in tagged_ctx_structs {
        items.push(syn::parse_quote! {
            impl naclac_lang::prelude::EventCpiTag for #ctx_struct {
                const EVENT_DISPATCH_TAG: &'static [u8] = &[#(#event_tag_bytes),*];
            }
        });
    }
    if !colliding_handlers.is_empty() {
        let names = colliding_handlers
            .iter()
            .map(|n| n.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let msg = format!(
            "Naclac Error: instruction(s) [{names}] use a discriminator equal to the reserved \
             event-CPI dispatch tag ({event_tag_bytes:?}), and this program uses `#[event_cpi]`. \
             Give the instruction a different `#[discriminator = ...]`."
        );
        items.push(syn::parse_quote! {
            const _: () = {
                if __NACLAC_NEEDS_EVENT_CPI {
                    panic!(#msg);
                }
            };
        });
    }

    let event_authority_seed = b"__event_authority".to_vec();
    let Some((event_authority_pda, _bump)) =
        crate::accounts::field::compile_time::find_pda_at_compile_time(
            std::slice::from_ref(&event_authority_seed),
            declared_address,
        )
    else {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            format!(
                "Naclac Error: the address `declare_id!` declares in {declaring_file} is not a \
                 valid 32-byte base58 address"
            ),
        ));
    };
    let event_authority_bytes = bs58::decode(event_authority_pda).into_vec().map_err(|_| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            "Naclac Error: the event authority address is not valid base58.",
        )
    })?;
    let expected_authority = quote! {
        naclac_lang::prelude::Address::new_from_array([ #(#event_authority_bytes),* ])
    };
    let track_program_id = quote! { const _: &[u8] = include_bytes!(#declaring_file); };
    let event_tag_literal = disc_arm_literal(ix_disc_len, &event_tag_bytes);
    let dispatch_arm = (
        quote! { #event_tag_literal if #mod_name::__NACLAC_NEEDS_EVENT_CPI },
        quote! {
            #[cfg(not(feature = "solana"))]
            { __naclac_receive_event_cpi(input, num_accounts) }
            #[cfg(feature = "solana")]
            { __naclac_receive_event_cpi(accounts) }
        },
    );
    let event_cpi_receive_fns = quote! {
        /// Handles an `emit_cpi!` self-invocation: succeeds only when the
        /// first account is the program's event authority PDA and signed.
        #[cfg(not(feature = "solana"))]
        #[inline(always)]
        fn __naclac_receive_event_cpi(input: *mut u8, num_accounts: usize) -> naclac_lang::prelude::ProgramResult {
            #track_program_id
            {
                if num_accounts == 0 {
                    return Err(naclac_lang::prelude::NaclacError::NotEnoughAccountKeys.err(0));
                }
                // SAFETY: at least one account is present, it starts right after
                // the 8-byte account count, and the first serialized account is
                // never a duplicate entry.
                let __ea = unsafe {
                    naclac_lang::prelude::AccountView::new_unchecked(
                        input.add(8) as *mut naclac_lang::pinocchio::account::RuntimeAccount,
                    )
                };
                if !__ea.is_signer() {
                    return Err(naclac_lang::prelude::NaclacError::ConstraintSigner.err(0));
                }
                if naclac_lang::prelude::Address::from_address_ref(__ea.address()) != &#expected_authority {
                    return Err(naclac_lang::prelude::NaclacError::ConstraintSeeds.err(0));
                }
                Ok(())
            }
        }

        /// Handles an `emit_cpi!` self-invocation: succeeds only when the
        /// first account is the program's event authority PDA and signed.
        #[cfg(feature = "solana")]
        #[inline(always)]
        fn __naclac_receive_event_cpi(accounts: &[naclac_lang::prelude::AccountType]) -> naclac_lang::prelude::Result {
            #track_program_id
            {
                let __ea_slice: &[naclac_lang::prelude::AccountType] = accounts;
                let Some(__ea) = __ea_slice.first() else {
                    return Err(naclac_lang::prelude::NaclacError::NotEnoughAccountKeys.err(0));
                };
                if !__ea.is_signer {
                    return Err(naclac_lang::prelude::NaclacError::ConstraintSigner.err(0));
                }
                if __ea.address() != #expected_authority {
                    return Err(naclac_lang::prelude::NaclacError::ConstraintSeeds.err(0));
                }
                Ok(())
            }
        }
    };

    Ok(EventCpi {
        event_tag_bytes,
        items,
        dispatch_arm,
        receive_fns: event_cpi_receive_fns,
    })
}
