//! `Burn`: destroys tokens from a token account, signed by its owner or
//! delegate.

#[cfg(feature = "pinocchio")]
use {
    super::invoke::{amount_data, invoke},
    pinocchio::instruction::InstructionAccount,
};
#[cfg(not(feature = "pinocchio"))]
use crate::wrappers::ToAddress;
use crate::prelude::{CpiHandle, CpiHandleMut, CpiSigner, Result};

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
        let ix = if program.address() == spl_token_2022::ID {
            spl_token_2022::instruction::burn(
                &program.address(),
                &from.info.address(),
                &mint.info.address(),
                &authority.info.address(),
                &[],
                amount,
            )?
        } else {
            spl_token::instruction::burn(
                &program.address(),
                &from.info.address(),
                &mint.info.address(),
                &authority.info.address(),
                &[],
                amount,
            )?
        };
        let accounts = [
            CpiHandle::from(from),
            CpiHandle::from(mint),
            authority,
            program,
        ];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
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
            &amount_data(8, amount),
            signer_seeds,
        )
    }
}
