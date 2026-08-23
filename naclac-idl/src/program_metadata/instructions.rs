//! Instruction builders for the Program Metadata Program, hand-built to
//! match its real on-chain byte layout exactly (see the parent module's
//! docs for why this isn't built on the program's own generated Rust
//! client). Every instruction's account order and instruction-data layout
//! is verified directly against `solana-program/program-metadata`'s own
//! processor source and test suite, not inferred from documentation.

use solana_address::Address;
use solana_instruction::{AccountMeta, Instruction};

use super::{Seed, PROGRAM_METADATA_ID, RENT_SYSVAR_ID};

/// The on-chain program's `encoding` instruction-data byte values (mirrors
/// `program-metadata`'s own `state::Encoding` enum ordinals).
pub mod encoding {
    pub const NONE: u8 = 0;
    pub const UTF8: u8 = 1;
    pub const BASE58: u8 = 2;
    pub const BASE64: u8 = 3;
}

/// The on-chain program's `compression` instruction-data byte values
/// (mirrors `program-metadata`'s own `state::Compression` enum ordinals).
pub mod compression {
    pub const NONE: u8 = 0;
    pub const GZIP: u8 = 1;
    pub const ZLIB: u8 = 2;
}

/// The on-chain program's `format` instruction-data byte values (mirrors
/// `program-metadata`'s own `state::Format` enum ordinals).
pub mod format {
    pub const NONE: u8 = 0;
    pub const JSON: u8 = 1;
    pub const YAML: u8 = 2;
    pub const TOML: u8 = 3;
}

/// The on-chain program's `data_source` instruction-data byte values
/// (mirrors `program-metadata`'s own `state::DataSource` enum ordinals).
pub mod data_source {
    pub const DIRECT: u8 = 0;
    pub const URL: u8 = 1;
    pub const EXTERNAL: u8 = 2;
}

const WRITE_DISCRIMINATOR: u8 = 0;
const INITIALIZE_DISCRIMINATOR: u8 = 1;
const SET_AUTHORITY_DISCRIMINATOR: u8 = 2;
const SET_DATA_DISCRIMINATOR: u8 = 3;
const SET_IMMUTABLE_DISCRIMINATOR: u8 = 4;
const TRIM_DISCRIMINATOR: u8 = 5;
const CLOSE_DISCRIMINATOR: u8 = 6;
const ALLOCATE_DISCRIMINATOR: u8 = 7;

/// The account group [`initialize`] and [`set_data`] both need: the
/// metadata account itself, its authority, and the program + program-data
/// accounts the on-chain program uses to verify that authority.
pub struct MetadataAccounts<'a> {
    pub metadata: &'a Address,
    pub authority: &'a Address,
    pub program: &'a Address,
    pub program_data: &'a Address,
}

/// The data half of a [`set_data`] call — the on-chain program only accepts
/// exactly these three shapes (any other combination of "inline data" +
/// "buffer account" is rejected), so this is an enum rather than loose
/// `Option` parameters to make the invalid combinations unrepresentable.
pub enum SetDataSource<'a> {
    /// Leave the existing data untouched — only `encoding`/`compression`/
    /// `format` on the header are updated.
    Unchanged,
    /// Replace the data inline (small payloads that fit in one transaction).
    Inline { data_source: u8, data: &'a [u8] },
    /// Replace the data by copying from an already `allocate`/[`write`]-
    /// populated buffer account (large payloads).
    FromBuffer { data_source: u8, buffer: Address },
}

/// Builds an `Allocate` instruction — creates a PDA `Buffer` account at
/// `buffer` (the same address a matching [`initialize`] call will later
/// finalize), ready to receive [`write`] calls.
pub fn allocate(
    buffer: &Address,
    authority: &Address,
    program: &Address,
    program_data: &Address,
    seed: &Seed,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*buffer, false),
        AccountMeta::new_readonly(*authority, true),
        AccountMeta::new_readonly(*program, false),
        AccountMeta::new_readonly(*program_data, false),
        AccountMeta::new_readonly(solana_system_interface::program::id(), false),
    ];

    let mut data = Vec::with_capacity(1 + seed.len());
    data.push(ALLOCATE_DISCRIMINATOR);
    data.extend_from_slice(seed);

    Instruction {
        program_id: PROGRAM_METADATA_ID,
        accounts,
        data,
    }
}

/// Builds a `Write` instruction — writes `chunk` at `offset` into a `Buffer`
/// account previously created by [`allocate`]. Always passes bytes directly
/// (never a `source_buffer` account to copy from — the third account slot is
/// filled with [`PROGRAM_METADATA_ID`] itself, this program's own documented
/// placeholder for "account omitted").
pub fn write(buffer: &Address, authority: &Address, offset: u32, chunk: &[u8]) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*buffer, false),
        AccountMeta::new_readonly(*authority, true),
        AccountMeta::new_readonly(PROGRAM_METADATA_ID, false),
    ];

    let mut data = Vec::with_capacity(5 + chunk.len());
    data.push(WRITE_DISCRIMINATOR);
    data.extend_from_slice(&offset.to_le_bytes());
    data.extend_from_slice(chunk);

    Instruction {
        program_id: PROGRAM_METADATA_ID,
        accounts,
        data,
    }
}

/// Builds an `Initialize` instruction. Pass `data = Some(bytes)` to create a
/// metadata account directly with inline data (only for payloads that fit
/// in one transaction); pass `data = None` to finalize an already-written
/// [`allocate`]/[`write`]-populated buffer into a metadata account instead
/// (the on-chain program rejects inline data in that case, since the bytes
/// are already there).
pub fn initialize(
    accounts: MetadataAccounts<'_>,
    seed: &Seed,
    encoding: u8,
    compression: u8,
    format: u8,
    data_source: u8,
    data: Option<&[u8]>,
) -> Instruction {
    let account_metas = vec![
        AccountMeta::new(*accounts.metadata, false),
        AccountMeta::new_readonly(*accounts.authority, true),
        AccountMeta::new_readonly(*accounts.program, false),
        AccountMeta::new_readonly(*accounts.program_data, false),
        AccountMeta::new_readonly(solana_system_interface::program::id(), false),
    ];

    let mut instruction_data = Vec::with_capacity(21 + data.map_or(0, <[u8]>::len));
    instruction_data.push(INITIALIZE_DISCRIMINATOR);
    instruction_data.extend_from_slice(seed);
    instruction_data.push(encoding);
    instruction_data.push(compression);
    instruction_data.push(format);
    instruction_data.push(data_source);
    if let Some(data) = data {
        instruction_data.extend_from_slice(data);
    }

    Instruction {
        program_id: PROGRAM_METADATA_ID,
        accounts: account_metas,
        data: instruction_data,
    }
}

/// Builds a `SetData` instruction — updates an already-`initialize`d
/// metadata account. The on-chain program resizes the account automatically
/// to fit the new data, growing or shrinking as needed; no separate
/// `extend`/`trim` call is required around this. This is the instruction a
/// program's normal rebuild-redeploy-reupload loop uses after the first
/// [`initialize`], not `initialize` again (which only ever succeeds once
/// per metadata account).
pub fn set_data(
    accounts: MetadataAccounts<'_>,
    encoding: u8,
    compression: u8,
    format: u8,
    source: SetDataSource<'_>,
) -> Instruction {
    let buffer = match &source {
        SetDataSource::FromBuffer { buffer, .. } => *buffer,
        SetDataSource::Unchanged | SetDataSource::Inline { .. } => PROGRAM_METADATA_ID,
    };

    let account_metas = vec![
        AccountMeta::new(*accounts.metadata, false),
        AccountMeta::new_readonly(*accounts.authority, true),
        AccountMeta::new_readonly(buffer, false),
        AccountMeta::new_readonly(*accounts.program, false),
        AccountMeta::new_readonly(*accounts.program_data, false),
    ];

    let mut instruction_data = vec![SET_DATA_DISCRIMINATOR, encoding, compression, format];
    match source {
        SetDataSource::Unchanged => {}
        SetDataSource::Inline { data_source, data } => {
            instruction_data.push(data_source);
            instruction_data.extend_from_slice(data);
        }
        SetDataSource::FromBuffer { data_source, .. } => {
            instruction_data.push(data_source);
        }
    }

    Instruction {
        program_id: PROGRAM_METADATA_ID,
        accounts: account_metas,
        data: instruction_data,
    }
}

/// Builds a `SetAuthority` instruction — changes (`Some`) or removes
/// (`None`) a buffer or metadata account's authority. `account` may be
/// either kind. A non-canonical metadata account's authority can never be
/// changed (its address derivation depends on the authority that created
/// it, so the on-chain program rejects the attempt).
pub fn set_authority(
    account: &Address,
    authority: &Address,
    program: &Address,
    program_data: &Address,
    new_authority: Option<&Address>,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*account, false),
        AccountMeta::new_readonly(*authority, true),
        AccountMeta::new_readonly(*program, false),
        AccountMeta::new_readonly(*program_data, false),
    ];

    let mut data = vec![SET_AUTHORITY_DISCRIMINATOR, new_authority.is_some() as u8];
    if let Some(new_authority) = new_authority {
        data.extend_from_slice(new_authority.as_ref());
    }

    Instruction {
        program_id: PROGRAM_METADATA_ID,
        accounts,
        data,
    }
}

/// Builds a `SetImmutable` instruction — permanently locks a metadata
/// account against any further [`set_data`]/[`set_authority`]/[`trim`]/
/// [`close`] calls. Irreversible on-chain.
pub fn set_immutable(
    metadata: &Address,
    authority: &Address,
    program: &Address,
    program_data: &Address,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*metadata, false),
        AccountMeta::new_readonly(*authority, true),
        AccountMeta::new_readonly(*program, false),
        AccountMeta::new_readonly(*program_data, false),
    ];

    Instruction {
        program_id: PROGRAM_METADATA_ID,
        accounts,
        data: vec![SET_IMMUTABLE_DISCRIMINATOR],
    }
}

/// Builds a `Trim` instruction — resizes a buffer or metadata account down
/// to the minimum size needed to stay rent-exempt, refunding the freed
/// lamports to `destination`. Distinct from [`set_data`]'s own automatic
/// resize: that adjusts the account's byte length for its new data, but
/// never withdraws the resulting excess lamports on its own.
pub fn trim(
    account: &Address,
    authority: &Address,
    program: &Address,
    program_data: &Address,
    destination: &Address,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*account, false),
        AccountMeta::new_readonly(*authority, true),
        AccountMeta::new_readonly(*program, false),
        AccountMeta::new_readonly(*program_data, false),
        AccountMeta::new(*destination, false),
        AccountMeta::new_readonly(RENT_SYSVAR_ID, false),
    ];

    Instruction {
        program_id: PROGRAM_METADATA_ID,
        accounts,
        data: vec![TRIM_DISCRIMINATOR],
    }
}

/// Builds a `Close` instruction — permanently deletes a buffer or metadata
/// account, transferring its full lamport balance to `destination`.
pub fn close(
    account: &Address,
    authority: &Address,
    program: &Address,
    program_data: &Address,
    destination: &Address,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*account, false),
        AccountMeta::new_readonly(*authority, true),
        AccountMeta::new_readonly(*program, false),
        AccountMeta::new_readonly(*program_data, false),
        AccountMeta::new(*destination, false),
    ];

    Instruction {
        program_id: PROGRAM_METADATA_ID,
        accounts,
        data: vec![CLOSE_DISCRIMINATOR],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::program_metadata::seed_from_str;

    fn addr(byte: u8) -> Address {
        Address::new_from_array([byte; 32])
    }

    #[test]
    fn allocate_matches_expected_wire_format() {
        let buffer = addr(1);
        let authority = addr(2);
        let program = addr(3);
        let program_data = addr(4);
        let seed = seed_from_str("idl");

        let ix = allocate(&buffer, &authority, &program, &program_data, &seed);

        assert_eq!(ix.program_id, PROGRAM_METADATA_ID);
        assert_eq!(ix.accounts.len(), 5);
        assert_eq!(ix.accounts[0], AccountMeta::new(buffer, false));
        assert_eq!(ix.accounts[1], AccountMeta::new_readonly(authority, true));
        assert_eq!(ix.accounts[2], AccountMeta::new_readonly(program, false));
        assert_eq!(
            ix.accounts[3],
            AccountMeta::new_readonly(program_data, false)
        );
        assert_eq!(
            ix.accounts[4],
            AccountMeta::new_readonly(solana_system_interface::program::id(), false)
        );

        // discriminator(1) + raw seed bytes(16), no length prefix, no Some/None tag.
        let mut expected = vec![ALLOCATE_DISCRIMINATOR];
        expected.extend_from_slice(&seed);
        assert_eq!(ix.data, expected);
    }

    #[test]
    fn write_matches_expected_wire_format() {
        let buffer = addr(1);
        let authority = addr(2);
        let chunk = b"hello world";

        let ix = write(&buffer, &authority, 96, chunk);

        assert_eq!(ix.program_id, PROGRAM_METADATA_ID);
        assert_eq!(ix.accounts.len(), 3);
        assert_eq!(ix.accounts[0], AccountMeta::new(buffer, false));
        assert_eq!(ix.accounts[1], AccountMeta::new_readonly(authority, true));
        assert_eq!(
            ix.accounts[2],
            AccountMeta::new_readonly(PROGRAM_METADATA_ID, false)
        );

        // discriminator(1) + offset(4, LE) + raw chunk bytes, no length prefix.
        let mut expected = vec![WRITE_DISCRIMINATOR];
        expected.extend_from_slice(&96u32.to_le_bytes());
        expected.extend_from_slice(chunk);
        assert_eq!(ix.data, expected);
    }

    #[test]
    fn initialize_with_inline_data_matches_expected_wire_format() {
        let metadata = addr(1);
        let authority = addr(2);
        let program = addr(3);
        let program_data = addr(4);
        let seed = seed_from_str("idl");
        let payload = b"{}";

        let ix = initialize(
            MetadataAccounts {
                metadata: &metadata,
                authority: &authority,
                program: &program,
                program_data: &program_data,
            },
            &seed,
            encoding::UTF8,
            compression::ZLIB,
            format::JSON,
            data_source::DIRECT,
            Some(payload),
        );

        assert_eq!(ix.program_id, PROGRAM_METADATA_ID);
        assert_eq!(ix.accounts.len(), 5);
        assert_eq!(ix.accounts[0], AccountMeta::new(metadata, false));
        assert_eq!(ix.accounts[1], AccountMeta::new_readonly(authority, true));
        assert_eq!(ix.accounts[2], AccountMeta::new_readonly(program, false));
        assert_eq!(
            ix.accounts[3],
            AccountMeta::new_readonly(program_data, false)
        );
        assert_eq!(
            ix.accounts[4],
            AccountMeta::new_readonly(solana_system_interface::program::id(), false)
        );

        // discriminator(1) + seed(16) + encoding(1) + compression(1) + format(1)
        // + data_source(1) + raw payload bytes, no length prefix.
        let mut expected = vec![INITIALIZE_DISCRIMINATOR];
        expected.extend_from_slice(&seed);
        expected.push(encoding::UTF8);
        expected.push(compression::ZLIB);
        expected.push(format::JSON);
        expected.push(data_source::DIRECT);
        expected.extend_from_slice(payload);
        assert_eq!(ix.data, expected);
    }

    #[test]
    fn initialize_finalizing_a_buffer_has_no_trailing_data() {
        let metadata = addr(1);
        let authority = addr(2);
        let program = addr(3);
        let program_data = addr(4);
        let seed = seed_from_str("idl");

        let ix = initialize(
            MetadataAccounts {
                metadata: &metadata,
                authority: &authority,
                program: &program,
                program_data: &program_data,
            },
            &seed,
            encoding::UTF8,
            compression::ZLIB,
            format::JSON,
            data_source::DIRECT,
            None,
        );

        // Exactly the 21-byte fixed header, nothing appended.
        assert_eq!(ix.data.len(), 21);
    }

    #[test]
    fn set_data_inline_matches_expected_wire_format() {
        let metadata = addr(1);
        let authority = addr(2);
        let program = addr(3);
        let program_data = addr(4);
        let payload = b"{}";

        let ix = set_data(
            MetadataAccounts {
                metadata: &metadata,
                authority: &authority,
                program: &program,
                program_data: &program_data,
            },
            encoding::UTF8,
            compression::ZLIB,
            format::JSON,
            SetDataSource::Inline {
                data_source: data_source::DIRECT,
                data: payload,
            },
        );

        assert_eq!(ix.accounts.len(), 5);
        assert_eq!(ix.accounts[0], AccountMeta::new(metadata, false));
        assert_eq!(ix.accounts[1], AccountMeta::new_readonly(authority, true));
        // No buffer account used in inline mode — "not provided" placeholder.
        assert_eq!(
            ix.accounts[2],
            AccountMeta::new_readonly(PROGRAM_METADATA_ID, false)
        );
        assert_eq!(ix.accounts[3], AccountMeta::new_readonly(program, false));
        assert_eq!(
            ix.accounts[4],
            AccountMeta::new_readonly(program_data, false)
        );

        // discriminator(1) + encoding(1) + compression(1) + format(1)
        // + data_source(1) + raw payload bytes, no length prefix.
        let mut expected = vec![
            SET_DATA_DISCRIMINATOR,
            encoding::UTF8,
            compression::ZLIB,
            format::JSON,
            data_source::DIRECT,
        ];
        expected.extend_from_slice(payload);
        assert_eq!(ix.data, expected);
    }

    #[test]
    fn set_data_from_buffer_matches_expected_wire_format() {
        let metadata = addr(1);
        let authority = addr(2);
        let program = addr(3);
        let program_data = addr(4);
        let buffer = addr(5);

        let ix = set_data(
            MetadataAccounts {
                metadata: &metadata,
                authority: &authority,
                program: &program,
                program_data: &program_data,
            },
            encoding::UTF8,
            compression::ZLIB,
            format::JSON,
            SetDataSource::FromBuffer {
                data_source: data_source::DIRECT,
                buffer,
            },
        );

        assert_eq!(ix.accounts[2], AccountMeta::new_readonly(buffer, false));

        // discriminator(1) + encoding(1) + compression(1) + format(1)
        // + data_source(1), no trailing data — it's read from `buffer` on-chain.
        let expected = vec![
            SET_DATA_DISCRIMINATOR,
            encoding::UTF8,
            compression::ZLIB,
            format::JSON,
            data_source::DIRECT,
        ];
        assert_eq!(ix.data, expected);
    }

    #[test]
    fn set_data_unchanged_only_updates_header_fields() {
        let metadata = addr(1);
        let authority = addr(2);
        let program = addr(3);
        let program_data = addr(4);

        let ix = set_data(
            MetadataAccounts {
                metadata: &metadata,
                authority: &authority,
                program: &program,
                program_data: &program_data,
            },
            encoding::UTF8,
            compression::ZLIB,
            format::JSON,
            SetDataSource::Unchanged,
        );

        // Exactly the 4-byte fixed header (discriminator + encoding +
        // compression + format), no data_source, no trailing data.
        assert_eq!(
            ix.data,
            vec![
                SET_DATA_DISCRIMINATOR,
                encoding::UTF8,
                compression::ZLIB,
                format::JSON
            ]
        );
    }

    #[test]
    fn set_authority_to_a_new_authority_matches_expected_wire_format() {
        let account = addr(1);
        let authority = addr(2);
        let program = addr(3);
        let program_data = addr(4);
        let new_authority = addr(5);

        let ix = set_authority(
            &account,
            &authority,
            &program,
            &program_data,
            Some(&new_authority),
        );

        assert_eq!(ix.accounts.len(), 4);
        assert_eq!(ix.accounts[0], AccountMeta::new(account, false));
        assert_eq!(ix.accounts[1], AccountMeta::new_readonly(authority, true));
        assert_eq!(ix.accounts[2], AccountMeta::new_readonly(program, false));
        assert_eq!(
            ix.accounts[3],
            AccountMeta::new_readonly(program_data, false)
        );

        // discriminator(1) + has_new_authority(1, =1) + raw new authority bytes(32).
        let mut expected = vec![SET_AUTHORITY_DISCRIMINATOR, 1];
        expected.extend_from_slice(new_authority.as_ref());
        assert_eq!(ix.data, expected);
    }

    #[test]
    fn set_authority_removal_has_no_trailing_address() {
        let account = addr(1);
        let authority = addr(2);
        let program = addr(3);
        let program_data = addr(4);

        let ix = set_authority(&account, &authority, &program, &program_data, None);

        // discriminator(1) + has_new_authority(1, =0), nothing else.
        assert_eq!(ix.data, vec![SET_AUTHORITY_DISCRIMINATOR, 0]);
    }

    #[test]
    fn set_immutable_matches_expected_wire_format() {
        let metadata = addr(1);
        let authority = addr(2);
        let program = addr(3);
        let program_data = addr(4);

        let ix = set_immutable(&metadata, &authority, &program, &program_data);

        assert_eq!(ix.accounts.len(), 4);
        assert_eq!(ix.accounts[0], AccountMeta::new(metadata, false));
        assert_eq!(ix.accounts[1], AccountMeta::new_readonly(authority, true));
        assert_eq!(ix.accounts[2], AccountMeta::new_readonly(program, false));
        assert_eq!(
            ix.accounts[3],
            AccountMeta::new_readonly(program_data, false)
        );
        assert_eq!(ix.data, vec![SET_IMMUTABLE_DISCRIMINATOR]);
    }

    #[test]
    fn trim_matches_expected_wire_format() {
        let account = addr(1);
        let authority = addr(2);
        let program = addr(3);
        let program_data = addr(4);
        let destination = addr(5);

        let ix = trim(&account, &authority, &program, &program_data, &destination);

        assert_eq!(ix.accounts.len(), 6);
        assert_eq!(ix.accounts[0], AccountMeta::new(account, false));
        assert_eq!(ix.accounts[1], AccountMeta::new_readonly(authority, true));
        assert_eq!(ix.accounts[2], AccountMeta::new_readonly(program, false));
        assert_eq!(
            ix.accounts[3],
            AccountMeta::new_readonly(program_data, false)
        );
        assert_eq!(ix.accounts[4], AccountMeta::new(destination, false));
        assert_eq!(
            ix.accounts[5],
            AccountMeta::new_readonly(RENT_SYSVAR_ID, false)
        );
        assert_eq!(ix.data, vec![TRIM_DISCRIMINATOR]);
    }

    #[test]
    fn close_matches_expected_wire_format() {
        let account = addr(1);
        let authority = addr(2);
        let program = addr(3);
        let program_data = addr(4);
        let destination = addr(5);

        let ix = close(&account, &authority, &program, &program_data, &destination);

        assert_eq!(ix.accounts.len(), 5);
        assert_eq!(ix.accounts[0], AccountMeta::new(account, false));
        assert_eq!(ix.accounts[1], AccountMeta::new_readonly(authority, true));
        assert_eq!(ix.accounts[2], AccountMeta::new_readonly(program, false));
        assert_eq!(
            ix.accounts[3],
            AccountMeta::new_readonly(program_data, false)
        );
        assert_eq!(ix.accounts[4], AccountMeta::new(destination, false));
        assert_eq!(ix.data, vec![CLOSE_DISCRIMINATOR]);
    }
}
