//! # Manual Account Reallocation
//!
//! Runtime primitive backing the `#[account(realloc = space, realloc::payer
//! = payer)]` constraint — `naclac-derive`' `accounts::field::realloc` module
//! generates a call into this same function, so the constraint and manual
//! paths can never diverge. Exposed here for handlers that need to resize
//! an account outside of the accounts-struct constraint system.

use {
    crate::prelude::{Rent, Result, Sysvar, ToAccountInfo, ToCpiHandleMut},
    core::cell::OnceCell,
};

/// Rent-exempt minimum balances from the cluster's `Rent` sysvar, read on
/// first use and reused afterwards, so an instruction that creates several
/// accounts pays for one sysvar read. Fails if the sysvar cannot be read, or
/// with `InvalidArgument` if `space` or the rent parameters are out of range.
#[derive(Default)]
pub struct LazyRent {
    rent: OnceCell<Rent>,
}

impl LazyRent {
    pub fn minimum_balance(&self, space: usize) -> Result<u64> {
        match self.rent.get() {
            Some(rent) => minimum_balance_under(rent, space),
            None => {
                let rent = <Rent as Sysvar>::get()?;
                let balance = minimum_balance_under(&rent, space);
                let _ = self.rent.set(rent);
                balance
            }
        }
    }
}

#[cfg(feature = "pinocchio")]
fn minimum_balance_under(rent: &Rent, space: usize) -> Result<u64> {
    rent.try_minimum_balance(space)
}

#[cfg(not(feature = "pinocchio"))]
fn minimum_balance_under(rent: &Rent, space: usize) -> Result<u64> {
    rent.try_minimum_balance(space)
        .ok_or(crate::prelude::ProgramError::InvalidArgument)
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
    let required_lamports = LazyRent::default().minimum_balance(new_space)?;

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
