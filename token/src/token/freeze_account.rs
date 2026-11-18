//! `FreezeAccount`: freezes a token account, signed by the mint's freeze
//! authority.

#[cfg(not(feature = "pinocchio"))]
use super::invoke::Role;
#[cfg(feature = "pinocchio")]
use pinocchio::instruction::InstructionAccount;
use {
    super::invoke::{invoke, FREEZE_ACCOUNT},
    crate::prelude::{CpiHandle, CpiHandleMut, CpiSigner, Result},
};

pub fn freeze_account(
    program: CpiHandle<'_>,
    account: CpiHandleMut<'_>,
    mint: CpiHandle<'_>,
    authority: CpiHandle<'_>,
) -> Result<()> {
    freeze_account_signed(program, account, mint, authority, &[])
}

pub fn freeze_account_signed(
    program: CpiHandle<'_>,
    account: CpiHandleMut<'_>,
    mint: CpiHandle<'_>,
    authority: CpiHandle<'_>,
    signer_seeds: &[CpiSigner<'_, '_>],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        invoke(
            program,
            [
                (CpiHandle::from(account), Role::Writable),
                (mint, Role::Readonly),
                (authority, Role::ReadonlySigner),
            ],
            &[FREEZE_ACCOUNT],
            signer_seeds,
        )
    }

    #[cfg(feature = "pinocchio")]
    {
        let account = CpiHandle::from(account);
        let accounts = [
            InstructionAccount::writable(account.info.view.address()),
            InstructionAccount::readonly(mint.info.view.address()),
            InstructionAccount::readonly_signer(authority.info.view.address()),
        ];
        invoke(
            &program,
            &accounts,
            &[account, mint, authority],
            &[FREEZE_ACCOUNT],
            signer_seeds,
        )
    }
}
