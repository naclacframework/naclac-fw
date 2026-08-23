//! # Instruction Account Reallocation Logic
//!
//! Generates the dynamic account resizing logic for accounts marked with
//! `#[account(realloc = space, realloc::payer = payer, realloc::zero = true)]`.

use crate::instruction::parser::ParsedField;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

/// Generates account reallocation logic.
///
/// Implements `resize` and handles the necessary System Program CPI to transfer
/// lamports to maintain rent-exemption for the new account size.
pub fn generate_realloc_logic(fields: &[ParsedField]) -> Vec<TokenStream> {
    let mut reallocs = Vec::new();

    for field in fields {
        if let Some(realloc_config) = &field.realloc {
            let target_ident = &field.ident;
            let payer_ident = &realloc_config.payer;
            // The space expression may reference an `#[instruction(...)]` argument,
            // which only exists as a local inside `load_and_validate` — not here in
            // `teardown`. It's pre-evaluated there and threaded through via a
            // dedicated field on the `Bumps` companion struct (see accounts.rs),
            // the same channel bump seeds already use to cross that boundary.
            let space_var = format_ident!("__realloc_space_{}", target_ident);

            // `realloc::zero` is intentionally not threaded through as a
            // separate step: both backends' own `resize()` primitives
            // already unconditionally zero newly-grown bytes themselves
            // (`solana-account-info`'s `sol_memset(&mut data[old_len..], 0, ...)`
            // and pinocchio's `AccountView::resize`'s own `write_bytes(..., 0, ...)`,
            // confirmed by reading both directly). So `zero = true` was
            // always redundant, and there's no safe way to honor
            // `zero = false` as an opt-out short of bypassing `resize()`
            // for an unsafe, unchecked variant purely to skip work the
            // runtime already does for free — not worth the risk for a
            // marginal CU saving.
            let _zero_flag = realloc_config.zero;

            // The rent/CPI logic itself lives in `naclac_lang::prelude::
            // resize_with_rent` (naclac-core/src/realloc.rs) so this
            // constraint and a handler calling that function manually can
            // never drift apart.
            let realloc_body = quote! {
                let new_space = bumps.#space_var;
                naclac_lang::prelude::resize_with_rent(__target, &mut self.#payer_ident, new_space)?;
            };

            // An absent optional target has nothing to resize — skip
            // entirely. A present one goes through exactly the same
            // realloc logic a required field would, via `__target` bound
            // to the inner value.
            if field.is_optional {
                reallocs.push(quote! {
                    if let Some(__target) = self.#target_ident.as_mut() {
                        #realloc_body
                    }
                });
            } else {
                reallocs.push(quote! {
                    {
                        let __target = &mut self.#target_ident;
                        #realloc_body
                    }
                });
            }
        }
    }

    reallocs
}

/// Builds the size-computing block for `realloc::any_of = [Type, ...]`:
/// reads the field's own raw discriminator bytes and looks up the matching
/// type's current compiled size, rejecting any discriminator not in the
/// list. `field_name` must already be bound in scope as the field's raw
/// `AccountInfo` local (true for every field inside `load_and_validate`,
/// where this is spliced — see `accounts.rs`'s per-field `let #field_name
/// = ...` binding).
///
/// `grow_only` (`realloc::grow_only = true`): clamps the result to never go
/// below the account's current length, even if the matched type's compiled
/// size is smaller. `any_of` has no dedicated authority account — any
/// signer may call it against any account it matches — so an unrestricted
/// shrink would let any caller collect the freed rent via `realloc::payer`
/// from an account they don't own, the moment any listed type's compiled
/// size ever decreases. Confirmed this also matches the real, deployed
/// pump.fun program's own `ExtendAccount`: probed live with a deliberately
/// oversized `Global`/`UserVolumeAccumulator`, both left unchanged with its
/// own log line "Account already has more than N bytes" rather than
/// shrinking.
pub fn generate_any_of_space_expr(
    field_name: &syn::Ident,
    types: &[syn::Type],
    idx: usize,
    grow_only: bool,
) -> TokenStream {
    let arms = types.iter().map(|ty| {
        quote! {
            if __disc == <#ty as naclac_lang::prelude::Discriminator>::DISCRIMINATOR {
                8 + core::mem::size_of::<#ty>()
            }
        }
    });

    let matched_size = quote! {
        #(#arms else)* {
            return Err(naclac_lang::prelude::NaclacError::InvalidAccountDiscriminator.err(#idx));
        }
    };

    if grow_only {
        quote! {
            {
                let __data = #field_name.try_borrow_data()?;
                if __data.len() < 8 {
                    return Err(naclac_lang::prelude::NaclacError::AccountDataTooSmall.err(#idx));
                }
                let __disc: [u8; 8] = __data[..8].try_into().unwrap();
                let __current_len = __data.len();
                drop(__data);
                let __matched_size = #matched_size;
                core::cmp::max(__current_len, __matched_size)
            }
        }
    } else {
        quote! {
            {
                let __data = #field_name.try_borrow_data()?;
                if __data.len() < 8 {
                    return Err(naclac_lang::prelude::NaclacError::AccountDataTooSmall.err(#idx));
                }
                let __disc: [u8; 8] = __data[..8].try_into().unwrap();
                drop(__data);
                #matched_size
            }
        }
    }
}
