// ===========================================================================
// extensions/token_metadata.rs — TokenMetadata (spl-token-metadata-interface)
// ===========================================================================

//! CPI wrappers for the `spl-token-metadata-interface` instructions
//! Token-2022 processes directly against a `MetadataPointer`-linked metadata
//! account: `Initialize`, `UpdateField`, `RemoveKey`, `UpdateAuthority`,
//! `Emit`. Each instruction's account list is built here directly, and its
//! data payload uses `spl-token-metadata-interface`'s own real
//! `Initialize`/`UpdateField`/`RemoveKey`/`UpdateAuthority`/`Emit` structs and
//! `SPL_DISCRIMINATOR_SLICE` constants, Borsh-serialized via
//! `pack_metadata_instruction` below rather than that crate's own
//! `instruction::*` constructor functions or `TokenMetadataInstruction::pack()`
//! — both panic in a way that requires `Debug` and pulls `core::fmt` into the
//! on-chain binary (see `docs/naclac_token_audit_progress.md` finding #3).
//!
//! `UpdateField`/`RemoveKey` may grow or shrink the metadata account;
//! Token-2022's own processor (`extension::token_metadata::processor`)
//! reallocs it directly via `AccountInfo::resize` as part of handling the
//! instruction — callers don't pass a payer or `system_program` to any of
//! these CPIs, and none is needed. `resize` only changes the account's data
//! length, not its lamports, and Solana only checks rent-exemption once, at
//! the end of the transaction — so the account must already hold enough
//! lamports to stay rent-exempt at whatever size it may grow to, funded
//! up front (e.g. by over-funding at `create_account` time), since there's
//! no other lamport source available mid-instruction here.
//!
//! Both backends: `pinocchio-token-2022` itself covers only the
//! `MetadataPointer` extension (not the content instructions it points at),
//! but every instruction here is built as a data-only `Instruction` value
//! with no `AccountInfo` involved, so `super::invoke_interface_instruction`
//! (`extensions/mod.rs`) can dispatch it under either backend — see that
//! function's own doc comment for how the pinocchio conversion is verified
//! safe. Beyond the official `anchor-spl` crate's own
//! `token_2022_extensions::token_metadata` module (which exposes only
//! `initialize`/`update_authority`/`update_field`), this also covers
//! `RemoveKey`/`Emit`, both real, separately-dispatched interface
//! instructions it omits.

use crate::prelude::{Address, CpiHandle, CpiHandleMut, Result, String};
pub use spl_token_metadata_interface::state::Field;

/// Borsh-serializes `data`, prefixed by its real `SPL_DISCRIMINATOR_SLICE` —
/// replicates exactly what `spl_token_metadata_interface::instruction::TokenMetadataInstruction::pack()`
/// produces, without going through it: that function calls
/// `borsh::to_vec(data).unwrap()` internally, which requires `Debug` on
/// borsh's `io::Error` and pulls `core::fmt` into the on-chain binary (see
/// `docs/naclac_token_audit_progress.md` finding #3). `data`'s own real,
/// unmodified struct and derived `BorshSerialize` impl are used here — only
/// the panic path changes.
fn pack_metadata_instruction<T>(data: &T) -> crate::prelude::Vec<u8>
where
    T: borsh::BorshSerialize + spl_discriminator::SplDiscriminate,
{
    let mut buf = crate::prelude::Vec::from(T::SPL_DISCRIMINATOR_SLICE);
    buf.extend_from_slice(
        &borsh::to_vec(data).unwrap_or_else(|_| panic!("failed to serialize instruction data")),
    );
    buf
}

/// Bundles `TokenMetadata::Initialize`'s three string fields — keeps
/// `initialize_token_metadata(_signed)` under `clippy::too_many_arguments`
/// alongside its five account handles, the same fix `CheckedTransferParams`
/// (`token.rs`) applies to `transfer_checked`'s own instruction data.
pub struct TokenMetadataInitializeParams {
    pub name: String,
    pub symbol: String,
    pub uri: String,
}

/// Initializes a `TokenMetadata` TLV entry on an already-allocated,
/// rent-exempt metadata account (the mint itself, when `MetadataPointer`
/// points at it — see `initialize_metadata_pointer` in `metadata_pointer.rs`).
/// Must be called after `initialize_mint`/`initialize_mint_signed`, since it
/// assumes the mint it references is already initialized.
pub fn initialize_token_metadata(
    program: CpiHandle<'_>,
    metadata: CpiHandleMut<'_>,
    update_authority: CpiHandle<'_>,
    mint: CpiHandle<'_>,
    mint_authority: CpiHandle<'_>,
    params: TokenMetadataInitializeParams,
) -> Result<()> {
    initialize_token_metadata_signed(
        program,
        metadata,
        update_authority,
        mint,
        mint_authority,
        params,
        &[],
    )
}

pub fn initialize_token_metadata_signed(
    program: CpiHandle<'_>,
    metadata: CpiHandleMut<'_>,
    update_authority: CpiHandle<'_>,
    mint: CpiHandle<'_>,
    mint_authority: CpiHandle<'_>,
    params: TokenMetadataInitializeParams,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    let TokenMetadataInitializeParams { name, symbol, uri } = params;
    let data = spl_token_metadata_interface::instruction::Initialize { name, symbol, uri };
    let ix = solana_instruction::Instruction {
        program_id: super::ix_addr(&program),
        accounts: crate::prelude::vec![
            solana_instruction::AccountMeta::new(super::ix_addr(&metadata.info), false),
            solana_instruction::AccountMeta::new_readonly(super::ix_addr(&update_authority), false),
            solana_instruction::AccountMeta::new_readonly(super::ix_addr(&mint), false),
            solana_instruction::AccountMeta::new_readonly(super::ix_addr(&mint_authority), true),
        ],
        data: pack_metadata_instruction(&data),
    };
    let accounts = [
        CpiHandle::from(metadata),
        update_authority,
        mint,
        mint_authority,
        program,
    ];
    super::invoke_interface_instruction(&ix, &accounts, signer_seeds)
}

/// Overwrites (or creates, if absent) one field of an already-initialized
/// `TokenMetadata` entry. `field` selects `name`/`symbol`/`uri`, or an
/// arbitrary `additional_metadata` key via `Field::Key`.
pub fn update_token_metadata_field(
    program: CpiHandle<'_>,
    metadata: CpiHandleMut<'_>,
    update_authority: CpiHandle<'_>,
    field: Field,
    value: String,
) -> Result<()> {
    update_token_metadata_field_signed(program, metadata, update_authority, field, value, &[])
}

pub fn update_token_metadata_field_signed(
    program: CpiHandle<'_>,
    metadata: CpiHandleMut<'_>,
    update_authority: CpiHandle<'_>,
    field: Field,
    value: String,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    let data = spl_token_metadata_interface::instruction::UpdateField { field, value };
    let ix = solana_instruction::Instruction {
        program_id: super::ix_addr(&program),
        accounts: crate::prelude::vec![
            solana_instruction::AccountMeta::new(super::ix_addr(&metadata.info), false),
            solana_instruction::AccountMeta::new_readonly(super::ix_addr(&update_authority), true),
        ],
        data: pack_metadata_instruction(&data),
    };
    let accounts = [CpiHandle::from(metadata), update_authority, program];
    super::invoke_interface_instruction(&ix, &accounts, signer_seeds)
}

/// Removes a key from `TokenMetadata`'s `additional_metadata`. Only applies
/// to additional keys, not the base `name`/`symbol`/`uri` fields. If
/// `idempotent` is `false`, the underlying instruction errors when `key`
/// isn't present.
pub fn remove_token_metadata_key(
    program: CpiHandle<'_>,
    metadata: CpiHandleMut<'_>,
    update_authority: CpiHandle<'_>,
    key: String,
    idempotent: bool,
) -> Result<()> {
    remove_token_metadata_key_signed(program, metadata, update_authority, key, idempotent, &[])
}

pub fn remove_token_metadata_key_signed(
    program: CpiHandle<'_>,
    metadata: CpiHandleMut<'_>,
    update_authority: CpiHandle<'_>,
    key: String,
    idempotent: bool,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    let data = spl_token_metadata_interface::instruction::RemoveKey { idempotent, key };
    let ix = solana_instruction::Instruction {
        program_id: super::ix_addr(&program),
        accounts: crate::prelude::vec![
            solana_instruction::AccountMeta::new(super::ix_addr(&metadata.info), false),
            solana_instruction::AccountMeta::new_readonly(super::ix_addr(&update_authority), true),
        ],
        data: pack_metadata_instruction(&data),
    };
    let accounts = [CpiHandle::from(metadata), update_authority, program];
    super::invoke_interface_instruction(&ix, &accounts, signer_seeds)
}

/// Updates `TokenMetadata`'s update authority. `new_authority` of `None`
/// permanently removes update authority from the metadata.
pub fn update_token_metadata_authority(
    program: CpiHandle<'_>,
    metadata: CpiHandleMut<'_>,
    current_authority: CpiHandle<'_>,
    new_authority: Option<&Address>,
) -> Result<()> {
    update_token_metadata_authority_signed(program, metadata, current_authority, new_authority, &[])
}

pub fn update_token_metadata_authority_signed(
    program: CpiHandle<'_>,
    metadata: CpiHandleMut<'_>,
    current_authority: CpiHandle<'_>,
    new_authority: Option<&Address>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    let new_authority = solana_nullable::MaybeNull::try_from(new_authority.map(super::ix_addr_ref))
        .map_err(|_| crate::prelude::NaclacError::InvalidAccountDiscriminator)?;
    let data = spl_token_metadata_interface::instruction::UpdateAuthority { new_authority };
    let ix = solana_instruction::Instruction {
        program_id: super::ix_addr(&program),
        accounts: crate::prelude::vec![
            solana_instruction::AccountMeta::new(super::ix_addr(&metadata.info), false),
            solana_instruction::AccountMeta::new_readonly(super::ix_addr(&current_authority), true),
        ],
        data: pack_metadata_instruction(&data),
    };
    let accounts = [CpiHandle::from(metadata), current_authority, program];
    super::invoke_interface_instruction(&ix, &accounts, signer_seeds)
}

/// Emits `TokenMetadata` as return data (`set_return_data`), scoped to the
/// `[start, end)` byte range of its Borsh encoding when given. Requires no
/// signer — `metadata` is read-only.
pub fn emit_token_metadata(
    program: CpiHandle<'_>,
    metadata: CpiHandle<'_>,
    start: Option<u64>,
    end: Option<u64>,
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    let data = spl_token_metadata_interface::instruction::Emit { start, end };
    let ix = solana_instruction::Instruction {
        program_id: super::ix_addr(&program),
        accounts: crate::prelude::vec![solana_instruction::AccountMeta::new_readonly(
            super::ix_addr(&metadata),
            false
        )],
        data: pack_metadata_instruction(&data),
    };
    let accounts = [metadata, program];
    super::invoke_interface_instruction(&ix, &accounts, &[])
}
