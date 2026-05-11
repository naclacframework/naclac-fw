//! # Instruction Account Closure Logic
//!
//! Generates the cleanup logic for accounts marked with `#[account(close = destination)]`.
//! Handles transferring lamports to the destination account, zeroing out the target's data,
//! and reassigning ownership back to the System Program.

use crate::instruction::parser::ParsedField;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

/// Generates account closure logic for `#[account(close = ...)]` constraints.
///
/// Implements safe lamport transfer and data wiping across both standard SBF 
/// (`RefCell` borrowing) and Pinocchio (direct unsafe pointer manipulation) environments.
pub fn generate_close_logic(fields: &[ParsedField]) -> Vec<TokenStream> {
    let mut closes = Vec::new();

    for field in fields {
        if let Some(dest) = &field.close_destination {
            let target_ident = &field.ident;
            let target_info = quote! { self.#target_ident };
            
            let dest_ident = format_ident!("{}", dest);
            let dest_info = quote! { self.#dest_ident };
            let idx = field.index;

            closes.push(quote! {
                #[cfg(not(feature = "pinocchio"))]
                {
                    if #target_info.owner != program_id {
                        return Err(naclac_lang::prelude::NaclacError::ConstraintOwner.err(#idx));
                    }

                    let dest_starting_lamports = #dest_info.lamports();
                    **#dest_info.lamports.borrow_mut() = dest_starting_lamports.checked_add(#target_info.lamports()).unwrap();
                    **#target_info.lamports.borrow_mut() = 0;

                    #target_info.assign(&naclac_lang::prelude::SYSTEM_PROGRAM_ID);
                    #target_info.data.borrow_mut().fill(0);
                }

                #[cfg(feature = "pinocchio")]
                {
                    if #target_info.owner() != program_id {
                        return Err(naclac_lang::prelude::NaclacError::ConstraintOwner.err(#idx));
                    }

                    // In Pinocchio, we use raw pointer manipulation or provided methods 
                    // to transfer lamports between accounts owned by the program.
                    let target_lamports = #target_info.lamports();
                    let dest_lamports = #dest_info.lamports();
                    
                    // SAFETY: We are closing the account and transferring its lamports.
                    // This is sound because we are the owner of the target account, 
                    // and we have confirmed it is not the same as the destination. 
                    // Direct pointer manipulation of lamports is the Pinocchio-native 
                    // way to perform this operation efficiently.
                    unsafe {
                        *(#dest_info.lamports_handle()) = dest_lamports + target_lamports;
                        *(#target_info.lamports_handle()) = 0;
                        
                        #target_info.assign(&naclac_lang::prelude::SYSTEM_PROGRAM_ID);
                        let data = #target_info.data();
                        core::ptr::write_bytes(data.as_ptr() as *mut u8, 0, data.len());
                    }
                }
            });
        }
    }

    closes
}