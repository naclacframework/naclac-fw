//! `Revoke`: removes a token account's delegate, signed by its owner.

use crate::prelude::{CpiHandle, CpiHandleMut, CpiSigner, Result};
#[cfg(not(feature = "pinocchio"))]
use crate::wrappers::ToAddress;
#[cfg(feature = "pinocchio")]
use {super::invoke::invoke, pinocchio::instruction::InstructionAccount};

pub fn revoke(
    program: CpiHandle<'_>,
    source: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
) -> Result<()> {
    revoke_signed(program, source, authority, &[])
}

pub fn revoke_signed(
    program: CpiHandle<'_>,
    source: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
    signer_seeds: &[CpiSigner<'_, '_>],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = if program.address() == spl_token_2022::ID {
            spl_token_2022::instruction::revoke(
                &program.address(),
                &source.info.address(),
                &authority.info.address(),
                &[],
            )?
        } else {
            spl_token::instruction::revoke(
                &program.address(),
                &source.info.address(),
                &authority.info.address(),
                &[],
            )?
        };
        let accounts = [CpiHandle::from(source), authority, program];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let source = CpiHandle::from(source);
        let accounts = [
            InstructionAccount::writable(source.info.view.address()),
            InstructionAccount::readonly_signer(authority.info.view.address()),
        ];
        invoke(
            &program,
            &accounts,
            &[source, authority],
            &[5],
            signer_seeds,
        )
    }
}
