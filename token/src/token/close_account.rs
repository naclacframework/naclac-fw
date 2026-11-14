//! `CloseAccount`: closes a token account, sending its lamports to a
//! destination, signed by the close authority.

use crate::prelude::{CpiHandle, CpiHandleMut, CpiSigner, Result};
#[cfg(not(feature = "pinocchio"))]
use crate::wrappers::ToAddress;
#[cfg(feature = "pinocchio")]
use {super::invoke::invoke, pinocchio::instruction::InstructionAccount};

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
        let ix = if program.address() == spl_token_2022::ID {
            spl_token_2022::instruction::close_account(
                &program.address(),
                &account.info.address(),
                &destination.info.address(),
                &authority.info.address(),
                &[],
            )?
        } else {
            spl_token::instruction::close_account(
                &program.address(),
                &account.info.address(),
                &destination.info.address(),
                &authority.info.address(),
                &[],
            )?
        };
        let accounts = [
            CpiHandle::from(account),
            CpiHandle::from(destination),
            authority,
            program,
        ];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
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
            &[9],
            signer_seeds,
        )
    }
}
