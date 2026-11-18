//! `Burn`: destroys tokens from a token account, signed by its owner or
//! delegate.

#[cfg(not(feature = "pinocchio"))]
use super::invoke::Role;
#[cfg(feature = "pinocchio")]
use pinocchio::instruction::InstructionAccount;
use {
    super::invoke::{amount_data, invoke, BURN},
    crate::prelude::{CpiHandle, CpiHandleMut, CpiSigner, Result},
};

pub fn burn(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    from: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
    amount: u64,
) -> Result<()> {
    burn_signed(program, mint, from, authority, amount, &[])
}

pub fn burn_signed(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    from: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
    amount: u64,
    signer_seeds: &[CpiSigner<'_, '_>],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        invoke(
            program,
            [
                (CpiHandle::from(from), Role::Writable),
                (CpiHandle::from(mint), Role::Writable),
                (authority, Role::ReadonlySigner),
            ],
            &amount_data(BURN, amount),
            signer_seeds,
        )
    }

    #[cfg(feature = "pinocchio")]
    {
        let from = CpiHandle::from(from);
        let mint = CpiHandle::from(mint);
        let accounts = [
            InstructionAccount::writable(from.info.view.address()),
            InstructionAccount::writable(mint.info.view.address()),
            InstructionAccount::readonly_signer(authority.info.view.address()),
        ];
        invoke(
            &program,
            &accounts,
            &[from, mint, authority],
            &amount_data(BURN, amount),
            signer_seeds,
        )
    }
}
