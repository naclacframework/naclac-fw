//! `Revoke`: removes a token account's delegate, signed by its owner.

#[cfg(not(feature = "pinocchio"))]
use super::invoke::Role;
#[cfg(feature = "pinocchio")]
use pinocchio::instruction::InstructionAccount;
use {
    super::invoke::{invoke, REVOKE},
    crate::prelude::{CpiHandle, CpiHandleMut, CpiSigner, Result},
};

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
        invoke(
            program,
            [
                (CpiHandle::from(source), Role::Writable),
                (authority, Role::ReadonlySigner),
            ],
            &[REVOKE],
            signer_seeds,
        )
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
            &[REVOKE],
            signer_seeds,
        )
    }
}
