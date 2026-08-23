// ===========================================================================
// extensions/interest_bearing_mint.rs — InterestBearingConfig
// ===========================================================================

//! `InterestBearingConfig` (mint extension), plus every CPI naclac-token
//! supports against it: `initialize_interest_bearing_mint`/
//! `update_interest_bearing_mint_rate` — confirmed against anchor-spl-v2's
//! own `token_2022_extensions::interest_bearing_mint` module as the parity
//! target, which exposes exactly these two functions.

use super::{read_optional_address, Extension};
use crate::prelude::{Address, CpiHandle, CpiHandleMut, Pod, Result, Zeroable};

#[cfg(not(feature = "pinocchio"))]
use crate::prelude::vec;
#[cfg(not(feature = "pinocchio"))]
use crate::wrappers::ToAddress;

/// Raw Token-2022 `InterestBearingConfig` mint extension (52 bytes): the
/// authority allowed to change the rate, plus the timestamps/rates needed
/// to compute continuously-compounded interest since initialization. Layout
/// verified against
/// `spl-token-2022-interface::extension::interest_bearing_mint::InterestBearingConfig`'s
/// real field order/sizes (`rate_authority: MaybeNull<Address>` (32),
/// `initialization_timestamp: I64` (8), `pre_update_average_rate: I16` (2),
/// `last_update_timestamp: I64` (8), `current_rate: I16` (2)); `MaybeNull<Address>`
/// reserves the all-zero address for `None`, the same convention
/// `read_optional_address` already handles for `PermanentDelegate`/
/// `TransferHook`, confirmed against `solana_address`'s real
/// `impl Nullable for Address`.
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct InterestBearingConfig(pub [u8; 52]);

unsafe impl Pod for InterestBearingConfig {}
unsafe impl Zeroable for InterestBearingConfig {}
impl Extension for InterestBearingConfig {
    const TYPE: u16 = 10; // ExtensionType::InterestBearingConfig
    const ACCOUNT_TYPE: u8 = 1; // AccountType::Mint
}

impl InterestBearingConfig {
    pub fn rate_authority(&self) -> Option<Address> {
        read_optional_address(&self.0, 0)
    }
    pub fn initialization_timestamp(&self) -> i64 {
        i64::from_le_bytes(self.0[32..40].try_into().unwrap())
    }
    pub fn pre_update_average_rate(&self) -> i16 {
        i16::from_le_bytes(self.0[40..42].try_into().unwrap())
    }
    pub fn last_update_timestamp(&self) -> i64 {
        i64::from_le_bytes(self.0[42..50].try_into().unwrap())
    }
    pub fn current_rate(&self) -> i16 {
        i16::from_le_bytes(self.0[50..52].try_into().unwrap())
    }
}

/// Initializes the `InterestBearingConfig` extension on a not-yet-initialized
/// mint. Must be called before `initialize_mint`/`initialize_mint_signed`,
/// same as the other extension initializers. Instruction encoding (top-level
/// discriminant `33` = `InterestBearingMintExtension`, sub-discriminant `0`
/// = `Initialize`, then `rate_authority` as a fixed 32 bytes (all-zero
/// meaning `None`) followed by `rate` as 2 raw little-endian bytes) verified
/// against `spl-token-2022-interface`'s real
/// `InterestBearingMintInstruction::Initialize`/`initialize`, cross-checked
/// against `pinocchio-token-2022`'s own
/// `extensions::interest_bearing_mint::Initialize::invoke`.
pub fn initialize_interest_bearing_mint(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    rate_authority: Option<&Address>,
    rate: i16,
) -> Result<()> {
    initialize_interest_bearing_mint_signed(program, mint, rate_authority, rate, &[])
}

pub fn initialize_interest_bearing_mint_signed(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    rate_authority: Option<&Address>,
    rate: i16,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = vec![33u8, 0u8];
        data.extend_from_slice(rate_authority.map(|a| a.as_ref()).unwrap_or(&[0u8; 32]));
        data.extend_from_slice(&rate.to_le_bytes());

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
        let ix = ::pinocchio_token_2022::instructions::interest_bearing_mint::Initialize {
            mint: &mint.info.view,
            rate_authority: rate_authority.map(|a| a.as_address()),
            rate,
            token_program: &program.info.view.address(),
        };
        ix.invoke()
    }
}

/// Updates an already-initialized mint's interest rate. Signed by the
/// mint's rate authority. Instruction encoding (top-level discriminant
/// `33`, sub-discriminant `1` = `UpdateRate`, then `rate` as 2 raw
/// little-endian bytes) verified against `spl-token-2022-interface`'s real
/// `InterestBearingMintInstruction::UpdateRate`/`update_rate`, cross-checked
/// against `pinocchio-token-2022`'s own
/// `extensions::interest_bearing_mint::Update::invoke_signed`.
///
/// Doesn't go through `pinocchio-token-2022`'s own `Update` struct — it's
/// generic over a multisig-signer type, and naclac-token supports only a
/// single fixed authority everywhere else (see `transfer_fee.rs`'s own note
/// on the same tradeoff for its ongoing-action CPIs). The raw
/// `InstructionView` is hand-built instead.
pub fn update_interest_bearing_mint_rate(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    rate_authority: CpiHandle<'_>,
    rate: i16,
) -> Result<()> {
    update_interest_bearing_mint_rate_signed(program, mint, rate_authority, rate, &[])
}

pub fn update_interest_bearing_mint_rate_signed(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    rate_authority: CpiHandle<'_>,
    rate: i16,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = vec![33u8, 1u8];
        data.extend_from_slice(&rate.to_le_bytes());

        let ix = solana_program::instruction::Instruction {
            program_id: program.address(),
            accounts: vec![
                solana_program::instruction::AccountMeta::new(mint.info.address(), false),
                solana_program::instruction::AccountMeta::new_readonly(
                    rate_authority.info.address(),
                    true,
                ),
            ],
            data,
        };
        let accounts = [CpiHandle::from(mint), rate_authority, program];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mut data = [0u8; 4];
        data[0] = 33;
        data[1] = 1;
        data[2..4].copy_from_slice(&rate.to_le_bytes());

        let mint_handle: CpiHandle<'_> = CpiHandle::from(mint);
        let ix_accounts = [
            ::pinocchio::instruction::InstructionAccount::writable(mint_handle.info.view.address()),
            ::pinocchio::instruction::InstructionAccount::readonly_signer(
                rate_authority.info.view.address(),
            ),
        ];
        let instruction = ::pinocchio::instruction::InstructionView {
            program_id: program.info.view.address(),
            accounts: &ix_accounts,
            data: &data,
        };
        let handles = [mint_handle, rate_authority];
        crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles, signer_seeds)
    }
}
