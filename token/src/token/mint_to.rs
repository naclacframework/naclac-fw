//! `MintTo`: mints new tokens to a token account, signed by the mint authority.

use crate::prelude::{CpiHandle, CpiHandleMut, CpiSigner, Result};
#[cfg(not(feature = "pinocchio"))]
use crate::wrappers::ToAddress;
#[cfg(feature = "pinocchio")]
use {
    super::invoke::{amount_data, invoke},
    pinocchio::instruction::InstructionAccount,
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
        let ix = if program.address() == spl_token_2022::ID {
            spl_token_2022::instruction::mint_to(
                &program.address(),
                &mint.info.address(),
                &to.info.address(),
                &authority.info.address(),
                &[],
                amount,
            )?
        } else {
            spl_token::instruction::mint_to(
                &program.address(),
                &mint.info.address(),
                &to.info.address(),
                &authority.info.address(),
                &[],
                amount,
            )?
        };
        let accounts = [
            CpiHandle::from(mint),
            CpiHandle::from(to),
            authority,
            program,
        ];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
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
            &amount_data(7, amount),
            signer_seeds,
        )
    }
}
