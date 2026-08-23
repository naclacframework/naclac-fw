// ===========================================================================
// extensions/memo_transfer.rs — MemoTransfer
// ===========================================================================

//! `MemoTransfer` (token-account extension), plus both CPIs naclac-token
//! supports against it: `enable_required_memo_transfers`/
//! `disable_required_memo_transfers` — confirmed against both anchor-spl-v2's
//! own `token_2022_extensions::memo_transfer` module and the official
//! `anchor-spl` crate's own module of the same name, which expose exactly
//! these two actions (`memo_transfer_initialize`/`memo_transfer_disable`)
//! and nothing else. Unlike every mint-extension initializer, `Enable`
//! implicitly adds the extension if it isn't already present — there's no
//! separate "initialize before `initialize_account`" step the way
//! `ImmutableOwner` has.

use super::Extension;
use crate::prelude::{CpiHandle, CpiHandleMut, Pod, Result, Zeroable};

#[cfg(not(feature = "pinocchio"))]
use crate::prelude::vec;
#[cfg(not(feature = "pinocchio"))]
use crate::wrappers::ToAddress;

/// Raw Token-2022 `MemoTransfer` token-account extension (1 byte): whether
/// inbound transfers to this account must carry a preceding memo. Layout
/// verified against
/// `spl-token-2022-interface::extension::memo_transfer::MemoTransfer`.
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct MemoTransfer(pub [u8; 1]);

unsafe impl Pod for MemoTransfer {}
unsafe impl Zeroable for MemoTransfer {}
impl Extension for MemoTransfer {
    const TYPE: u16 = 8; // ExtensionType::MemoTransfer
    const ACCOUNT_TYPE: u8 = 2; // AccountType::Account
}

impl MemoTransfer {
    /// Raw `bool`-as-`u8` flag: whether inbound transfers require a memo.
    pub fn require_incoming_transfer_memos(&self) -> bool {
        self.0[0] != 0
    }
}

/// Requires inbound transfers into `account` to carry a preceding memo,
/// adding the `MemoTransfer` extension if it isn't already present. Signed
/// by the account's owner. Instruction encoding (top-level discriminant `30`
/// = `MemoTransferExtension`, sub-discriminant `0` = `Enable`, no data
/// payload) verified against `spl-token-2022-interface`'s real
/// `RequiredMemoTransfersInstruction::Enable`/`enable_required_transfer_memos`,
/// cross-checked against `pinocchio-token-2022`'s own
/// `extensions::memo_transfer::Enable::invoke_signed`.
///
/// Doesn't go through `pinocchio-token-2022`'s own `Enable` struct — it's
/// generic over a multisig-signer type, and naclac-token supports only a
/// single fixed authority everywhere else (see `transfer_fee.rs`'s own note
/// on the same tradeoff for its ongoing-action CPIs). The raw
/// `InstructionView` is hand-built instead.
pub fn enable_required_memo_transfers(
    program: CpiHandle<'_>,
    account: CpiHandleMut<'_>,
    owner: CpiHandle<'_>,
) -> Result<()> {
    enable_required_memo_transfers_signed(program, account, owner, &[])
}

pub fn enable_required_memo_transfers_signed(
    program: CpiHandle<'_>,
    account: CpiHandleMut<'_>,
    owner: CpiHandle<'_>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = solana_program::instruction::Instruction {
            program_id: program.address(),
            accounts: vec![
                solana_program::instruction::AccountMeta::new(account.info.address(), false),
                solana_program::instruction::AccountMeta::new_readonly(owner.info.address(), true),
            ],
            data: vec![30u8, 0u8],
        };
        let accounts = [CpiHandle::from(account), owner, program];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let account_handle: CpiHandle<'_> = CpiHandle::from(account);
        let ix_accounts = [
            ::pinocchio::instruction::InstructionAccount::writable(
                account_handle.info.view.address(),
            ),
            ::pinocchio::instruction::InstructionAccount::readonly_signer(
                owner.info.view.address(),
            ),
        ];
        let instruction = ::pinocchio::instruction::InstructionView {
            program_id: program.info.view.address(),
            accounts: &ix_accounts,
            data: &[30u8, 0u8],
        };
        let handles = [account_handle, owner];
        crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles, signer_seeds)
    }
}

/// Stops requiring a memo on inbound transfers into `account`, implicitly
/// initializing the extension if it isn't already present. Signed by the
/// account's owner. Instruction encoding (top-level discriminant `30`,
/// sub-discriminant `1` = `Disable`, no data payload) verified against
/// `spl-token-2022-interface`'s real
/// `RequiredMemoTransfersInstruction::Disable`/`disable_required_transfer_memos`,
/// cross-checked against `pinocchio-token-2022`'s own
/// `extensions::memo_transfer::Disable::invoke_signed`.
pub fn disable_required_memo_transfers(
    program: CpiHandle<'_>,
    account: CpiHandleMut<'_>,
    owner: CpiHandle<'_>,
) -> Result<()> {
    disable_required_memo_transfers_signed(program, account, owner, &[])
}

pub fn disable_required_memo_transfers_signed(
    program: CpiHandle<'_>,
    account: CpiHandleMut<'_>,
    owner: CpiHandle<'_>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = solana_program::instruction::Instruction {
            program_id: program.address(),
            accounts: vec![
                solana_program::instruction::AccountMeta::new(account.info.address(), false),
                solana_program::instruction::AccountMeta::new_readonly(owner.info.address(), true),
            ],
            data: vec![30u8, 1u8],
        };
        let accounts = [CpiHandle::from(account), owner, program];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let account_handle: CpiHandle<'_> = CpiHandle::from(account);
        let ix_accounts = [
            ::pinocchio::instruction::InstructionAccount::writable(
                account_handle.info.view.address(),
            ),
            ::pinocchio::instruction::InstructionAccount::readonly_signer(
                owner.info.view.address(),
            ),
        ];
        let instruction = ::pinocchio::instruction::InstructionView {
            program_id: program.info.view.address(),
            accounts: &ix_accounts,
            data: &[30u8, 1u8],
        };
        let handles = [account_handle, owner];
        crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles, signer_seeds)
    }
}
