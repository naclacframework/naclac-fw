//! # Instruction Account Closure Logic
//!
//! Generates the cleanup logic for accounts marked with `#[account(close = destination)]`.
//! Handles transferring lamports to the destination account, zeroing out the target's data,
//! and reassigning ownership back to the System Program.

use crate::instruction::parser::ParsedField;
use proc_macro2::TokenStream;
use quote::quote;

/// Generates account closure logic for `#[account(close = ...)]` constraints.
///
/// Implements safe lamport transfer and data wiping across both standard SBF
/// (`RefCell` borrowing) and Pinocchio (direct unsafe pointer manipulation) environments.
pub fn generate_close_logic(fields: &[ParsedField]) -> Vec<TokenStream> {
    let mut closes = Vec::new();

    for field in fields {
        if let Some(dest) = &field.close_destination {
            if !fields.iter().any(|f| f.ident == *dest) {
                let error_msg = format!(
                    "Field '{}' not found in struct for `close = {}` on '{}'",
                    dest, dest, field.ident
                );
                closes.push(quote! {
                    core::compile_error!(#error_msg);
                });
                continue;
            }

            let target_ident = &field.ident;
            let target_info =
                quote! { naclac_lang::prelude::ToAccountInfo::to_account_info(__target) };

            let dest_ident = dest;
            let dest_info =
                quote! { naclac_lang::prelude::ToAccountInfo::to_account_info(&self.#dest_ident) };
            let idx = field.index;

            let close_body = quote! {
                if naclac_lang::prelude::ToAddress::address(__target) == naclac_lang::prelude::ToAddress::address(&self.#dest_ident) {
                    return Err(naclac_lang::prelude::NaclacError::ConstraintClose.err(#idx));
                }

                #[cfg(not(feature = "pinocchio"))]
                {
                    let target_info = #target_info;
                    let dest_info = #dest_info;
                    if target_info.owner != program_id {
                        return Err(naclac_lang::prelude::NaclacError::ConstraintOwner.err(#idx));
                    }

                    let dest_starting_lamports = dest_info.lamports();
                    **dest_info.lamports.borrow_mut() = dest_starting_lamports.checked_add(target_info.lamports()).unwrap();
                    **target_info.lamports.borrow_mut() = 0;

                    target_info.assign(&naclac_lang::prelude::SYSTEM_PROGRAM_ID);
                    target_info.data.borrow_mut().fill(0);
                }

                #[cfg(feature = "pinocchio")]
                {
                    let target_info = #target_info;
                    let dest_info = #dest_info;
                    if target_info.owner() != *program_id {
                        return Err(naclac_lang::prelude::NaclacError::ConstraintOwner.err(#idx));
                    }

                    // In Pinocchio, we use safe wrapper methods to transfer lamports.
                    let target_lamports = target_info.lamports();
                    dest_info.add_lamports(target_lamports)?;
                    target_info.sub_lamports(target_lamports)?;

                    let mut target_view = target_info.view;
                    unsafe {
                        target_view.assign(naclac_lang::prelude::SYSTEM_PROGRAM_ID.as_address());
                        core::ptr::write_bytes(target_view.data_ptr() as *mut u8, 0, target_view.data_len());
                    }
                }
            };

            // An absent optional target has nothing to close — skip entirely.
            // A present one goes through exactly the same close logic a
            // required field would, via `__target` bound to the inner value.
            if field.is_optional {
                closes.push(quote! {
                    if let Some(__target) = &self.#target_ident {
                        #close_body
                    }
                });
            } else {
                closes.push(quote! {
                    let __target = &self.#target_ident;
                    #close_body
                });
            }
        }
    }

    closes
}
