//! Hand-built client for Solana's Program Metadata Program (PMP,
//! `ProgM6JCCvbYkfKqJYHePx4xxSUSqJp7rh8Lyv7nk7S`) — used to store a
//! program's IDL/security.txt on-chain. Built directly against the real
//! on-chain program's byte layout (github.com/solana-program/
//! program-metadata) rather than its own generated Rust client: that
//! client's `RemainderOptionBytes` convenience wrapper Borsh-length-prefixes
//! trailing instruction data that the on-chain program actually parses as
//! raw, unprefixed bytes, and its account-decoding side carries the same
//! unverified-wrapper risk (custom zero-sentinel Option/PDA types that would
//! need the same byte-for-byte verification before they could be trusted).

pub mod header;
mod instructions;

pub use instructions::*;

use solana_address::Address;

/// Program Metadata Program's on-chain address.
pub const PROGRAM_METADATA_ID: Address =
    solana_address::address!("ProgM6JCCvbYkfKqJYHePx4xxSUSqJp7rh8Lyv7nk7S");

/// The upgradeable BPF loader's on-chain address — owns every upgradeable
/// program's "program data" account.
pub const BPF_LOADER_UPGRADEABLE_ID: Address =
    solana_address::address!("BPFLoaderUpgradeab1e11111111111111111111111");

/// The Rent sysvar's on-chain address, required by [`trim`].
pub const RENT_SYSVAR_ID: Address =
    solana_address::address!("SysvarRent111111111111111111111111111111111");

/// Fixed length of a metadata/buffer PDA's seed value.
pub const SEED_LEN: usize = 16;

/// Fixed byte length of a metadata or buffer account's header — the
/// on-chain `Header`/`Buffer` structs are both exactly this size by design
/// (padded to match each other), so a fresh account needs at least this
/// many rent-exempt lamports even before any payload data is written.
/// Callers computing how much to pre-fund an account for (before
/// [`allocate`]/[`initialize`]/[`set_data`], none of which transfer
/// lamports themselves) should use `HEADER_LEN + <final payload length>`.
pub const HEADER_LEN: usize = 96;

/// A metadata/buffer PDA seed — always exactly [`SEED_LEN`] bytes, zero-padded.
pub type Seed = [u8; SEED_LEN];

/// Zero-pads `s` into a [`Seed`]. Panics if `s` is longer than [`SEED_LEN`]
/// bytes — every real seed (e.g. `"idl"`) is a short, known-at-call-time
/// constant, so an oversized seed is a programmer error, not user input.
pub fn seed_from_str(s: &str) -> Seed {
    let bytes = s.as_bytes();
    assert!(
        bytes.len() <= SEED_LEN,
        "seed \"{s}\" is longer than {SEED_LEN} bytes"
    );
    let mut seed = [0u8; SEED_LEN];
    seed[..bytes.len()].copy_from_slice(bytes);
    seed
}

/// Derives a canonical metadata (or buffer) PDA — one whose authority is the
/// program's own upgrade authority. Matches the on-chain program's own
/// derivation exactly (`[program_id, seed]`, owned by [`PROGRAM_METADATA_ID`]).
pub fn derive_canonical_metadata_pda(program_id: &Address, seed: &Seed) -> (Address, u8) {
    Address::find_program_address(&[program_id.as_ref(), seed.as_ref()], &PROGRAM_METADATA_ID)
}

/// Derives a non-canonical metadata (or buffer) PDA — one whose authority is
/// an arbitrary signer rather than the program's upgrade authority. Matches
/// the on-chain program's own derivation exactly (`[program_id, authority,
/// seed]`, owned by [`PROGRAM_METADATA_ID`]).
pub fn derive_metadata_pda(
    program_id: &Address,
    authority: &Address,
    seed: &Seed,
) -> (Address, u8) {
    Address::find_program_address(
        &[program_id.as_ref(), authority.as_ref(), seed.as_ref()],
        &PROGRAM_METADATA_ID,
    )
}

/// Derives the upgradeable BPF loader's "program data" account for
/// `program_id` — the account that records the program's current upgrade
/// authority. Required by [`initialize`]/[`allocate`] to validate a
/// canonical metadata account's authority on-chain.
pub fn derive_program_data_address(program_id: &Address) -> (Address, u8) {
    Address::find_program_address(&[program_id.as_ref()], &BPF_LOADER_UPGRADEABLE_ID)
}
