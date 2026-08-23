// ===========================================================================
// extensions/mod.rs — Token-2022 extension TLV parsing (shared machinery)
// ===========================================================================

//! # Token-2022 Extension Support
//!
//! Token-2022 extensions live in a Type-Length-Value (TLV) region appended
//! after an account's base data. Every extension region — for both a
//! 165-byte `TokenAccount` and an 82-byte `Mint` — begins at the same
//! absolute offset, 165: a `Mint` is zero-padded from its own end up to that
//! offset first, specifically so a mint's extension region can never be
//! mistaken for a token account's or vice versa. This module's byte offsets
//! and TLV-walk logic were verified against the real
//! `spl-token-2022-interface` crate's source before writing, not assumed —
//! see `naclac-token/docs/` for the specific file/line references checked.
//!
//! Only reachable through `InterfaceAccount<TokenAccount>`/
//! `InterfaceAccount<Mint>` (via `TokenInterfaceAccountExtensions`) —
//! extensions don't exist on accounts owned by the legacy Token program at
//! all, but `InterfaceAccount<T>` accepts either owner, so
//! `get_extension_from_info` re-checks for Token-2022 specifically rather
//! than trusting the caller.
//!
//! One file per extension type (`transfer_fee.rs`, `transfer_hook.rs`,
//! `permanent_delegate.rs`, `non_transferable.rs`, ...), each pulling the
//! shared `Extension` trait and byte-reading helpers defined here in via
//! `use super::{...}`.

use crate::prelude::{
    AccountInfo, Address, CpiHandle, InterfaceAccount, NaclacError, Owner, Pod, Result, Zeroable,
    TOKEN_2022_PROGRAM_ID,
};
use crate::token::{Mint, TokenAccount};
use crate::wrappers::ToAddress;

/// Every Token-2022 extension CPI in this module must call this first, on
/// whichever account it's about to invoke as the token program. Unlike
/// `Program<Token2022>`-typed instruction fields (checked at account-load
/// time before the handler runs), the `program: CpiHandle<'_>` these
/// functions take is a raw handle with no validation of its own — without
/// this, a caller whose own `#[derive(Accounts)]` struct under-constrains
/// that field (a bare `AccountInfo` instead of `Program<Token2022>`) would
/// let an attacker substitute a program that mimics Token-2022's
/// instruction-discriminant layout and silently "succeed" without doing
/// anything real, rather than the CPI failing loudly. Matches
/// `associated_token.rs`'s own `validate_ata_cpi_programs`, the existing
/// precedent for this check in this crate.
pub(crate) fn validate_token_2022_program(program: &CpiHandle<'_>) -> Result<()> {
    if program.address() != TOKEN_2022_PROGRAM_ID {
        return Err(NaclacError::ProgramIdMismatch.into());
    }
    Ok(())
}

pub mod cpi_guard;
pub mod default_account_state;
pub mod group_member_pointer;
pub mod group_pointer;
pub mod immutable_owner;
pub mod interest_bearing_mint;
pub mod memo_transfer;
pub mod metadata_pointer;
pub mod mint_close_authority;
pub mod non_transferable;
pub mod pausable;
pub mod permanent_delegate;
pub mod permissioned_burn;
pub mod scaled_ui_amount;
pub mod token_group;
pub mod token_metadata;
pub mod transfer_fee;
pub mod transfer_hook;

pub use cpi_guard::*;
pub use default_account_state::*;
pub use group_member_pointer::*;
pub use group_pointer::*;
pub use immutable_owner::*;
pub use interest_bearing_mint::*;
pub use memo_transfer::*;
pub use metadata_pointer::*;
pub use mint_close_authority::*;
pub use non_transferable::*;
pub use pausable::*;
pub use permanent_delegate::*;
pub use permissioned_burn::*;
pub use scaled_ui_amount::*;
pub use token_group::*;
pub use token_metadata::*;
pub use transfer_fee::*;
pub use transfer_hook::*;

/// Invokes an `Instruction` built by a real SPL interface crate's own
/// `instruction::*` constructor (`spl-token-metadata-interface`,
/// `spl-token-group-interface`) — data-only, backend-agnostic values with no
/// `AccountInfo` involved. Solana backend passes it straight to
/// `crate::cpi::invoke_signed` (the same type, since `solana_program::instruction::Instruction`
/// is a direct re-export of `solana_instruction::Instruction`). Pinocchio
/// backend converts each `AccountMeta` into pinocchio's own
/// `InstructionAccount` one-for-one (`AccountMeta::pubkey`,
/// `solana_pubkey::Pubkey`, and `pinocchio::address::Address` are the same
/// re-exported `solana_address::Address` type end to end — verified against
/// each crate's own source, not assumed) and calls
/// `crate::cpi::invoke_signed_pinocchio_handles` — the same pattern
/// `anchor-spl-v2`'s own pinocchio-native `token_2022_extensions` module
/// uses for these two interfaces.
#[cfg(not(feature = "pinocchio"))]
pub(crate) fn invoke_interface_instruction(
    ix: &solana_instruction::Instruction,
    accounts: &[crate::prelude::CpiHandle<'_>],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    crate::cpi::invoke_signed(ix, accounts, signer_seeds)
}

/// Extracts the raw address a real SPL interface crate's `instruction::*`
/// constructor expects (`solana_address::Address`, by value), from anything
/// with a `ToAddress` impl — `CpiHandle`/`CpiHandleMut` and their own `.info`
/// field (a plain `AccountInfo` in both backends, not a nested `CpiHandle`)
/// alike. Solana backend: naclac's own `Address` already *is* that type
/// (`crate::prelude::Address` is a direct re-export there), so this is a
/// no-op read. Pinocchio backend: naclac's own `Address` is instead its own
/// `#[repr(transparent)]` newtype (needed for pinocchio's no_std/zero-copy
/// account model), so this goes through `Address::as_address()`'s existing
/// verified-safe transmute to get the real `solana_address::Address` (named
/// `PinocchioAddress` in this crate's prelude) the interface crate's own
/// types actually need.
#[cfg(not(feature = "pinocchio"))]
pub(crate) fn ix_addr<T: crate::wrappers::ToAddress>(target: &T) -> crate::prelude::Address {
    target.address()
}

#[cfg(feature = "pinocchio")]
pub(crate) fn ix_addr<T: crate::wrappers::ToAddress>(
    target: &T,
) -> crate::prelude::PinocchioAddress {
    *target.address().as_address()
}

/// Same conversion as `ix_addr`, for an already-borrowed `&Address` (e.g. an
/// `Option<&Address>` authority parameter) rather than a `CpiHandle`.
#[cfg(not(feature = "pinocchio"))]
pub(crate) fn ix_addr_ref(addr: &crate::prelude::Address) -> crate::prelude::Address {
    *addr
}

#[cfg(feature = "pinocchio")]
pub(crate) fn ix_addr_ref(addr: &crate::prelude::Address) -> crate::prelude::PinocchioAddress {
    *addr.as_address()
}

#[cfg(feature = "pinocchio")]
pub(crate) fn invoke_interface_instruction(
    ix: &solana_instruction::Instruction,
    accounts: &[crate::prelude::CpiHandle<'_>],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let ix_accounts: crate::prelude::Vec<::pinocchio::instruction::InstructionAccount<'_>> = ix
        .accounts
        .iter()
        .map(|meta| {
            ::pinocchio::instruction::InstructionAccount::new(
                &meta.pubkey,
                meta.is_writable,
                meta.is_signer,
            )
        })
        .collect();
    let view = ::pinocchio::instruction::InstructionView {
        program_id: &ix.program_id,
        accounts: &ix_accounts,
        data: &ix.data,
    };
    crate::cpi::invoke_signed_pinocchio_handles(&view, accounts, signer_seeds)
}

/// Absolute byte offset every Token-2022 extension region begins at,
/// regardless of whether the base account is a 165-byte `TokenAccount` or an
/// 82-byte `Mint`. Verified against `spl-token-2022-interface`'s real
/// `type_and_tlv_indices`/`BASE_ACCOUNT_LENGTH` (`Account::LEN`, 165) — a
/// `Mint`'s bytes `[82..165)` are zero-padding, checked below, not
/// naclac-invented.
const EXTENSION_BASE_OFFSET: usize = 165;

/// Marker trait for a Token-2022 extension's Pod data layout. `TYPE` is the
/// TLV entry's 2-byte little-endian discriminant, matching
/// `spl-token-2022-interface::extension::ExtensionType`'s real discriminant
/// values (verified against that enum's source, a plain `#[repr(u16)]` with
/// no explicit overrides). `ACCOUNT_TYPE` is which base account kind this
/// extension can appear on (`Mint` = 1, `TokenAccount` = 2,
/// `spl-token-2022-interface::extension::AccountType`) — mint extensions and
/// account extensions share one TLV numbering space but are never valid on
/// the other kind, so a mismatch here is a malformed/spoofed account, not
/// just "extension absent".
pub trait Extension: Pod + Zeroable + Copy {
    const TYPE: u16;
    const ACCOUNT_TYPE: u8;
}

/// Walks `data`'s TLV extension region looking for an entry of type
/// `Ext::TYPE`, returning its raw value bytes. Mirrors
/// `spl-token-2022-interface`'s `get_extension_indices`/`get_extension_bytes`
/// walk: a 2-byte little-endian type, a 2-byte little-endian length, then
/// `length` value bytes, repeating until a `0` (`Uninitialized`) type marks
/// the end or the buffer runs out. Every bounds check here mirrors that
/// crate's real implementation, verified before writing this, not assumed.
fn find_extension_bytes<Ext: Extension>(
    data: &[u8],
    base_len: usize,
) -> core::result::Result<&[u8], NaclacError> {
    if data.len() <= EXTENSION_BASE_OFFSET {
        // No extensions at all — a plain, unextended account.
        return Err(NaclacError::ConstraintAccountIsNone);
    }
    if data[base_len..EXTENSION_BASE_OFFSET]
        .iter()
        .any(|&b| b != 0)
    {
        return Err(NaclacError::InvalidAccountDiscriminator);
    }
    if data[EXTENSION_BASE_OFFSET] != Ext::ACCOUNT_TYPE {
        return Err(NaclacError::InvalidAccountDiscriminator);
    }

    let tlv = &data[EXTENSION_BASE_OFFSET + 1..];
    let mut offset = 0usize;
    while offset < tlv.len() {
        if tlv.len() < offset + 4 {
            return Err(NaclacError::InvalidAccountDiscriminator);
        }
        let entry_type = u16::from_le_bytes(tlv[offset..offset + 2].try_into().unwrap());
        if entry_type == 0 {
            // Uninitialized marker: no more entries were ever written here.
            return Err(NaclacError::ConstraintAccountIsNone);
        }
        let entry_len =
            u16::from_le_bytes(tlv[offset + 2..offset + 4].try_into().unwrap()) as usize;
        let value_start = offset + 4;
        let value_end = value_start
            .checked_add(entry_len)
            .ok_or(NaclacError::InvalidAccountDiscriminator)?;
        if value_end > tlv.len() {
            return Err(NaclacError::InvalidAccountDiscriminator);
        }
        if entry_type == Ext::TYPE {
            return Ok(&tlv[value_start..value_end]);
        }
        offset = value_end;
    }
    Err(NaclacError::ConstraintAccountIsNone)
}

/// Finds and copies out the extension `Ext`, verifying its TLV entry is
/// exactly `size_of::<Ext>()` bytes before reinterpreting it. Copies rather
/// than returning a reference into `data`, since the caller only ever holds
/// a short-lived borrow of the account's data (see
/// `TokenInterfaceAccountExtensions`) — every `Extension` type here is a
/// small, `Copy` value, so this has no meaningful cost.
fn get_extension<Ext: Extension>(
    data: &[u8],
    base_len: usize,
) -> core::result::Result<Ext, NaclacError> {
    let bytes = find_extension_bytes::<Ext>(data, base_len)?;
    if bytes.len() != core::mem::size_of::<Ext>() {
        return Err(NaclacError::InvalidAccountDiscriminator);
    }
    // SAFETY: `Ext: Pod` guarantees any bit pattern is a valid `Ext`, and
    // `bytes.len() == size_of::<Ext>()` was just checked. `read_unaligned`
    // (not a plain dereference) since a TLV value's offset within the
    // account buffer has no alignment guarantee.
    Ok(unsafe { core::ptr::read_unaligned(bytes.as_ptr() as *const Ext) })
}

fn get_extension_from_info<Ext: Extension>(info: &AccountInfo, base_len: usize) -> Result<Ext> {
    // Extensions only exist on Token-2022 accounts — `InterfaceAccount<T>`
    // itself accepts either Token or Token-2022 ownership, so this re-checks
    // specifically for Token-2022 rather than trusting the caller.
    if Owner::program_owner(info) != TOKEN_2022_PROGRAM_ID {
        return Err(NaclacError::ConstraintOwner.into());
    }
    #[cfg(not(feature = "pinocchio"))]
    {
        let data = info.try_borrow_data()?;
        get_extension::<Ext>(&data, base_len).map_err(|e| e.into())
    }
    #[cfg(feature = "pinocchio")]
    {
        get_extension::<Ext>(info.data(), base_len).map_err(|e| e.into())
    }
}

/// Reads a Token-2022 extension off an already-loaded `InterfaceAccount`.
pub trait TokenInterfaceAccountExtensions {
    fn get_extension<Ext: Extension>(&self) -> Result<Ext>;
}

impl TokenInterfaceAccountExtensions for InterfaceAccount<TokenAccount> {
    fn get_extension<Ext: Extension>(&self) -> Result<Ext> {
        get_extension_from_info(&self.info, 165)
    }
}

impl TokenInterfaceAccountExtensions for InterfaceAccount<Mint> {
    fn get_extension<Ext: Extension>(&self) -> Result<Ext> {
        get_extension_from_info(&self.info, 82)
    }
}

/// Shared little-endian byte-reading helpers for TLV extension values —
/// every extension file in this directory reads its fields through these
/// rather than typed `#[repr(C)]` struct fields, since TLV value offsets
/// have no alignment guarantee and a native multi-byte field would risk
/// unaligned-access UB.
fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}
fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}
fn read_optional_address(bytes: &[u8], offset: usize) -> Option<Address> {
    let raw: [u8; 32] = bytes[offset..offset + 32].try_into().unwrap();
    if raw == [0u8; 32] {
        None
    } else {
        Some(Address::new_from_array(raw))
    }
}
fn ceil_div_u128(numerator: u128, denominator: u128) -> Option<u128> {
    numerator
        .checked_add(denominator)?
        .checked_sub(1)?
        .checked_div(denominator)
}
