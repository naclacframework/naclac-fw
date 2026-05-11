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

            // In V2, we are operating on the hydrated struct, so we reference `self`
            let target_info = quote! { self.#target_ident };

            let space_expr = &realloc_config.space;
            let payer_ident = format_ident!("{}", realloc_config.payer);
            let payer_info = quote! { self.#payer_ident };
            let _zero_flag = realloc_config.zero;

            reallocs.push(quote! {
                #[cfg(not(feature = "pinocchio"))]
                {
                    let rent = naclac_lang::solana_program::rent::Rent::get()?;
                    let new_space = (#space_expr) as usize;
                    let current_lamports = **#target_info.lamports.borrow();
                    let required_lamports = rent.minimum_balance(new_space);

                    if required_lamports > current_lamports {
                        let diff = required_lamports.saturating_sub(current_lamports);

                        naclac_lang::solana_program::program::invoke(
                            &naclac_lang::prelude::solana_system_interface::instruction::transfer(
                                #payer_info.key(),
                                #target_info.key(),
                                diff,
                            ),
                            &[
                                #payer_info.clone(),
                                #target_info.clone(),
                            ],
                        )?;
                    }

                    #target_info.resize(new_space)?;
                }
                #[cfg(feature = "pinocchio")]
                {
                    let new_space = (#space_expr) as usize;
                    // In no_std Pinocchio, dynamic rent-exemption checks are typically
                    // skipped or done via custom CPI. Here we just resize.
                    #target_info.resize(new_space)?;
                }
            });
        }
    }

    reallocs
}
