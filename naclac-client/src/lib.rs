pub mod builder;
pub mod error;
pub mod fetcher;
pub mod provider;
pub mod token;
pub mod utils;

pub use token::{Mint, TokenAccount};

pub use builder::InstructionBuilder;
pub use error::{decode_custom_error, translate_error_code, NaclacClientError, NaclacError};
pub use fetcher::{
    decode_borsh_checked, decode_pod_checked, decode_pod_unchecked, AccountFetcher, NaclacDecode,
};
pub use provider::{
    ClientBackend, NaclacAllocEvent, NaclacProvider, NaclacTransactionMetadata, RpcCluster,
};
pub use utils::{
    create_ata, create_ata_with_program, create_mint, create_token_account, get_discriminator,
    load_node_wallet, mint_to, resolve_cargo_target_dir, transfer_sol, SignerAddressExt,
    ASSOCIATED_TOKEN_PROGRAM_ID, RENT_SYSVAR_ID, SYSTEM_PROGRAM_ID, TOKEN_2022_PROGRAM_ID,
    TOKEN_PROGRAM_ID,
};

// Custom AccountMeta and Address
pub use solana_address::{address, Address};

#[derive(Clone, Debug)]
pub struct AccountMeta {
    pub address: Address,
    pub is_signer: bool,
    pub is_writable: bool,
}

impl AccountMeta {
    pub fn new(address: Address, is_signer: bool) -> Self {
        Self {
            address,
            is_signer,
            is_writable: true,
        }
    }
    pub fn new_readonly(address: Address, is_signer: bool) -> Self {
        Self {
            address,
            is_signer,
            is_writable: false,
        }
    }
}

// `Span`/`ZcString` are pure `core`/`bytemuck` types with no `no_std`-only
// dependency, so they're safe to re-export offchain too, for code that needs
// to inspect a zero-copy program's CPI-mode types directly.
pub use naclac_core::wrappers::{Span, ZcString};
pub type ZcVec<T> = Span<T>;

/// A binary-stable wrapper for `bool` — mirrors `naclac-core`'s own
/// `Bool(pub u8)` exactly (independent definition, not re-exported, to avoid
/// pulling every `naclac-core` feature/cfg surface into offchain code that
/// only needs this one type).
#[repr(transparent)]
#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    bytemuck::Pod,
    bytemuck::Zeroable,
)]
pub struct Bool(pub u8);

impl From<u8> for Bool {
    fn from(b: u8) -> Self {
        Self(b)
    }
}

impl From<bool> for Bool {
    fn from(b: bool) -> Self {
        Self(if b { 1 } else { 0 })
    }
}

impl From<Bool> for bool {
    fn from(b: Bool) -> Self {
        b.0 != 0
    }
}

/// Reads a value's own in-memory bytes without requiring `bytemuck::Pod`.
///
/// `Pod` (and `bytemuck::bytes_of`) is required for the *reverse* direction
/// — interpreting arbitrary, possibly-invalid bytes as a value — which is
/// why a data-carrying zero-copy `#[defined_type]` enum's generated client
/// type deliberately gets only `CheckedBitPattern`, not `Pod`. Going the
/// other way — reading the bytes of a value that already exists and is
/// therefore already valid — carries no such risk and is sound for any
/// `Copy` type regardless of whether it's `Pod`, since no new value is ever
/// constructed from unchecked bytes here. Used by generated instruction-arg
/// encoding for exactly that case (`naclac-client-gen/src/rust/mod.rs`'s
/// `write_field_bytes`).
#[inline(always)]
pub fn bytes_of_checked_bit_pattern<T: Copy>(value: &T) -> &[u8] {
    // SAFETY: see the doc comment above — `value` is already a valid `T`,
    // so reading its own bytes never constructs a `T` from unchecked bytes.
    unsafe { core::slice::from_raw_parts((value as *const T).cast::<u8>(), core::mem::size_of::<T>()) }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Proves `bytes_of_checked_bit_pattern` never panics and returns
    /// exactly `size_of::<T>()` bytes matching `value`'s own representation
    /// — for two representative `Copy` types (a small primitive and a
    /// larger fixed-size struct), since the function's soundness doesn't
    /// depend on which `T` is used, only that `T: Copy`.
    #[kani::proof]
    fn prove_bytes_of_checked_bit_pattern_u32_never_panics() {
        let value: u32 = kani::any();
        let bytes = bytes_of_checked_bit_pattern(&value);
        assert_eq!(bytes.len(), core::mem::size_of::<u32>());
        assert_eq!(bytes, &value.to_ne_bytes());
    }

    #[kani::proof]
    fn prove_bytes_of_checked_bit_pattern_array_never_panics() {
        let value: [u8; 32] = kani::any();
        let bytes = bytes_of_checked_bit_pattern(&value);
        assert_eq!(bytes.len(), 32);
        assert_eq!(bytes, &value[..]);
    }
}

// Standard Solana SDK Re-exports
pub use borsh;
pub use bytemuck;
pub use solana_keypair::Keypair;
pub use solana_program::instruction::{Instruction, InstructionError};
pub use solana_signature::Signature;
pub use solana_signer::Signer;
pub use solana_transaction::versioned::VersionedTransaction;
pub use std::string::String;
pub use std::vec;
pub use std::vec::Vec;
