//! `Approve`: lets a delegate spend up to an amount from a token account,
//! signed by its owner.

#[cfg(not(feature = "pinocchio"))]
use super::invoke::Role;
#[cfg(feature = "pinocchio")]
use pinocchio::instruction::InstructionAccount;
use {
    super::invoke::{amount_data, invoke, APPROVE},
    crate::prelude::{CpiHandle, CpiHandleMut, CpiSigner, Result},
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
        invoke(
            program,
            [
                (CpiHandle::from(to), Role::Writable),
                (delegate, Role::Readonly),
                (authority, Role::ReadonlySigner),
            ],
            &amount_data(APPROVE, amount),
            signer_seeds,
        )
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
            &amount_data(APPROVE, amount),
            signer_seeds,
        )
    }
}
