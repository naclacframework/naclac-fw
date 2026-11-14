//! `Approve`: lets a delegate spend up to an amount from a token account,
//! signed by its owner.

use crate::prelude::{CpiHandle, CpiHandleMut, CpiSigner, Result};
#[cfg(not(feature = "pinocchio"))]
use crate::wrappers::ToAddress;
#[cfg(feature = "pinocchio")]
use {
    super::invoke::{amount_data, invoke},
    pinocchio::instruction::InstructionAccount,
};

pub fn approve(
    program: CpiHandle<'_>,
    to: CpiHandleMut<'_>,
    delegate: CpiHandle<'_>,
    authority: CpiHandle<'_>,
    amount: u64,
) -> Result<()> {
    approve_signed(program, to, delegate, authority, amount, &[])
}

pub fn approve_signed(
    program: CpiHandle<'_>,
    to: CpiHandleMut<'_>,
    delegate: CpiHandle<'_>,
    authority: CpiHandle<'_>,
    amount: u64,
    signer_seeds: &[CpiSigner<'_, '_>],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = if program.address() == spl_token_2022::ID {
            spl_token_2022::instruction::approve(
                &program.address(),
                &to.info.address(),
                &delegate.info.address(),
                &authority.info.address(),
                &[],
                amount,
            )?
        } else {
            spl_token::instruction::approve(
                &program.address(),
                &to.info.address(),
                &delegate.info.address(),
                &authority.info.address(),
                &[],
                amount,
            )?
        };
        let accounts = [CpiHandle::from(to), delegate, authority, program];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let to = CpiHandle::from(to);
        let accounts = [
            InstructionAccount::writable(to.info.view.address()),
            InstructionAccount::readonly(delegate.info.view.address()),
            InstructionAccount::readonly_signer(authority.info.view.address()),
        ];
        invoke(
            &program,
            &accounts,
            &[to, delegate, authority],
            &amount_data(4, amount),
            signer_seeds,
        )
    }
}
