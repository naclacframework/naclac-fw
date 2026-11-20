//! The CPI shared by the instruction files in this folder, with one body per
//! runtime. Each file lists its instruction accounts and its CPI handles in the
//! same order; on pinocchio that is what lets the send go through
//! naclac-core's borrow-checked invoke without the per-account address
//! comparison, and on Solana it is the role each handle plays in the
//! instruction. The instruction data is encoded by hand on both runtimes, from
//! the discriminators below.
//!
//! Nothing here checks that `program` is the Token or Token-2022 program: the
//! `Program<Token>`, `Program<Token2022>` and `Interface<TokenInterface>`
//! wrappers verify it once when the accounts load, and every `TokenCpi` call
//! takes its program from one of them. Callers of the free functions that pass
//! any other handle must verify it themselves.

#[cfg(not(feature = "pinocchio"))]
use crate::{
    cpi::{invoke_signed_paired, CpiAccount},
    prelude::{CpiHandle, CpiSigner, Result},
};
#[cfg(feature = "pinocchio")]
use {
    crate::prelude::{CpiHandle, CpiSigner, Result},
    pinocchio::instruction::InstructionAccount,
};

pub(super) const APPROVE: u8 = 4;
pub(super) const REVOKE: u8 = 5;
pub(super) const SET_AUTHORITY: u8 = 6;
pub(super) const MINT_TO: u8 = 7;
pub(super) const BURN: u8 = 8;
pub(super) const CLOSE_ACCOUNT: u8 = 9;
pub(super) const FREEZE_ACCOUNT: u8 = 10;
pub(super) const THAW_ACCOUNT: u8 = 11;
pub(super) const TRANSFER_CHECKED: u8 = 12;
pub(super) const SYNC_NATIVE: u8 = 17;
pub(super) const INITIALIZE_ACCOUNT3: u8 = 18;
pub(super) const INITIALIZE_MINT2: u8 = 20;

/// Sends `data` to `program` with `accounts`, whose entry `i` must be the
/// account of `handles[i]`.
#[cfg(feature = "pinocchio")]
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

/// How an account takes part in a token instruction on the Solana runtime.
/// `ReadonlySigner` is the authority of a single-owner (non-multisig) call.
#[cfg(not(feature = "pinocchio"))]
#[derive(Clone, Copy)]
pub(super) enum Role {
    Writable,
    Readonly,
    ReadonlySigner,
}

#[cfg(not(feature = "pinocchio"))]
impl Role {
    /// `(is_signer, is_writable)`.
    fn flags(self) -> (bool, bool) {
        match self {
            Role::Writable => (false, true),
            Role::Readonly => (false, false),
            Role::ReadonlySigner => (true, false),
        }
    }

    #[inline(always)]
    fn account(self, handle: CpiHandle<'_>) -> CpiAccount<'_> {
        let (is_signer, is_writable) = self.flags();
        CpiAccount {
            handle,
            is_signer,
            is_writable,
        }
    }
}

/// Sends `data` to `program` with `accounts` in the order and roles given.
#[cfg(not(feature = "pinocchio"))]
#[inline(always)]
pub(super) fn invoke<const N: usize>(
    program: CpiHandle<'_>,
    accounts: [(CpiHandle<'_>, Role); N],
    data: &[u8],
    signer_seeds: &[CpiSigner<'_, '_>],
) -> Result<()> {
    let accounts = accounts.map(|(handle, role)| role.account(handle));
    invoke_signed_paired(program, &accounts, data, signer_seeds)
}

#[cfg(all(test, not(feature = "pinocchio")))]
mod tests {
    use {
        super::*,
        crate::prelude::Address,
        spl_token_2022_interface::{instruction as token_2022, ID as TOKEN_2022_ID},
        spl_token_interface::{instruction as classic, ID as CLASSIC_ID},
    };

    fn key(seed: u8) -> Address {
        Address::new_from_array([seed; 32])
    }

    #[test]
    fn each_discriminator_is_the_one_the_token_program_packs() {
        let (a, b, c) = (key(1), key(2), key(3));
        let id = CLASSIC_ID;
        let cases: [(u8, u8); 12] = [
            (
                INITIALIZE_ACCOUNT3,
                classic::initialize_account3(&id, &a, &b, &c).unwrap().data[0],
            ),
            (
                INITIALIZE_MINT2,
                classic::initialize_mint2(&id, &a, &b, None, 6)
                    .unwrap()
                    .data[0],
            ),
            (
                APPROVE,
                classic::approve(&id, &a, &b, &c, &[], 1).unwrap().data[0],
            ),
            (REVOKE, classic::revoke(&id, &a, &c, &[]).unwrap().data[0]),
            (
                MINT_TO,
                classic::mint_to(&id, &a, &b, &c, &[], 1).unwrap().data[0],
            ),
            (
                BURN,
                classic::burn(&id, &a, &b, &c, &[], 1).unwrap().data[0],
            ),
            (
                CLOSE_ACCOUNT,
                classic::close_account(&id, &a, &b, &c, &[]).unwrap().data[0],
            ),
            (
                FREEZE_ACCOUNT,
                classic::freeze_account(&id, &a, &b, &c, &[]).unwrap().data[0],
            ),
            (
                THAW_ACCOUNT,
                classic::thaw_account(&id, &a, &b, &c, &[]).unwrap().data[0],
            ),
            (
                TRANSFER_CHECKED,
                classic::transfer_checked(&id, &a, &b, &c, &c, &[], 1, 2)
                    .unwrap()
                    .data[0],
            ),
            (SYNC_NATIVE, classic::sync_native(&id, &a).unwrap().data[0]),
            (
                SET_AUTHORITY,
                classic::set_authority(&id, &a, None, classic::AuthorityType::MintTokens, &c, &[])
                    .unwrap()
                    .data[0],
            ),
        ];
        for (ours, theirs) in cases {
            assert!(ours == theirs);
        }
    }

    #[test]
    fn an_amount_is_the_discriminator_then_eight_little_endian_bytes() {
        let (a, b, c) = (key(1), key(2), key(3));
        let amount = 0x0102_0304_0506_0708u64;
        let id = CLASSIC_ID;
        let id_2022 = TOKEN_2022_ID;
        assert!(
            amount_data(APPROVE, amount)[..]
                == classic::approve(&id, &a, &b, &c, &[], amount).unwrap().data[..]
        );
        assert!(
            amount_data(MINT_TO, amount)[..]
                == classic::mint_to(&id, &a, &b, &c, &[], amount).unwrap().data[..]
        );
        assert!(
            amount_data(BURN, amount)[..]
                == token_2022::burn(&id_2022, &a, &b, &c, &[], amount)
                    .unwrap()
                    .data[..]
        );
    }

    #[test]
    fn the_roles_are_the_account_metas_the_token_program_builds() {
        let (from, mint, to, authority) = (key(1), key(2), key(3), key(4));
        let built =
            classic::transfer_checked(&CLASSIC_ID, &from, &mint, &to, &authority, &[], 5, 6)
                .unwrap();
        let ours = [
            (from, Role::Writable),
            (mint, Role::Readonly),
            (to, Role::Writable),
            (authority, Role::ReadonlySigner),
        ];
        assert!(built.accounts.len() == ours.len());
        for (meta, (address, role)) in built.accounts.iter().zip(ours) {
            assert!(meta.pubkey == address);
            assert!(role.flags() == (meta.is_signer, meta.is_writable));
        }
    }
}
