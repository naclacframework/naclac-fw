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
pub use provider::{ClientBackend, NaclacProvider, NaclacTransactionMetadata};
pub use utils::{
    create_ata, create_mint, create_token_account, get_discriminator, load_node_wallet, mint_to,
    transfer_sol, SignerAddressExt, ASSOCIATED_TOKEN_PROGRAM_ID, SYSTEM_PROGRAM_ID,
    TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID,
};

// Custom AccountMeta and Address
pub use solana_address::Address;

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

// Standard Solana SDK Re-exports
pub use borsh;
pub use bytemuck;
pub use solana_keypair::Keypair;
pub use solana_program::instruction::{Instruction, InstructionError};
pub use solana_signature::Signature;
pub use solana_signer::Signer;
pub use solana_transaction::versioned::VersionedTransaction;
pub use std::vec;
pub use std::vec::Vec;
