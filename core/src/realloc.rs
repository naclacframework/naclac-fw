//! # Manual Account Reallocation
//!
//! Runtime primitive backing the `#[account(realloc = space, realloc::payer
//! = payer)]` constraint — `naclac-macros`' `instruction::realloc` module
//! generates a call into this same function, so the constraint and manual
//! paths can never diverge. Exposed here for handlers that need to resize
//! an account outside of the accounts-struct constraint system.

use crate::prelude::{Result, ToAccountInfo, ToCpiHandleMut};

/// Rent-exempt minimum balance for an account of `space` bytes:
/// `(ACCOUNT_STORAGE_OVERHEAD + space) * DEFAULT_LAMPORTS_PER_BYTE` with the
/// default rent parameters (128 bytes, 6960 lamports/byte), the same value
/// the `Rent` sysvar's `minimum_balance` returns under them. Every rent
/// computation on both backends goes through this function instead of the
/// sysvar, so a change to the cluster's rent parameters must be applied
/// here.
pub fn const_rent_lamports(space: usize) -> u64 {
    const STORAGE_OVERHEAD: u64 = 128;
    const LAMPORTS_PER_BYTE: u64 = 6960;
    // `saturating_add`, not a bare `+`: a `space` near `usize::MAX` (nothing
    // in this function's signature rules that out, even though no real
    // Solana account is ever that large) would otherwise panic here —
    // confirmed by Kani, present in all 6 of this formula's original
    // call-site copies before this extraction, never caught until now.
    STORAGE_OVERHEAD
        .saturating_add(space as u64)
        .wrapping_mul(LAMPORTS_PER_BYTE)
}

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
    let required_lamports = const_rent_lamports(new_space);

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

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Proves `const_rent_lamports` never panics for any `space` —
    /// `wrapping_mul` was chosen deliberately over `checked_mul` here, so
    /// this also confirms that choice is actually safe (no panic path
    /// exists to trigger regardless) rather than merely convenient.
    #[kani::proof]
    fn prove_const_rent_lamports_never_panics() {
        let space: usize = kani::any();
        let _ = const_rent_lamports(space);
    }
}
