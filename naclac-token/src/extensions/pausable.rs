// ===========================================================================
// extensions/pausable.rs — PausableConfig / PausableAccount
// ===========================================================================

//! `PausableConfig` (mint extension) plus every CPI naclac-token supports
//! against it: `initialize_pausable_config`/`pause_mint`/`resume_mint` — no
//! official `anchor-spl` module exists for this extension at all (confirmed
//! by a crate-wide search); the local pinocchio-native `anchor-spl-v2`'s
//! own `token_2022_extensions::pausable` module exposes exactly these three
//! actions, used here as the parity target. Also `PausableAccount` — a
//! zero-byte marker Token-2022 attaches automatically to every token
//! account of a pausable mint, read-only like `TransferHookAccount`/
//! `NonTransferableAccount`, since the protocol itself creates it rather
//! than any dedicated init instruction.
//!
//! Unlike `CpiGuard`, `Pause`/`Resume` have no CPI-depth restriction in the
//! real processor (verified directly — `pausable::processor::process_toggle_pause`
//! has no `in_cpi()`-style check), so these are ordinary CPI helpers.

use super::Extension;
use crate::prelude::{Address, CpiHandle, CpiHandleMut, Pod, Result, Zeroable};

#[cfg(not(feature = "pinocchio"))]
use crate::prelude::vec;
#[cfg(not(feature = "pinocchio"))]
use crate::wrappers::ToAddress;

/// Raw Token-2022 `PausableConfig` mint extension (33 bytes): the authority
/// that can pause/resume the mint, plus whether it's currently paused.
/// Layout verified against
/// `spl-token-2022-interface::extension::pausable::PausableConfig`'s real
/// field order/sizes.
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct PausableConfig(pub [u8; 33]);

unsafe impl Pod for PausableConfig {}
unsafe impl Zeroable for PausableConfig {}
impl Extension for PausableConfig {
    const TYPE: u16 = 26; // ExtensionType::Pausable
    const ACCOUNT_TYPE: u8 = 1; // AccountType::Mint
}

impl PausableConfig {
    pub fn authority(&self) -> Option<Address> {
        super::read_optional_address(&self.0, 0)
    }
    pub fn paused(&self) -> bool {
        self.0[32] != 0
    }
}

/// Zero-byte marker Token-2022 attaches automatically to every token
/// account of a `PausableConfig` mint — no dedicated init instruction, same
/// shape as `TransferHookAccount`/`NonTransferableAccount`.
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct PausableAccount;

unsafe impl Pod for PausableAccount {}
unsafe impl Zeroable for PausableAccount {}
impl Extension for PausableAccount {
    const TYPE: u16 = 27; // ExtensionType::PausableAccount
    const ACCOUNT_TYPE: u8 = 2; // AccountType::Account
}

/// Initializes the `PausableConfig` extension on a not-yet-initialized
/// mint. Must be called before `initialize_mint`/`initialize_mint_signed`,
/// same as the other extension initializers. Instruction encoding
/// (top-level discriminant `44` = `PausableExtension`, sub-discriminant `0`
/// = `Initialize`, then `authority` as a fixed, non-optional 32 bytes —
/// unlike most other extension initializers, the real
/// `InitializeInstructionData` here has no `None` encoding) verified
/// against `spl-token-2022-interface`'s real
/// `pausable::instruction::{PausableInstruction::Initialize, initialize}`,
/// cross-checked against `pinocchio-token-2022`'s own
/// `extensions::pausable::Initialize::invoke`.
pub fn initialize_pausable_config(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    authority: &Address,
) -> Result<()> {
    initialize_pausable_config_signed(program, mint, authority, &[])
}

pub fn initialize_pausable_config_signed(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    authority: &Address,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = vec![44u8, 0u8];
        data.extend_from_slice(authority.as_ref());

        let ix = solana_program::instruction::Instruction {
            program_id: program.address(),
            accounts: vec![solana_program::instruction::AccountMeta::new(
                mint.info.address(),
                false,
            )],
            data,
        };
        let accounts = [CpiHandle::from(mint), program];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let ix = ::pinocchio_token_2022::instructions::pausable::Initialize {
            mint: &mint.info.view,
            authority: authority.as_address(),
            token_program: &program.info.view.address(),
        };
        let _ = signer_seeds;
        ix.invoke()
    }
}

/// Pauses minting/burning/transferring on the mint. Signed by the mint's
/// pause authority. Instruction encoding (top-level discriminant `44`,
/// sub-discriminant `1` = `Pause`, no data payload) verified against
/// `spl-token-2022-interface`'s real
/// `PausableInstruction::Pause`/`pause`, cross-checked against
/// `pinocchio-token-2022`'s own `extensions::pausable::Pause::invoke_signed`.
///
/// Doesn't go through `pinocchio-token-2022`'s own `Pause` struct — it's
/// generic over a multisig-signer type, and naclac-token supports only a
/// single fixed authority everywhere else (see `transfer_fee.rs`'s own note
/// on the same tradeoff for its ongoing-action CPIs). The raw
/// `InstructionView` is hand-built instead.
pub fn pause_mint(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
) -> Result<()> {
    pause_mint_signed(program, mint, authority, &[])
}

pub fn pause_mint_signed(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = solana_program::instruction::Instruction {
            program_id: program.address(),
            accounts: vec![
                solana_program::instruction::AccountMeta::new(mint.info.address(), false),
                solana_program::instruction::AccountMeta::new_readonly(
                    authority.info.address(),
                    true,
                ),
            ],
            data: vec![44u8, 1u8],
        };
        let accounts = [CpiHandle::from(mint), authority, program];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mint_handle: CpiHandle<'_> = CpiHandle::from(mint);
        let ix_accounts = [
            ::pinocchio::instruction::InstructionAccount::writable(mint_handle.info.view.address()),
            ::pinocchio::instruction::InstructionAccount::readonly_signer(
                authority.info.view.address(),
            ),
        ];
        let instruction = ::pinocchio::instruction::InstructionView {
            program_id: program.info.view.address(),
            accounts: &ix_accounts,
            data: &[44u8, 1u8],
        };
        let handles = [mint_handle, authority];
        crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles, signer_seeds)
    }
}

/// Resumes minting/burning/transferring on the mint. Signed by the mint's
/// pause authority. Instruction encoding (top-level discriminant `44`,
/// sub-discriminant `2` = `Resume`, no data payload) verified against
/// `spl-token-2022-interface`'s real
/// `PausableInstruction::Resume`/`resume`, cross-checked against
/// `pinocchio-token-2022`'s own `extensions::pausable::Resume::invoke_signed`.
pub fn resume_mint(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
) -> Result<()> {
    resume_mint_signed(program, mint, authority, &[])
}

pub fn resume_mint_signed(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = solana_program::instruction::Instruction {
            program_id: program.address(),
            accounts: vec![
                solana_program::instruction::AccountMeta::new(mint.info.address(), false),
                solana_program::instruction::AccountMeta::new_readonly(
                    authority.info.address(),
                    true,
                ),
            ],
            data: vec![44u8, 2u8],
        };
        let accounts = [CpiHandle::from(mint), authority, program];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mint_handle: CpiHandle<'_> = CpiHandle::from(mint);
        let ix_accounts = [
            ::pinocchio::instruction::InstructionAccount::writable(mint_handle.info.view.address()),
            ::pinocchio::instruction::InstructionAccount::readonly_signer(
                authority.info.view.address(),
            ),
        ];
        let instruction = ::pinocchio::instruction::InstructionView {
            program_id: program.info.view.address(),
            accounts: &ix_accounts,
            data: &[44u8, 2u8],
        };
        let handles = [mint_handle, authority];
        crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles, signer_seeds)
    }
}
