//! `ThawAccount`: unfreezes a token account, signed by the mint's freeze
//! authority.

#[cfg(feature = "pinocchio")]
use {super::invoke::invoke, pinocchio::instruction::InstructionAccount};
#[cfg(not(feature = "pinocchio"))]
use crate::wrappers::ToAddress;
use crate::prelude::{CpiHandle, CpiHandleMut, CpiSigner, Result};

pub fn thaw_account(
    program: CpiHandle<'_>,
    account: CpiHandleMut<'_>,
    mint: CpiHandle<'_>,
    authority: CpiHandle<'_>,
) -> Result<()> {
    thaw_account_signed(program, account, mint, authority, &[])
}

pub fn thaw_account_signed(
    program: CpiHandle<'_>,
    account: CpiHandleMut<'_>,
    mint: CpiHandle<'_>,
    authority: CpiHandle<'_>,
    signer_seeds: &[CpiSigner<'_, '_>],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = if program.address() == spl_token_2022::ID {
            spl_token_2022::instruction::thaw_account(
                &program.address(),
                &account.info.address(),
                &mint.info.address(),
                &authority.info.address(),
                &[],
            )?
        } else {
            spl_token::instruction::thaw_account(
                &program.address(),
                &account.info.address(),
                &mint.info.address(),
                &authority.info.address(),
                &[],
            )?
        };
        let accounts = [CpiHandle::from(account), mint, authority, program];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
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
            &[11],
            signer_seeds,
        )
    }
}
