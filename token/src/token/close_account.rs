//! `CloseAccount`: closes a token account, sending its lamports to a
//! destination, signed by the close authority.

#[cfg(not(feature = "pinocchio"))]
use super::invoke::Role;
#[cfg(feature = "pinocchio")]
use pinocchio::instruction::InstructionAccount;
use {
    super::invoke::{invoke, CLOSE_ACCOUNT},
    crate::prelude::{CpiHandle, CpiHandleMut, CpiSigner, Result},
};

pub fn close_account(
    program: CpiHandle<'_>,
    account: CpiHandleMut<'_>,
    destination: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
) -> Result<()> {
    close_account_signed(program, account, destination, authority, &[])
}

pub fn close_account_signed(
    program: CpiHandle<'_>,
    account: CpiHandleMut<'_>,
    destination: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
    signer_seeds: &[CpiSigner<'_, '_>],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        invoke(
            program,
            [
                (CpiHandle::from(account), Role::Writable),
                (CpiHandle::from(destination), Role::Writable),
                (authority, Role::ReadonlySigner),
            ],
            &[CLOSE_ACCOUNT],
            signer_seeds,
        )
    }

    #[cfg(feature = "pinocchio")]
    {
        let account = CpiHandle::from(account);
        let destination = CpiHandle::from(destination);
        let accounts = [
            InstructionAccount::writable(account.info.view.address()),
            InstructionAccount::writable(destination.info.view.address()),
            InstructionAccount::readonly_signer(authority.info.view.address()),
        ];
        invoke(
            &program,
            &accounts,
            &[account, destination, authority],
            &[CLOSE_ACCOUNT],
            signer_seeds,
        )
    }
}
