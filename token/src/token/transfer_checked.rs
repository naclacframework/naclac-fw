//! `TransferChecked`: moves tokens between two token accounts of one mint,
//! checking the mint's decimals.

use crate::prelude::{CpiHandle, CpiHandleMut, CpiSigner, Result};
#[cfg(not(feature = "pinocchio"))]
use crate::wrappers::ToAddress;
#[cfg(feature = "pinocchio")]
use {super::invoke::invoke, pinocchio::instruction::InstructionAccount};

/// Numeric parameters for a checked token transfer.
/// Grouping amount + decimals keeps `transfer_checked_signed` under clippy's
/// argument-count limit while remaining fully explicit at call sites.
#[derive(Copy, Clone, Debug)]
pub struct CheckedTransferParams {
    pub amount: u64,
    pub decimals: u8,
}

pub fn transfer_checked(
    program: CpiHandle<'_>,
    from: CpiHandleMut<'_>,
    mint: CpiHandle<'_>,
    to: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
    amount: u64,
    decimals: u8,
) -> Result<()> {
    transfer_checked_signed(
        program,
        from,
        mint,
        to,
        authority,
        CheckedTransferParams { amount, decimals },
        &[],
    )
}

pub fn transfer_checked_signed(
    program: CpiHandle<'_>,
    from: CpiHandleMut<'_>,
    mint: CpiHandle<'_>,
    to: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
    params: CheckedTransferParams,
    signer_seeds: &[CpiSigner<'_, '_>],
) -> Result<()> {
    let CheckedTransferParams { amount, decimals } = params;
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = if program.address() == spl_token_2022::ID {
            spl_token_2022::instruction::transfer_checked(
                &program.address(),
                &from.info.address(),
                &mint.info.address(),
                &to.info.address(),
                &authority.info.address(),
                &[],
                amount,
                decimals,
            )?
        } else {
            spl_token::instruction::transfer_checked(
                &program.address(),
                &from.info.address(),
                &mint.info.address(),
                &to.info.address(),
                &authority.info.address(),
                &[],
                amount,
                decimals,
            )?
        };
        let accounts = [
            CpiHandle::from(from),
            mint,
            CpiHandle::from(to),
            authority,
            program,
        ];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let from = CpiHandle::from(from);
        let to = CpiHandle::from(to);
        let accounts = [
            InstructionAccount::writable(from.info.view.address()),
            InstructionAccount::readonly(mint.info.view.address()),
            InstructionAccount::writable(to.info.view.address()),
            InstructionAccount::readonly_signer(authority.info.view.address()),
        ];
        let amount = amount.to_le_bytes();
        let data = [
            12, amount[0], amount[1], amount[2], amount[3], amount[4], amount[5], amount[6],
            amount[7], decimals,
        ];
        invoke(
            &program,
            &accounts,
            &[from, mint, to, authority],
            &data,
            signer_seeds,
        )
    }
}
