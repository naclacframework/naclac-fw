//! # Manual Account Reallocation
//!
//! Runtime primitive backing the `#[account(realloc = space, realloc::payer
//! = payer)]` constraint — `naclac-macros`' `instruction::realloc` module
//! generates a call into this same function, so the constraint and manual
//! paths can never diverge. Exposed here for handlers that need to resize
//! an account outside of the accounts-struct constraint system.

use crate::prelude::{Result, ToAccountInfo, ToCpiHandleMut};

#[cfg(not(feature = "pinocchio"))]
use crate::prelude::{Rent, Sysvar};

/// Resizes `target` to `new_space`, funding the rent-exemption shortfall
/// from `payer` via a System Program CPI when growing, and refunding the
/// excess back to `payer` directly when shrinking. A no-op (no lamport
/// movement, no `.resize()` call) when `target` is already `new_space`.
pub fn resize_with_rent<T, P>(target: &mut T, payer: &mut P, new_space: usize) -> Result<()>
where
    T: ToAccountInfo + for<'a> ToCpiHandleMut<'a>,
    P: ToAccountInfo + for<'a> ToCpiHandleMut<'a>,
{
    let target_info = target.to_account_info();

    // Already the requested size: no lamport movement possible (rent is a
    // pure function of size) and nothing for `.resize()` to change either.
    if target_info.try_borrow_data()?.len() == new_space {
        return Ok(());
    }

    let current_lamports = target_info.lamports();

    #[cfg(not(feature = "pinocchio"))]
    let required_lamports = Rent::get()?.minimum_balance(new_space);

    // No `Rent` sysvar under pinocchio; matches the same const-rent formula
    // (`ACCOUNT_STORAGE_OVERHEAD` / `DEFAULT_LAMPORTS_PER_BYTE`) used by
    // `naclac-macros`' `init`/`init_if_needed` codegen.
    #[cfg(feature = "pinocchio")]
    let required_lamports = {
        const STORAGE_OVERHEAD: u64 = 128;
        const LAMPORTS_PER_BYTE: u64 = 6960;
        (STORAGE_OVERHEAD + new_space as u64).wrapping_mul(LAMPORTS_PER_BYTE)
    };

    if required_lamports > current_lamports {
        // Growing: the payer is owned by the System Program, so it must be
        // debited via a signed CPI rather than a direct lamport write.
        let diff = required_lamports.saturating_sub(current_lamports);
        crate::system_program::transfer(
            payer.to_cpi_handle_mut(),
            target.to_cpi_handle_mut(),
            diff,
        )?;
    } else if current_lamports > required_lamports {
        // Shrinking: target is owned by this program, so the excess can be
        // debited directly and credited to payer without a CPI.
        let refund = current_lamports.saturating_sub(required_lamports);
        payer.to_account_info().add_lamports(refund)?;
        target_info.sub_lamports(refund)?;
    }

    target_info.resize(new_space)?;
    Ok(())
}
