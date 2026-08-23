// ===========================================================================
// extensions/default_account_state.rs — DefaultAccountState
// ===========================================================================

//! `DefaultAccountState` (mint extension), plus every CPI naclac-token
//! supports against it: `initialize_default_account_state`/
//! `update_default_account_state` — confirmed against anchor-spl-v2's own
//! `token_2022_extensions::default_account_state` module as the parity
//! target, which exposes exactly these two functions.

use super::Extension;
use crate::prelude::{CpiHandle, CpiHandleMut, Pod, Result, Zeroable};

#[cfg(not(feature = "pinocchio"))]
use crate::prelude::vec;
#[cfg(not(feature = "pinocchio"))]
use crate::wrappers::ToAddress;

/// Raw Token-2022 `DefaultAccountState` mint extension (1 byte): the
/// `AccountState` every new token account of this mint is created in.
/// Layout verified against
/// `spl-token-2022-interface::extension::default_account_state::DefaultAccountState`.
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct DefaultAccountState(pub [u8; 1]);

unsafe impl Pod for DefaultAccountState {}
unsafe impl Zeroable for DefaultAccountState {}
impl Extension for DefaultAccountState {
    const TYPE: u16 = 6; // ExtensionType::DefaultAccountState
    const ACCOUNT_TYPE: u8 = 1; // AccountType::Mint
}

impl DefaultAccountState {
    /// Raw SPL `AccountState` byte: `0` = uninitialized, `1` = initialized, `2` = frozen.
    pub fn state(&self) -> u8 {
        self.0[0]
    }
}

/// Initializes the `DefaultAccountState` extension on a not-yet-initialized
/// mint. Must be called before `initialize_mint`/`initialize_mint_signed`,
/// same as the other extension initializers. Instruction encoding (top-level
/// discriminant `28` = `DefaultAccountStateExtension`, sub-discriminant `0`
/// = `Initialize`, then the raw `AccountState` byte) verified against
/// `spl-token-2022-interface`'s real
/// `extension::default_account_state::instruction::{DefaultAccountStateInstruction::Initialize, initialize_default_account_state}`,
/// cross-checked against `pinocchio-token-2022`'s own
/// `extensions::default_account_state::Initialize::invoke`.
pub fn initialize_default_account_state(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    state: u8,
) -> Result<()> {
    initialize_default_account_state_signed(program, mint, state, &[])
}

pub fn initialize_default_account_state_signed(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    state: u8,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    #[cfg(not(feature = "pinocchio"))]
    {
        let data = vec![28u8, 0u8, state];

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
        let data = [28u8, 0u8, state];

        let mint_handle: CpiHandle<'_> = CpiHandle::from(mint);
        let ix_accounts = [::pinocchio::instruction::InstructionAccount::writable(
            mint_handle.info.view.address(),
        )];
        let instruction = ::pinocchio::instruction::InstructionView {
            program_id: program.info.view.address(),
            accounts: &ix_accounts,
            data: &data,
        };
        let handles = [mint_handle];
        crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles, signer_seeds)
    }
}

/// Updates an already-initialized mint's default account state. Signed by
/// the mint's freeze authority. Instruction encoding (top-level discriminant
/// `28`, sub-discriminant `1` = `Update`, then the raw `AccountState` byte)
/// verified against `spl-token-2022-interface`'s real
/// `DefaultAccountStateInstruction::Update`/`update_default_account_state`,
/// cross-checked against `pinocchio-token-2022`'s own
/// `extensions::default_account_state::Update::invoke_signed`.
///
/// Doesn't go through `pinocchio-token-2022`'s own `Update` struct — it's
/// generic over a multisig-signer type, and naclac-token supports only a
/// single fixed authority everywhere else (see `transfer_fee.rs`'s own note
/// on the same tradeoff for its ongoing-action CPIs). The raw
/// `InstructionView` is hand-built instead.
pub fn update_default_account_state(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    freeze_authority: CpiHandle<'_>,
    state: u8,
) -> Result<()> {
    update_default_account_state_signed(program, mint, freeze_authority, state, &[])
}

pub fn update_default_account_state_signed(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    freeze_authority: CpiHandle<'_>,
    state: u8,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    #[cfg(not(feature = "pinocchio"))]
    {
        let data = vec![28u8, 1u8, state];

        let ix = solana_program::instruction::Instruction {
            program_id: program.address(),
            accounts: vec![
                solana_program::instruction::AccountMeta::new(mint.info.address(), false),
                solana_program::instruction::AccountMeta::new_readonly(
                    freeze_authority.info.address(),
                    true,
                ),
            ],
            data,
        };
        let accounts = [CpiHandle::from(mint), freeze_authority, program];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let data = [28u8, 1u8, state];

        let mint_handle: CpiHandle<'_> = CpiHandle::from(mint);
        let ix_accounts = [
            ::pinocchio::instruction::InstructionAccount::writable(mint_handle.info.view.address()),
            ::pinocchio::instruction::InstructionAccount::readonly_signer(
                freeze_authority.info.view.address(),
            ),
        ];
        let instruction = ::pinocchio::instruction::InstructionView {
            program_id: program.info.view.address(),
            accounts: &ix_accounts,
            data: &data,
        };
        let handles = [mint_handle, freeze_authority];
        crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles, signer_seeds)
    }
}
