//! `TransferChecked`: moves tokens between two token accounts of one mint,
//! checking the mint's decimals.

#[cfg(not(feature = "pinocchio"))]
use super::invoke::Role;
#[cfg(feature = "pinocchio")]
use pinocchio::instruction::InstructionAccount;
use {
    super::invoke::{invoke, TRANSFER_CHECKED},
    crate::prelude::{CpiHandle, CpiHandleMut, CpiSigner, Result},
};

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
    let data = instruction_data(params);

    #[cfg(not(feature = "pinocchio"))]
    {
        invoke(
            program,
            [
                (CpiHandle::from(from), Role::Writable),
                (mint, Role::Readonly),
                (CpiHandle::from(to), Role::Writable),
                (authority, Role::ReadonlySigner),
            ],
            &data,
            signer_seeds,
        )
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
        invoke(
            &program,
            &accounts,
            &[from, mint, to, authority],
            &data,
            signer_seeds,
        )
    }
}

/// `[TRANSFER_CHECKED, amount, decimals]` with the amount little-endian.
#[inline(always)]
fn instruction_data(params: CheckedTransferParams) -> [u8; 10] {
    let CheckedTransferParams { amount, decimals } = params;
    let amount = amount.to_le_bytes();
    [
        TRANSFER_CHECKED,
        amount[0],
        amount[1],
        amount[2],
        amount[3],
        amount[4],
        amount[5],
        amount[6],
        amount[7],
        decimals,
    ]
}

#[cfg(all(test, not(feature = "pinocchio")))]
mod tests {
    use {
        super::*, crate::prelude::Address, spl_token::instruction as classic,
        spl_token_2022::instruction as token_2022,
    };

    #[test]
    fn the_instruction_data_is_what_both_token_programs_build() {
        let key = |seed: u8| Address::new_from_array([seed; 32]);
        let (from, mint, to, authority) = (key(1), key(2), key(3), key(4));
        let (amount, decimals) = (0x0102_0304_0506_0708u64, 9u8);
        let ours = instruction_data(CheckedTransferParams { amount, decimals });
        let classic_data = classic::transfer_checked(
            &spl_token::ID,
            &from,
            &mint,
            &to,
            &authority,
            &[],
            amount,
            decimals,
        )
        .unwrap()
        .data;
        let token_2022_data = token_2022::transfer_checked(
            &spl_token_2022::ID,
            &from,
            &mint,
            &to,
            &authority,
            &[],
            amount,
            decimals,
        )
        .unwrap()
        .data;
        assert!(ours[..] == classic_data[..]);
        assert!(ours[..] == token_2022_data[..]);
    }
}
