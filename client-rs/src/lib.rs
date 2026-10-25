pub mod builder;
pub mod error;
pub mod fetcher;
pub mod provider;
pub mod token;
pub mod utils;

// Custom AccountMeta and Address
pub use solana_address::{address, Address};
pub use {
    builder::InstructionBuilder,
    error::{decode_custom_error, translate_error_code, NaclacClientError, NaclacError},
    fetcher::{
        decode_borsh_checked, decode_pod_checked, decode_pod_unchecked, decode_wincode_checked,
        AccountFetcher, NaclacDecode,
    },
    provider::{
        ClientBackend, NaclacAllocEvent, NaclacEvent, NaclacInnerInstruction, NaclacProvider,
        NaclacTransactionMetadata, RpcCluster, TransactionVersion,
    },
    token::{Mint, TokenAccount},
    utils::{
        create_ata, create_ata_with_program, create_mint, create_token_account, load_node_wallet,
        mint_to, transfer_sol, workspace_deploy_dir, SignerAddressExt, ASSOCIATED_TOKEN_PROGRAM_ID,
        RENT_SYSVAR_ID, SYSTEM_PROGRAM_ID, TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID,
    },
};

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

// The plain-data types of a zero-copy layout are defined once, in
// `naclac-pod`, and used unchanged by programs and clients.
pub use naclac_pod::{
    decode_pod, encode_pod, Bool, FixedString, FixedVec, Opt, Unaligned, ZcCodec, ZcEnum,
    ZcEnumKind, ZcField, ZcStorage, ZcString, ZcVec,
};

// Standard Solana SDK Re-exports
pub use {borsh, bytemuck, wincode};

/// Off-chain mirror of `naclac_core::wincode_config` — deliberately a
/// separate definition, not a re-export. `naclac_core::wincode_config` exists
/// only when `naclac-core` is built with its `wincode` layout feature, which
/// `naclac-client` never enables, so a re-export would be absent whenever the
/// program under test uses another layout. Must stay byte-identical to
/// `naclac_core::wincode_config`, or on-chain and off-chain wire formats will
/// silently disagree.
pub mod wincode_config {
    pub type Config = wincode::config::Configuration<
        true,
        { wincode::config::DEFAULT_PREALLOCATION_SIZE_LIMIT },
        wincode::len::FixIntLen<u32>,
        wincode::int_encoding::LittleEndian,
        wincode::int_encoding::FixInt,
        u8,
    >;

    pub const CONFIG: Config = wincode::config::Configuration::default()
        .with_length_encoding::<wincode::len::FixIntLen<u32>>()
        .with_tag_encoding::<u8>();
}

pub use std::vec;
pub use {
    solana_instruction::Instruction,
    solana_instruction_error::InstructionError,
    solana_keypair::Keypair,
    solana_signature::Signature,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    std::{string::String, vec::Vec},
};
