// ===========================================================================
// extensions/permissioned_burn.rs — PermissionedBurnConfig
// ===========================================================================

//! `PermissionedBurnConfig` (mint extension), plus the CPIs naclac-token
//! supports against it: `initialize_permissioned_burn`/`permissioned_burn`/
//! `permissioned_burn_checked`. Present in neither the official `anchor-spl`
//! crate nor the local pinocchio-native `anchor-spl-v2` — confirmed by a
//! crate-wide search of both — so there is no existing parity target; this
//! mirrors `PermanentDelegate`'s own shape (a single required authority, no
//! `Option`) for `Initialize`. The real protocol's fourth action,
//! `ConfidentialBurn`, is deliberately not implemented — it requires the
//! same ElGamal/zero-knowledge-proof machinery as `ConfidentialTransfer`,
//! out of scope for the same reason.

use super::Extension;
use crate::prelude::{Address, CpiHandle, CpiHandleMut, Pod, Result, Zeroable};

#[cfg(not(feature = "pinocchio"))]
use crate::prelude::vec;
#[cfg(not(feature = "pinocchio"))]
use crate::wrappers::ToAddress;

/// Raw Token-2022 `PermissionedBurnConfig` mint extension (32 bytes): the
/// authority that must co-sign any burn of this mint's tokens, alongside
/// the token account's own owner/delegate. Layout verified against
/// `spl-token-2022-interface::extension::permissioned_burn::PermissionedBurnConfig`.
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct PermissionedBurnConfig(pub [u8; 32]);

unsafe impl Pod for PermissionedBurnConfig {}
unsafe impl Zeroable for PermissionedBurnConfig {}
impl Extension for PermissionedBurnConfig {
    const TYPE: u16 = 28; // ExtensionType::PermissionedBurn
    const ACCOUNT_TYPE: u8 = 1; // AccountType::Mint
}

impl PermissionedBurnConfig {
    pub fn authority(&self) -> Option<Address> {
        super::read_optional_address(&self.0, 0)
    }
}

/// Initializes the `PermissionedBurnConfig` extension on a not-yet-initialized
/// mint. Must be called before `initialize_mint`/`initialize_mint_signed`,
/// same as the other extension initializers. Unlike most other extension
/// initializers, `authority` is a required, non-optional 32 bytes — the
/// real `InitializeInstructionData` has no `None` encoding, same shape as
/// `PermanentDelegate`'s own initializer. Instruction encoding (top-level
/// discriminant `46` = `PermissionedBurnExtension`, sub-discriminant `0` =
/// `Initialize`) verified against `spl-token-2022-interface`'s real
/// `PermissionedBurnInstruction::Initialize`/`initialize`, cross-checked
/// against `pinocchio-token-2022`'s own
/// `extensions::permissioned_burn::Initialize::invoke`.
pub fn initialize_permissioned_burn(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    authority: &Address,
) -> Result<()> {
    initialize_permissioned_burn_signed(program, mint, authority, &[])
}

pub fn initialize_permissioned_burn_signed(
    program: CpiHandle<'_>,
    mint: CpiHandleMut<'_>,
    authority: &Address,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = vec![46u8, 0u8];
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
        let _ = signer_seeds;
        let ix = ::pinocchio_token_2022::instructions::permissioned_burn::Initialize {
            mint: &mint.info.view,
            authority: authority.as_address(),
            token_program: &program.info.view.address(),
        };
        ix.invoke()
    }
}

/// Burns tokens from `source` on a mint with `PermissionedBurnConfig`
/// enabled. Requires *both* the mint's permissioned-burn authority and the
/// token account's own owner/delegate to sign — real Token-2022 rejects the
/// burn if either is missing. Instruction encoding (top-level discriminant
/// `46`, sub-discriminant `1` = `Burn`, then `amount` as 8 raw
/// little-endian bytes) verified against `spl-token-2022-interface`'s real
/// `PermissionedBurnInstruction::Burn`/`burn`, cross-checked against
/// `pinocchio-token-2022`'s own `extensions::permissioned_burn::Burn::invoke_signed`.
///
/// Doesn't go through `pinocchio-token-2022`'s own `Burn` struct — it's
/// generic over a multisig-signer type, and naclac-token supports only a
/// single fixed authority everywhere else (see `transfer_fee.rs`'s own note
/// on the same tradeoff for its ongoing-action CPIs). The raw
/// `InstructionView` is hand-built instead.
pub fn permissioned_burn(
    program: CpiHandle<'_>,
    source: CpiHandleMut<'_>,
    mint: CpiHandleMut<'_>,
    permissioned_burn_authority: CpiHandle<'_>,
    authority: CpiHandle<'_>,
    amount: u64,
) -> Result<()> {
    permissioned_burn_signed(
        program,
        source,
        mint,
        permissioned_burn_authority,
        authority,
        amount,
        &[],
    )
}

pub fn permissioned_burn_signed(
    program: CpiHandle<'_>,
    source: CpiHandleMut<'_>,
    mint: CpiHandleMut<'_>,
    permissioned_burn_authority: CpiHandle<'_>,
    authority: CpiHandle<'_>,
    amount: u64,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = vec![46u8, 1u8];
        data.extend_from_slice(&amount.to_le_bytes());

        let ix = solana_program::instruction::Instruction {
            program_id: program.address(),
            accounts: vec![
                solana_program::instruction::AccountMeta::new(source.info.address(), false),
                solana_program::instruction::AccountMeta::new(mint.info.address(), false),
                solana_program::instruction::AccountMeta::new_readonly(
                    permissioned_burn_authority.info.address(),
                    true,
                ),
                solana_program::instruction::AccountMeta::new_readonly(
                    authority.info.address(),
                    true,
                ),
            ],
            data,
        };
        let accounts = [
            CpiHandle::from(source),
            CpiHandle::from(mint),
            permissioned_burn_authority,
            authority,
            program,
        ];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mut data = [0u8; 10];
        data[0] = 46;
        data[1] = 1;
        data[2..10].copy_from_slice(&amount.to_le_bytes());

        let source_handle: CpiHandle<'_> = CpiHandle::from(source);
        let mint_handle: CpiHandle<'_> = CpiHandle::from(mint);
        let ix_accounts = [
            ::pinocchio::instruction::InstructionAccount::writable(
                source_handle.info.view.address(),
            ),
            ::pinocchio::instruction::InstructionAccount::writable(mint_handle.info.view.address()),
            ::pinocchio::instruction::InstructionAccount::readonly_signer(
                permissioned_burn_authority.info.view.address(),
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
        let handles = [source_handle, mint_handle, permissioned_burn_authority, authority];
        crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles, signer_seeds)
    }
}

/// Same as [`permissioned_burn`], but additionally validates the mint and
/// `decimals` — the real SPL Token `BurnChecked`-style safety check.
/// Instruction encoding (top-level discriminant `46`, sub-discriminant `2`
/// = `BurnChecked`, then `amount` as 8 raw little-endian bytes followed by
/// `decimals` as 1 byte) verified against `spl-token-2022-interface`'s real
/// `PermissionedBurnInstruction::BurnChecked`/`burn_checked`, cross-checked
/// against `pinocchio-token-2022`'s own
/// `extensions::permissioned_burn::BurnChecked::invoke_signed`.
pub fn permissioned_burn_checked(
    program: CpiHandle<'_>,
    source: CpiHandleMut<'_>,
    mint: CpiHandleMut<'_>,
    permissioned_burn_authority: CpiHandle<'_>,
    authority: CpiHandle<'_>,
    amount: u64,
    decimals: u8,
) -> Result<()> {
    permissioned_burn_checked_signed(
        program,
        source,
        mint,
        permissioned_burn_authority,
        authority,
        amount,
        decimals,
        &[],
    )
}

#[allow(clippy::too_many_arguments)]
pub fn permissioned_burn_checked_signed(
    program: CpiHandle<'_>,
    source: CpiHandleMut<'_>,
    mint: CpiHandleMut<'_>,
    permissioned_burn_authority: CpiHandle<'_>,
    authority: CpiHandle<'_>,
    amount: u64,
    decimals: u8,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = vec![46u8, 2u8];
        data.extend_from_slice(&amount.to_le_bytes());
        data.push(decimals);

        let ix = solana_program::instruction::Instruction {
            program_id: program.address(),
            accounts: vec![
                solana_program::instruction::AccountMeta::new(source.info.address(), false),
                solana_program::instruction::AccountMeta::new(mint.info.address(), false),
                solana_program::instruction::AccountMeta::new_readonly(
                    permissioned_burn_authority.info.address(),
                    true,
                ),
                solana_program::instruction::AccountMeta::new_readonly(
                    authority.info.address(),
                    true,
                ),
            ],
            data,
        };
        let accounts = [
            CpiHandle::from(source),
            CpiHandle::from(mint),
            permissioned_burn_authority,
            authority,
            program,
        ];
        crate::cpi::invoke_signed(&ix, &accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mut data = [0u8; 11];
        data[0] = 46;
        data[1] = 2;
        data[2..10].copy_from_slice(&amount.to_le_bytes());
        data[10] = decimals;

        let source_handle: CpiHandle<'_> = CpiHandle::from(source);
        let mint_handle: CpiHandle<'_> = CpiHandle::from(mint);
        let ix_accounts = [
            ::pinocchio::instruction::InstructionAccount::writable(
                source_handle.info.view.address(),
            ),
            ::pinocchio::instruction::InstructionAccount::writable(mint_handle.info.view.address()),
            ::pinocchio::instruction::InstructionAccount::readonly_signer(
                permissioned_burn_authority.info.view.address(),
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
        let handles = [source_handle, mint_handle, permissioned_burn_authority, authority];
        crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles, signer_seeds)
    }
}
