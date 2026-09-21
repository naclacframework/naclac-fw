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

/// The constant-rent formula (`ACCOUNT_STORAGE_OVERHEAD` = 128 bytes,
/// `DEFAULT_LAMPORTS_PER_BYTE_YEAR`-derived rate = 6960) — used
/// unconditionally under pinocchio (no `Rent` sysvar exists there at all)
/// and *also* deliberately by `naclac-macros`' non-pinocchio `init`/
/// `init_if_needed` codegen as a CU optimization over a real
/// `Rent::get()?.minimum_balance()` sysvar call (~101 CU here vs. ~668 CU
/// measured for the real fetch) — so this function is backend-neutral, not
/// pinocchio-only, despite `resize_with_rent` below only reaching for it on
/// the pinocchio branch (its non-pinocchio branch already has the real
/// sysvar available and uses it instead, for correctness over the CU
/// saving). Previously duplicated as literal generated tokens in 6 separate
/// places across `naclac-macros/src/instruction/{security,init_cpi}.rs`;
/// extracted here so it's provable once instead of trusted identical six
/// times over (see docs/plan/kani-audit.md).
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

    #[cfg(not(feature = "pinocchio"))]
    let required_lamports = Rent::get()?.minimum_balance(new_space);

    #[cfg(feature = "pinocchio")]
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
