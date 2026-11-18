//! `MintTo`: mints new tokens to a token account, signed by the mint authority.

#[cfg(not(feature = "pinocchio"))]
use super::invoke::Role;
#[cfg(feature = "pinocchio")]
use pinocchio::instruction::InstructionAccount;
use {
    super::invoke::{amount_data, invoke, MINT_TO},
    crate::prelude::{CpiHandle, CpiHandleMut, CpiSigner, Result},
};

pub fn mint_to(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    to: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
    amount: u64,
) -> Result<()> {
    mint_to_signed(program, mint, to, authority, amount, &[])
}

pub fn mint_to_signed(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    to: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
    amount: u64,
    signer_seeds: &[CpiSigner<'_, '_>],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        invoke(
            program,
            [
                (CpiHandle::from(mint), Role::Writable),
                (CpiHandle::from(to), Role::Writable),
                (authority, Role::ReadonlySigner),
            ],
            &amount_data(MINT_TO, amount),
            signer_seeds,
        )
    }

    #[cfg(feature = "pinocchio")]
    {
        let mint = CpiHandle::from(mint);
        let to = CpiHandle::from(to);
        let accounts = [
            InstructionAccount::writable(mint.info.view.address()),
            InstructionAccount::writable(to.info.view.address()),
            InstructionAccount::readonly_signer(authority.info.view.address()),
        ];
        invoke(
            &program,
            &accounts,
            &[mint, to, authority],
            &amount_data(MINT_TO, amount),
            signer_seeds,
        )
    }
}
