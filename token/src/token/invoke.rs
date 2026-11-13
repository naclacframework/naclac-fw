//! The pinocchio-side CPI shared by the instruction files in this folder. Each
//! file builds its instruction accounts and its CPI handles from the same
//! views in the same order, which is what lets this send through
//! naclac-core's borrow-checked invoke without the per-account address
//! comparison.

use {
    crate::prelude::{CpiHandle, CpiSigner, Result},
    pinocchio::instruction::InstructionAccount,
};

/// Sends `data` to `program` with `accounts`, whose entry `i` must be the
/// account of `handles[i]`.
#[inline(always)]
pub(super) fn invoke<const N: usize>(
    program: &CpiHandle<'_>,
    accounts: &[InstructionAccount<'_>; N],
    handles: &[CpiHandle<'_>; N],
    data: &[u8],
    signer_seeds: &[CpiSigner<'_, '_>],
) -> Result<()> {
    crate::cpi::invoke_signed_pinocchio_paired_array(program, accounts, handles, data, signer_seeds)
}

/// `[discriminator, amount]` with the amount little-endian.
#[inline(always)]
pub(super) fn amount_data(discriminator: u8, amount: u64) -> [u8; 9] {
    let amount = amount.to_le_bytes();
    [
        discriminator,
        amount[0],
        amount[1],
        amount[2],
        amount[3],
        amount[4],
        amount[5],
        amount[6],
        amount[7],
    ]
}
