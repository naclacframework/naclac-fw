// ===========================================================================
// extensions/scaled_ui_amount.rs — ScaledUiAmountConfig
// ===========================================================================

//! `ScaledUiAmountConfig` (mint extension), plus every CPI naclac-token
//! supports against it: `initialize_scaled_ui_amount_config`/
//! `update_scaled_ui_amount_multiplier`. Present in neither the official
//! `anchor-spl` crate nor the local pinocchio-native `anchor-spl-v2` —
//! confirmed by a crate-wide search of both — so there is no existing
//! parity target; this mirrors `InterestBearingConfig`'s own shape (an
//! authority plus a current/scheduled value pair), the closest real
//! precedent in this crate for a scheduled-value mint extension.

use super::{read_optional_address, Extension};
use crate::prelude::{Address, CpiHandle, CpiHandleMut, Pod, Result, Zeroable};

#[cfg(not(feature = "pinocchio"))]
use crate::prelude::vec;
#[cfg(not(feature = "pinocchio"))]
use crate::wrappers::ToAddress;

/// Raw Token-2022 `ScaledUiAmountConfig` mint extension (56 bytes): the
/// authority allowed to change the multiplier, the current multiplier, and
/// a scheduled next multiplier plus the timestamp it takes effect at.
/// Layout verified against
/// `spl-token-2022-interface::extension::scaled_ui_amount::ScaledUiAmountConfig`'s
/// real field order/sizes (`authority: MaybeNull<Address>` (32),
/// `multiplier: PodF64` (8), `new_multiplier_effective_timestamp: UnixTimestamp`
/// (8), `new_multiplier: PodF64` (8)) — `multiplier`/`new_multiplier` are
/// raw little-endian `f64` bytes, not a naclac-invented encoding.
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct ScaledUiAmountConfig(pub [u8; 56]);

unsafe impl Pod for ScaledUiAmountConfig {}
unsafe impl Zeroable for ScaledUiAmountConfig {}
impl Extension for ScaledUiAmountConfig {
    const TYPE: u16 = 25; // ExtensionType::ScaledUiAmount
    const ACCOUNT_TYPE: u8 = 1; // AccountType::Mint
}

impl ScaledUiAmountConfig {
    pub fn authority(&self) -> Option<Address> {
        read_optional_address(&self.0, 0)
    }
    pub fn multiplier(&self) -> f64 {
        f64::from_le_bytes(self.0[32..40].try_into().unwrap())
    }
    pub fn new_multiplier_effective_timestamp(&self) -> i64 {
        i64::from_le_bytes(self.0[40..48].try_into().unwrap())
    }
    pub fn new_multiplier(&self) -> f64 {
        f64::from_le_bytes(self.0[48..56].try_into().unwrap())
    }
}

/// Initializes the `ScaledUiAmountConfig` extension on a not-yet-initialized
/// mint. Must be called before `initialize_mint`/`initialize_mint_signed`,
/// same as the other extension initializers. Instruction encoding (top-level
/// discriminant `43` = `ScaledUiAmountExtension`, sub-discriminant `0` =
/// `Initialize`, then `authority` as a fixed 32 bytes (all-zero meaning
/// `None`) followed by `multiplier` as 8 raw little-endian `f64` bytes)
/// verified against `spl-token-2022-interface`'s real
/// `ScaledUiAmountMintInstruction::Initialize`/`initialize`, cross-checked
/// against `pinocchio-token-2022`'s own
/// `extensions::scaled_ui_amount::Initialize::invoke`.
pub fn initialize_scaled_ui_amount_config(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    authority: Option<&Address>,
    multiplier: f64,
) -> Result<()> {
    initialize_scaled_ui_amount_config_signed(program, mint, authority, multiplier, &[])
}

pub fn initialize_scaled_ui_amount_config_signed(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    authority: Option<&Address>,
    multiplier: f64,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = vec![43u8, 0u8];
        data.extend_from_slice(authority.map(|a| a.as_ref()).unwrap_or(&[0u8; 32]));
        data.extend_from_slice(&multiplier.to_le_bytes());

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
        let _ = signer_seeds;
        let ix = ::pinocchio_token_2022::instructions::scaled_ui_amount::Initialize {
            mint_account: &mint.info.view,
            authority: authority.map(|a| a.as_address()),
            multiplier,
            token_program: &program.info.view.address(),
        };
        ix.invoke()
    }
}

/// Updates an already-initialized mint's multiplier, scheduled to take
/// effect at `effective_timestamp` (immediately, if already in the past).
/// Signed by the mint's multiplier authority. Instruction encoding
/// (top-level discriminant `43`, sub-discriminant `1` = `UpdateMultiplier`,
/// then `multiplier` as 8 raw little-endian `f64` bytes followed by
/// `effective_timestamp` as 8 raw little-endian bytes) verified against
/// `spl-token-2022-interface`'s real
/// `ScaledUiAmountMintInstruction::UpdateMultiplier`/`update_multiplier`,
/// cross-checked against `pinocchio-token-2022`'s own
/// `extensions::scaled_ui_amount::UpdateMultiplier::invoke_signed`.
///
/// Doesn't go through `pinocchio-token-2022`'s own `UpdateMultiplier`
/// struct — it's generic over a multisig-signer type, and naclac-token
/// supports only a single fixed authority everywhere else (see
/// `transfer_fee.rs`'s own note on the same tradeoff for its ongoing-action
/// CPIs). The raw `InstructionView` is hand-built instead.
pub fn update_scaled_ui_amount_multiplier(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
    multiplier: f64,
    effective_timestamp: i64,
) -> Result<()> {
    update_scaled_ui_amount_multiplier_signed(
        program,
        mint,
        authority,
        multiplier,
        effective_timestamp,
        &[],
    )
}

pub fn update_scaled_ui_amount_multiplier_signed(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
    multiplier: f64,
    effective_timestamp: i64,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = vec![43u8, 1u8];
        data.extend_from_slice(&multiplier.to_le_bytes());
        data.extend_from_slice(&effective_timestamp.to_le_bytes());

        let ix = solana_program::instruction::Instruction {
            program_id: program.address(),
            accounts: vec![
                solana_program::instruction::AccountMeta::new(mint.info.address(), false),
                solana_program::instruction::AccountMeta::new_readonly(
                    authority.info.address(),
                    true,
                ),
            ],
            data,
        };
        let accounts = [CpiHandle::from(mint), authority, program];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mut data = [0u8; 18];
        data[0] = 43;
        data[1] = 1;
        data[2..10].copy_from_slice(&multiplier.to_le_bytes());
        data[10..18].copy_from_slice(&effective_timestamp.to_le_bytes());

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
            data: &data,
        };
        let handles = [mint_handle, authority];
        crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles, signer_seeds)
    }
}
