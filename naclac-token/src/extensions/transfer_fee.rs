// ===========================================================================
// extensions/transfer_fee.rs — TransferFeeConfig / TransferFeeAmount
// ===========================================================================

//! `TransferFeeConfig` (mint extension) / `TransferFeeAmount` (token-account
//! extension), plus every CPI naclac-token supports against them: creation
//! (`initialize_transfer_fee_config`), the ongoing-action set
//! (`set_transfer_fee`, `transfer_checked_with_fee`,
//! `harvest_withheld_tokens_to_mint`, `withdraw_withheld_tokens_from_mint`,
//! `withdraw_withheld_tokens_from_accounts`) confirmed against
//! anchor-spl-v2's own `token_2022_extensions::transfer_fee` module as the
//! parity target — that module exposes exactly these six functions, no more.

use super::{ceil_div_u128, read_optional_address, read_u16, read_u64, Extension};
use crate::prelude::{Address, CpiHandle, CpiHandleMut, NaclacError, Pod, Result, Zeroable};

#[cfg(not(feature = "pinocchio"))]
use crate::prelude::vec;
#[cfg(not(feature = "pinocchio"))]
use crate::wrappers::ToAddress;

/// Raw Token-2022 `TransferFeeConfig` mint extension (108 bytes): current
/// and upcoming fee rate/cap, plus the withheld-and-not-yet-withdrawn
/// amount sitting on the mint itself. Layout verified against
/// `spl-token-2022-interface::extension::transfer_fee::TransferFeeConfig`'s
/// real field order/sizes.
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct TransferFeeConfig(pub [u8; 108]);

unsafe impl Pod for TransferFeeConfig {}
unsafe impl Zeroable for TransferFeeConfig {}
impl Extension for TransferFeeConfig {
    const TYPE: u16 = 1; // ExtensionType::TransferFeeConfig
    const ACCOUNT_TYPE: u8 = 1; // AccountType::Mint
}

impl TransferFeeConfig {
    pub fn transfer_fee_config_authority(&self) -> Option<Address> {
        read_optional_address(&self.0, 0)
    }
    pub fn withdraw_withheld_authority(&self) -> Option<Address> {
        read_optional_address(&self.0, 32)
    }
    /// Withheld transfer fees already moved to the mint, pending withdrawal.
    pub fn withheld_amount(&self) -> u64 {
        read_u64(&self.0, 64)
    }
    pub fn older_transfer_fee_epoch(&self) -> u64 {
        read_u64(&self.0, 72)
    }
    pub fn older_transfer_fee_maximum_fee(&self) -> u64 {
        read_u64(&self.0, 80)
    }
    pub fn older_transfer_fee_basis_points(&self) -> u16 {
        read_u16(&self.0, 88)
    }
    pub fn newer_transfer_fee_epoch(&self) -> u64 {
        read_u64(&self.0, 90)
    }
    pub fn newer_transfer_fee_maximum_fee(&self) -> u64 {
        read_u64(&self.0, 98)
    }
    pub fn newer_transfer_fee_basis_points(&self) -> u16 {
        read_u16(&self.0, 106)
    }

    /// The `(basis_points, maximum_fee)` pair in effect at `current_epoch` —
    /// the newer rate once `current_epoch >= newer_transfer_fee_epoch`, the
    /// older rate otherwise. Matches `TransferFeeConfig::get_epoch_fee` in
    /// `spl-token-2022-interface`.
    fn active_rate(&self, current_epoch: u64) -> (u16, u64) {
        if current_epoch >= self.newer_transfer_fee_epoch() {
            (
                self.newer_transfer_fee_basis_points(),
                self.newer_transfer_fee_maximum_fee(),
            )
        } else {
            (
                self.older_transfer_fee_basis_points(),
                self.older_transfer_fee_maximum_fee(),
            )
        }
    }

    /// The fee Token-2022 will withhold from a transfer of `pre_fee_amount`
    /// at `current_epoch` — `None` only on arithmetic overflow. Ceiling
    /// division, capped at the active `maximum_fee`; matches
    /// `TransferFee::calculate_fee` in `spl-token-2022-interface` exactly.
    pub fn calculate_fee(&self, current_epoch: u64, pre_fee_amount: u64) -> Option<u64> {
        let (basis_points, maximum_fee) = self.active_rate(current_epoch);
        if basis_points == 0 || pre_fee_amount == 0 {
            return Some(0);
        }
        let numerator = (pre_fee_amount as u128).checked_mul(basis_points as u128)?;
        let raw_fee = ceil_div_u128(numerator, 10_000)?;
        let raw_fee: u64 = raw_fee.try_into().ok()?;
        Some(raw_fee.min(maximum_fee))
    }

    /// The amount the recipient actually receives from a transfer of
    /// `pre_fee_amount` — `pre_fee_amount` minus `calculate_fee`'s result.
    /// A naclac program's own accounting must use this, not the raw
    /// transfer amount, whenever the mint carries this extension, or its
    /// bookkeeping will silently diverge from what actually lands in the
    /// destination account.
    pub fn calculate_post_fee_amount(
        &self,
        current_epoch: u64,
        pre_fee_amount: u64,
    ) -> Option<u64> {
        pre_fee_amount.checked_sub(self.calculate_fee(current_epoch, pre_fee_amount)?)
    }
}

/// Raw Token-2022 `TransferFeeAmount` token-account extension (8 bytes):
/// fees withheld on this specific account, not yet harvested to the mint.
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct TransferFeeAmount(pub [u8; 8]);

unsafe impl Pod for TransferFeeAmount {}
unsafe impl Zeroable for TransferFeeAmount {}
impl Extension for TransferFeeAmount {
    const TYPE: u16 = 2; // ExtensionType::TransferFeeAmount
    const ACCOUNT_TYPE: u8 = 2; // AccountType::Account
}

impl TransferFeeAmount {
    pub fn withheld_amount(&self) -> u64 {
        read_u64(&self.0, 0)
    }
}

/// Initializes the `TransferFeeConfig` extension on a not-yet-initialized
/// mint. Must be called before `initialize_mint`/`initialize_mint_signed` —
/// Token-2022 rejects extension-initialize instructions on an already-
/// initialized mint. Instruction encoding (top-level discriminant `26` =
/// `TransferFeeExtension`, sub-discriminant `0` = `Initialize`, each
/// optional authority as a 1-byte presence tag plus 32 bytes if present)
/// verified against `spl-token-2022-interface`'s real
/// `TransferFeeInstruction::pack`/`TokenInstruction::pack_pubkey_option`,
/// and independently cross-checked against `pinocchio-token-2022`'s own
/// `InitializeTransferFeeConfig::invoke` — both agree.
pub fn initialize_transfer_fee_config(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    transfer_fee_config_authority: Option<&Address>,
    withdraw_withheld_authority: Option<&Address>,
    transfer_fee_basis_points: u16,
    maximum_fee: u64,
) -> Result<()> {
    initialize_transfer_fee_config_signed(
        program,
        mint,
        transfer_fee_config_authority,
        withdraw_withheld_authority,
        transfer_fee_basis_points,
        maximum_fee,
        &[],
    )
}

pub fn initialize_transfer_fee_config_signed(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    transfer_fee_config_authority: Option<&Address>,
    withdraw_withheld_authority: Option<&Address>,
    transfer_fee_basis_points: u16,
    maximum_fee: u64,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = vec![26u8, 0u8];
        if let Some(authority) = transfer_fee_config_authority {
            data.push(1);
            data.extend_from_slice(authority.as_ref());
        } else {
            data.push(0);
        }
        if let Some(authority) = withdraw_withheld_authority {
            data.push(1);
            data.extend_from_slice(authority.as_ref());
        } else {
            data.push(0);
        }
        data.extend_from_slice(&transfer_fee_basis_points.to_le_bytes());
        data.extend_from_slice(&maximum_fee.to_le_bytes());

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
        let ix = ::pinocchio_token_2022::instructions::transfer_fee::InitializeTransferFeeConfig {
            mint: &mint.info.view,
            transfer_fee_config_authority: transfer_fee_config_authority.map(|a| a.as_address()),
            withdraw_withheld_authority: withdraw_withheld_authority.map(|a| a.as_address()),
            transfer_fee_basis_points,
            maximum_fee,
            token_program: program.info.view.address(),
        };
        ix.invoke()
    }
}

/// Maximum number of source token accounts `harvest_withheld_tokens_to_mint`/
/// `withdraw_withheld_tokens_from_accounts` can batch into a single CPI.
/// Bounded well under `invoke_signed_pinocchio_handles`'s internal 32-account
/// cap (`naclac-core/src/cpi.rs`) even after adding the fixed leading
/// accounts each instruction also needs.
pub const MAX_TRANSFER_FEE_SOURCE_ACCOUNTS: usize = 16;

/// Updates an already-initialized mint's transfer fee rate/cap. Signed by
/// the mint's `transfer_fee_config_authority`. Instruction encoding
/// (top-level discriminant `26` = `TransferFeeExtension`, sub-discriminant
/// `5` = `SetTransferFee`, then `transfer_fee_basis_points`/`maximum_fee`)
/// verified against `spl-token-2022-interface`'s real
/// `TransferFeeInstruction::SetTransferFee`/`pack`.
///
/// Neither this nor any other function in this file goes through
/// `pinocchio-token-2022`'s own per-instruction structs for their
/// pinocchio-feature path — every one of that crate's matching structs is
/// generic over a multisig-signer type (`MultisigSigner: AsRef<AccountView>`),
/// and naclac-token supports only a single fixed authority everywhere else
/// (`transfer`/`transfer_checked`/`mint_to` in `naclac-token/src/token.rs`
/// have no multisig support either). The raw `InstructionView`/`Instruction`
/// is hand-built instead, the same way `initialize_transfer_fee_config`'s
/// non-pinocchio path already does.
pub fn set_transfer_fee(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
    transfer_fee_basis_points: u16,
    maximum_fee: u64,
) -> Result<()> {
    set_transfer_fee_signed(
        program,
        mint,
        authority,
        transfer_fee_basis_points,
        maximum_fee,
        &[],
    )
}

pub fn set_transfer_fee_signed(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
    transfer_fee_basis_points: u16,
    maximum_fee: u64,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = vec![26u8, 5u8];
        data.extend_from_slice(&transfer_fee_basis_points.to_le_bytes());
        data.extend_from_slice(&maximum_fee.to_le_bytes());

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
        let mut data = [0u8; 12];
        data[0] = 26;
        data[1] = 5;
        data[2..4].copy_from_slice(&transfer_fee_basis_points.to_le_bytes());
        data[4..12].copy_from_slice(&maximum_fee.to_le_bytes());

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

/// The real Token-2022 fee-aware transfer. Required whenever the mint
/// carries `TransferFeeConfig` — a plain `transfer`/`transfer_checked` CPI
/// against such a mint is rejected. `fee` must equal what
/// `TransferFeeConfig::calculate_fee` computes for `amount`, or Token-2022
/// rejects the instruction; passing `fee = 0` against a mint with no
/// `TransferFeeConfig` extension is explicitly allowed by the protocol
/// (lets a caller use this unconditionally without probing the mint first).
/// Instruction encoding (discriminant `26`, sub-discriminant `1`) verified
/// against `spl-token-2022-interface`'s real
/// `TransferFeeInstruction::TransferCheckedWithFee`/`pack`.
pub fn transfer_checked_with_fee(
    program: CpiHandle<'_>,
    source: CpiHandleMut<'_>,
    mint: CpiHandle<'_>,
    destination: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
    amount: u64,
    decimals: u8,
    fee: u64,
) -> Result<()> {
    transfer_checked_with_fee_signed(
        program,
        source,
        mint,
        destination,
        authority,
        amount,
        decimals,
        fee,
        &[],
    )
}

#[allow(clippy::too_many_arguments)]
pub fn transfer_checked_with_fee_signed(
    program: CpiHandle<'_>,
    source: CpiHandleMut<'_>,
    mint: CpiHandle<'_>,
    destination: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
    amount: u64,
    decimals: u8,
    fee: u64,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = vec![26u8, 1u8];
        data.extend_from_slice(&amount.to_le_bytes());
        data.push(decimals);
        data.extend_from_slice(&fee.to_le_bytes());

        let ix = solana_program::instruction::Instruction {
            program_id: program.address(),
            accounts: vec![
                solana_program::instruction::AccountMeta::new(source.info.address(), false),
                solana_program::instruction::AccountMeta::new_readonly(mint.info.address(), false),
                solana_program::instruction::AccountMeta::new(destination.info.address(), false),
                solana_program::instruction::AccountMeta::new_readonly(
                    authority.info.address(),
                    true,
                ),
            ],
            data,
        };
        let accounts = [
            CpiHandle::from(source),
            mint,
            CpiHandle::from(destination),
            authority,
            program,
        ];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mut data = [0u8; 19];
        data[0] = 26;
        data[1] = 1;
        data[2..10].copy_from_slice(&amount.to_le_bytes());
        data[10] = decimals;
        data[11..19].copy_from_slice(&fee.to_le_bytes());

        let source_handle: CpiHandle<'_> = CpiHandle::from(source);
        let destination_handle: CpiHandle<'_> = CpiHandle::from(destination);
        let ix_accounts = [
            ::pinocchio::instruction::InstructionAccount::writable(
                source_handle.info.view.address(),
            ),
            ::pinocchio::instruction::InstructionAccount::readonly(mint.info.view.address()),
            ::pinocchio::instruction::InstructionAccount::writable(
                destination_handle.info.view.address(),
            ),
            ::pinocchio::instruction::InstructionAccount::readonly_signer(
                authority.info.view.address(),
            ),
        ];
        let instruction = ::pinocchio::instruction::InstructionView {
            program_id: program.info.view.address(),
            accounts: &ix_accounts,
            data: &data,
        };
        let handles = [source_handle, mint, destination_handle, authority];
        crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles, signer_seeds)
    }
}

/// Sweeps each of `sources`' withheld transfer fees into the mint's own
/// withheld balance. Permissionless — no authority account, matching the
/// real protocol instruction, which anyone may call. Instruction encoding
/// (discriminant `26`, sub-discriminant `4`, no data beyond the
/// discriminants) verified against `spl-token-2022-interface`'s real
/// `TransferFeeInstruction::HarvestWithheldTokensToMint`/`pack`.
pub fn harvest_withheld_tokens_to_mint(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    sources: &[CpiHandle<'_>],
) -> Result<()> {
    harvest_withheld_tokens_to_mint_signed(program, mint, sources, &[])
}

pub fn harvest_withheld_tokens_to_mint_signed(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    sources: &[CpiHandle<'_>],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    if sources.len() > MAX_TRANSFER_FEE_SOURCE_ACCOUNTS {
        return Err(NaclacError::TooManyExtraAccounts.into());
    }
    let data = [26u8, 4u8];

    #[cfg(not(feature = "pinocchio"))]
    {
        let mut accounts = vec![solana_program::instruction::AccountMeta::new(
            mint.info.address(),
            false,
        )];
        accounts.extend(
            sources.iter().map(|source| {
                solana_program::instruction::AccountMeta::new(source.address(), false)
            }),
        );
        let ix = solana_program::instruction::Instruction {
            program_id: program.address(),
            accounts,
            data: data.to_vec(),
        };
        let mut handles = crate::prelude::Vec::with_capacity(2 + sources.len());
        handles.push(CpiHandle::from(mint));
        handles.extend(sources.iter().cloned());
        handles.push(program);
        crate::cpi::invoke_signed(&ix, &handles, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mint_handle: CpiHandle<'_> = CpiHandle::from(mint);
        let mut addresses: [&::pinocchio::Address; MAX_TRANSFER_FEE_SOURCE_ACCOUNTS + 1] =
            [mint_handle.info.view.address(); MAX_TRANSFER_FEE_SOURCE_ACCOUNTS + 1];
        for (slot, source) in addresses[1..].iter_mut().zip(sources.iter()) {
            *slot = source.info.view.address();
        }
        let ix_accounts_all: [::pinocchio::instruction::InstructionAccount;
            MAX_TRANSFER_FEE_SOURCE_ACCOUNTS + 1] = core::array::from_fn(|i| {
            ::pinocchio::instruction::InstructionAccount::writable(addresses[i])
        });
        let ix_accounts = &ix_accounts_all[..sources.len() + 1];

        let instruction = ::pinocchio::instruction::InstructionView {
            program_id: program.info.view.address(),
            accounts: ix_accounts,
            data: &data,
        };
        let mut cpi_handles = [mint_handle; MAX_TRANSFER_FEE_SOURCE_ACCOUNTS + 1];
        cpi_handles[1..1 + sources.len()].clone_from_slice(sources);
        crate::cpi::invoke_signed_pinocchio_handles(
            &instruction,
            &cpi_handles[..sources.len() + 1],
            signer_seeds,
        )
    }
}

/// Withdraws all of the mint's own withheld fee balance to `destination`.
/// Signed by the mint's `withdraw_withheld_authority`. Instruction encoding
/// (discriminant `26`, sub-discriminant `2`) verified against
/// `spl-token-2022-interface`'s real
/// `TransferFeeInstruction::WithdrawWithheldTokensFromMint`/`pack`.
pub fn withdraw_withheld_tokens_from_mint(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    destination: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
) -> Result<()> {
    withdraw_withheld_tokens_from_mint_signed(program, mint, destination, authority, &[])
}

pub fn withdraw_withheld_tokens_from_mint_signed(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    destination: CpiHandleMut<'_>,
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
                solana_program::instruction::AccountMeta::new(destination.info.address(), false),
                solana_program::instruction::AccountMeta::new_readonly(
                    authority.info.address(),
                    true,
                ),
            ],
            data: vec![26u8, 2u8],
        };
        let accounts = [
            CpiHandle::from(mint),
            CpiHandle::from(destination),
            authority,
            program,
        ];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mint_handle: CpiHandle<'_> = CpiHandle::from(mint);
        let destination_handle: CpiHandle<'_> = CpiHandle::from(destination);
        let ix_accounts = [
            ::pinocchio::instruction::InstructionAccount::writable(mint_handle.info.view.address()),
            ::pinocchio::instruction::InstructionAccount::writable(
                destination_handle.info.view.address(),
            ),
            ::pinocchio::instruction::InstructionAccount::readonly_signer(
                authority.info.view.address(),
            ),
        ];
        let instruction = ::pinocchio::instruction::InstructionView {
            program_id: program.info.view.address(),
            accounts: &ix_accounts,
            data: &[26u8, 2u8],
        };
        let handles = [mint_handle, destination_handle, authority];
        crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles, signer_seeds)
    }
}

/// Withdraws each of `sources`' withheld fee balance directly to
/// `destination`, bypassing the mint. Signed by the mint's
/// `withdraw_withheld_authority`. Note `mint` is read-only here (unlike
/// `withdraw_withheld_tokens_from_mint`, where it's written into) — verified
/// against the real accounts list, not assumed to match the sibling
/// instruction. Instruction encoding (discriminant `26`, sub-discriminant
/// `3`, then `num_token_accounts: u8`) verified against
/// `spl-token-2022-interface`'s real
/// `TransferFeeInstruction::WithdrawWithheldTokensFromAccounts`/`pack`.
pub fn withdraw_withheld_tokens_from_accounts(
    program: CpiHandle<'_>,
    mint: CpiHandle<'_>,
    destination: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
    sources: &[CpiHandle<'_>],
) -> Result<()> {
    withdraw_withheld_tokens_from_accounts_signed(
        program,
        mint,
        destination,
        authority,
        sources,
        &[],
    )
}

pub fn withdraw_withheld_tokens_from_accounts_signed(
    program: CpiHandle<'_>,
    mint: CpiHandle<'_>,
    destination: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
    sources: &[CpiHandle<'_>],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    if sources.len() > MAX_TRANSFER_FEE_SOURCE_ACCOUNTS {
        return Err(NaclacError::TooManyExtraAccounts.into());
    }
    let num_token_accounts = sources.len() as u8;

    #[cfg(not(feature = "pinocchio"))]
    {
        let mut accounts = vec![
            solana_program::instruction::AccountMeta::new_readonly(mint.info.address(), false),
            solana_program::instruction::AccountMeta::new(destination.info.address(), false),
            solana_program::instruction::AccountMeta::new_readonly(authority.info.address(), true),
        ];
        accounts.extend(
            sources.iter().map(|source| {
                solana_program::instruction::AccountMeta::new(source.address(), false)
            }),
        );
        let ix = solana_program::instruction::Instruction {
            program_id: program.address(),
            accounts,
            data: vec![26u8, 3u8, num_token_accounts],
        };
        let mut handles = crate::prelude::Vec::with_capacity(4 + sources.len());
        handles.push(mint);
        handles.push(CpiHandle::from(destination));
        handles.push(authority);
        handles.extend(sources.iter().cloned());
        handles.push(program);
        crate::cpi::invoke_signed(&ix, &handles, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let destination_handle: CpiHandle<'_> = CpiHandle::from(destination);
        let mut addresses: [&::pinocchio::Address; MAX_TRANSFER_FEE_SOURCE_ACCOUNTS + 3] =
            [mint.info.view.address(); MAX_TRANSFER_FEE_SOURCE_ACCOUNTS + 3];
        addresses[1] = destination_handle.info.view.address();
        addresses[2] = authority.info.view.address();
        for (slot, source) in addresses[3..].iter_mut().zip(sources.iter()) {
            *slot = source.info.view.address();
        }

        let mut ix_accounts_all: [::pinocchio::instruction::InstructionAccount;
            MAX_TRANSFER_FEE_SOURCE_ACCOUNTS + 3] = core::array::from_fn(|i| {
            ::pinocchio::instruction::InstructionAccount::writable(addresses[i])
        });
        ix_accounts_all[0] = ::pinocchio::instruction::InstructionAccount::readonly(addresses[0]);
        ix_accounts_all[2] =
            ::pinocchio::instruction::InstructionAccount::readonly_signer(addresses[2]);
        let ix_accounts = &ix_accounts_all[..sources.len() + 3];

        let data = [26u8, 3u8, num_token_accounts];
        let instruction = ::pinocchio::instruction::InstructionView {
            program_id: program.info.view.address(),
            accounts: ix_accounts,
            data: &data,
        };
        let mut cpi_handles: [CpiHandle<'_>; MAX_TRANSFER_FEE_SOURCE_ACCOUNTS + 3] =
            core::array::from_fn(|_| mint.clone());
        cpi_handles[1] = destination_handle;
        cpi_handles[2] = authority;
        cpi_handles[3..3 + sources.len()].clone_from_slice(sources);
        crate::cpi::invoke_signed_pinocchio_handles(
            &instruction,
            &cpi_handles[..sources.len() + 3],
            signer_seeds,
        )
    }
}
