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
