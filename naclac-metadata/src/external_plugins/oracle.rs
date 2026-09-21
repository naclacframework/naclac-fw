// ===========================================================================
// external_plugins/oracle.rs — the `Oracle` external plugin adapter
// ===========================================================================

//! `Oracle` — attaches to an `Asset` or `Collection`, delegating lifecycle
//! approval (create/transfer/burn/update) to an external account's data.
//! Real layout verified against `mpl-core`'s `generated::types::{Oracle,
//! OracleInitInfo, OracleUpdateInfo, ExtraAccount, Seed,
//! ValidationResultsOffset}`. Attach/update/remove/write need no extra
//! accounts beyond the standard shape — verified directly:
//! `Oracle::validate_add_external_plugin_adapter` is an unconditional
//! `abstain!()`, unlike `AgentIdentity`'s (which requires a specific extra
//! signer — see `docs/05-remaining-external-adapters-plan.md` for why
//! `AgentIdentity` isn't implemented). The oracle account itself is only
//! consulted later, during the asset's own transfer/burn/update/create —
//! a separate, pre-existing gap in `lifecycle.rs` (it doesn't support
//! dynamic extra accounts for *any* hook-consuming asset yet), not
//! something attaching `Oracle` itself needs.
//!
//! No real protocol maximum exists for `lifecycle_checks`,
//! `ExtraAccount::CustomPda`'s `seeds`, or `Seed::Bytes`'s length (verified
//! the same way as every other cap in this crate). Caps used on the write
//! side, agreed with the user:
//! - `lifecycle_checks`: **5** — not a guess, the real ceiling
//!   (`HookableLifecycleEvent` only has 5 variants; a real `(event,
//!   result)` list only ever makes sense once per event).
//! - `ExtraAccount::CustomPda.seeds`: **4** — real PDA derivation never
//!   needs more than a handful of seeds.
//! - `Seed::Bytes`: **32** — mirrors Solana's own real per-seed length
//!   limit.
//!
//! The read side has no such cap — it parses whatever's actually on-chain
//! (which could in principle exceed these write-side caps, e.g. written by
//! a different program/framework), using owned `Vec`s for the nested
//! `Seed::Bytes`/`seeds` list the same way `attributes.rs`/`autograph.rs`
//! do for their own genuinely unbounded content.

use crate::prelude::*;

/// Maximum `lifecycle_checks` entries — see this file's header.
pub const MAX_ORACLE_LIFECYCLE_CHECKS: usize = 5;
/// Maximum `ExtraAccount::CustomPda.seeds` entries — see this file's header.
pub const MAX_ORACLE_CUSTOM_PDA_SEEDS: usize = 4;
/// Maximum `Seed::Bytes` length — see this file's header.
pub const MAX_ORACLE_SEED_BYTES_LEN: usize = 32;

/// Backend-neutral mirror of the real `HookableLifecycleEvent` enum
/// (`Create=0, Transfer=1, Burn=2, Update=3, Execute=4`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HookableLifecycleEventArg {
    Create,
    Transfer,
    Burn,
    Update,
    Execute,
}

/// Backend-neutral mirror of the real `ExternalCheckResult` struct.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExternalCheckResultArg {
    pub flags: u32,
}

/// Backend-neutral mirror of the real `ValidationResultsOffset` enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidationResultsOffsetArg {
    NoOffset,
    Anchor,
    Custom(u64),
}

/// Argument form of `Seed` for `attach_asset_oracle_signed`/`update_*` —
/// borrows rather than owns, since it only needs to live for the CPI call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeedArg<'a> {
    Collection,
    Owner,
    Recipient,
    Asset,
    Address(Address),
    Bytes(&'a [u8]),
}

/// Owned mirror of `Seed`, for the read side — see this file's header for
/// why this stays `Vec`-based (genuinely unbounded on read, unlike the
/// capped write side).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeedOwned {
    Collection,
    Owner,
    Recipient,
    Asset,
    Address(Address),
    Bytes(crate::prelude::Vec<u8>),
}

/// Argument form of `ExtraAccount` for `attach_asset_oracle_signed`/
/// `update_*` — borrows rather than owns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtraAccountArg<'a> {
    PreconfiguredProgram { is_signer: bool, is_writable: bool },
    PreconfiguredCollection { is_signer: bool, is_writable: bool },
    PreconfiguredOwner { is_signer: bool, is_writable: bool },
    PreconfiguredRecipient { is_signer: bool, is_writable: bool },
    PreconfiguredAsset { is_signer: bool, is_writable: bool },
    CustomPda {
        seeds: &'a [SeedArg<'a>],
        custom_program_id: Option<Address>,
        is_signer: bool,
        is_writable: bool,
    },
    Address { address: Address, is_signer: bool, is_writable: bool },
}

/// Owned mirror of `ExtraAccount`, for the read side.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExtraAccountOwned {
    PreconfiguredProgram { is_signer: bool, is_writable: bool },
    PreconfiguredCollection { is_signer: bool, is_writable: bool },
    PreconfiguredOwner { is_signer: bool, is_writable: bool },
    PreconfiguredRecipient { is_signer: bool, is_writable: bool },
    PreconfiguredAsset { is_signer: bool, is_writable: bool },
    CustomPda {
        seeds: crate::prelude::Vec<SeedOwned>,
        custom_program_id: Option<Address>,
        is_signer: bool,
        is_writable: bool,
    },
    Address { address: Address, is_signer: bool, is_writable: bool },
}

/// Backend-neutral `Oracle` payload — `fetch_asset_oracle`/
/// `fetch_collection_oracle` return this on both backends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OracleInfo {
    pub base_address: Address,
    pub base_address_config: Option<ExtraAccountOwned>,
    pub results_offset: ValidationResultsOffsetArg,
}

// ===========================================================================
// solana — hand-rolled Borsh wire-format encoding, `Vec<u8>`-based mirrors
// of the `pinocchio` encoders below
// ===========================================================================

#[cfg(not(feature = "pinocchio"))]
fn encode_seed_owned(data: &mut crate::prelude::Vec<u8>, seed: &SeedArg<'_>) -> Result<()> {
    match seed {
        SeedArg::Collection => data.push(0u8),
        SeedArg::Owner => data.push(1u8),
        SeedArg::Recipient => data.push(2u8),
        SeedArg::Asset => data.push(3u8),
        SeedArg::Address(addr) => {
            data.push(4u8);
            data.extend_from_slice(addr.as_ref());
        }
        SeedArg::Bytes(bytes) => {
            if bytes.len() > MAX_ORACLE_SEED_BYTES_LEN {
                return Err(NaclacError::InvalidInstructionData.err(0));
            }
            data.push(5u8);
            data.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            data.extend_from_slice(bytes);
        }
    }
    Ok(())
}

#[cfg(not(feature = "pinocchio"))]
fn encode_extra_account_owned(
    data: &mut crate::prelude::Vec<u8>,
    ea: &ExtraAccountArg<'_>,
) -> Result<()> {
    match ea {
        ExtraAccountArg::PreconfiguredProgram { is_signer, is_writable } => {
            data.push(0u8);
            data.push(*is_signer as u8);
            data.push(*is_writable as u8);
        }
        ExtraAccountArg::PreconfiguredCollection { is_signer, is_writable } => {
            data.push(1u8);
            data.push(*is_signer as u8);
            data.push(*is_writable as u8);
        }
        ExtraAccountArg::PreconfiguredOwner { is_signer, is_writable } => {
            data.push(2u8);
            data.push(*is_signer as u8);
            data.push(*is_writable as u8);
        }
        ExtraAccountArg::PreconfiguredRecipient { is_signer, is_writable } => {
            data.push(3u8);
            data.push(*is_signer as u8);
            data.push(*is_writable as u8);
        }
        ExtraAccountArg::PreconfiguredAsset { is_signer, is_writable } => {
            data.push(4u8);
            data.push(*is_signer as u8);
            data.push(*is_writable as u8);
        }
        ExtraAccountArg::CustomPda { seeds, custom_program_id, is_signer, is_writable } => {
            if seeds.len() > MAX_ORACLE_CUSTOM_PDA_SEEDS {
                return Err(NaclacError::InvalidInstructionData.err(0));
            }
            data.push(5u8);
            data.extend_from_slice(&(seeds.len() as u32).to_le_bytes());
            for s in *seeds {
                encode_seed_owned(data, s)?;
            }
            match custom_program_id {
                Some(addr) => {
                    data.push(1u8);
                    data.extend_from_slice(addr.as_ref());
                }
                None => data.push(0u8),
            }
            data.push(*is_signer as u8);
            data.push(*is_writable as u8);
        }
        ExtraAccountArg::Address { address, is_signer, is_writable } => {
            data.push(6u8);
            data.extend_from_slice(address.as_ref());
            data.push(*is_signer as u8);
            data.push(*is_writable as u8);
        }
    }
    Ok(())
}

#[cfg(not(feature = "pinocchio"))]
fn encode_validation_results_offset_owned(
    data: &mut crate::prelude::Vec<u8>,
    v: ValidationResultsOffsetArg,
) {
    match v {
        ValidationResultsOffsetArg::NoOffset => data.push(0u8),
        ValidationResultsOffsetArg::Anchor => data.push(1u8),
        ValidationResultsOffsetArg::Custom(n) => {
            data.push(2u8);
            data.extend_from_slice(&n.to_le_bytes());
        }
    }
}

#[cfg(not(feature = "pinocchio"))]
fn encode_lifecycle_checks_owned(
    data: &mut crate::prelude::Vec<u8>,
    checks: &[(HookableLifecycleEventArg, ExternalCheckResultArg)],
) -> Result<()> {
    if checks.len() > MAX_ORACLE_LIFECYCLE_CHECKS {
        return Err(NaclacError::InvalidInstructionData.err(0));
    }
    data.extend_from_slice(&(checks.len() as u32).to_le_bytes());
    for (event, result) in checks {
        data.push(match event {
            HookableLifecycleEventArg::Create => 0u8,
            HookableLifecycleEventArg::Transfer => 1u8,
            HookableLifecycleEventArg::Burn => 2u8,
            HookableLifecycleEventArg::Update => 3u8,
            HookableLifecycleEventArg::Execute => 4u8,
        });
        data.extend_from_slice(&result.flags.to_le_bytes());
    }
    Ok(())
}

// ===========================================================================
// pinocchio — hand-rolled Borsh wire-format encoding, `FixedBuf`-based
// ===========================================================================

/// Max Borsh-encoded width of a `Seed` — the `Bytes` variant (tag + 4-byte
/// len + up to `MAX_ORACLE_SEED_BYTES_LEN`) is the widest.
#[cfg(feature = "pinocchio")]
const MAX_SEED_ENCODED_LEN: usize = 1 + 4 + MAX_ORACLE_SEED_BYTES_LEN;

/// Max Borsh-encoded width of an `ExtraAccount` — the `CustomPda` variant
/// (tag + 4-byte seed count + up to `MAX_ORACLE_CUSTOM_PDA_SEEDS` seeds +
/// `Option<Pubkey>` + 2 bools) is the widest.
#[cfg(feature = "pinocchio")]
const MAX_EXTRA_ACCOUNT_ENCODED_LEN: usize =
    1 + 4 + MAX_ORACLE_CUSTOM_PDA_SEEDS * MAX_SEED_ENCODED_LEN + (1 + 32) + 1 + 1;

/// Max Borsh-encoded width of `Option<ExtraAccount>`.
#[cfg(feature = "pinocchio")]
const MAX_EXTRA_ACCOUNT_OPTION_LEN: usize = 1 + MAX_EXTRA_ACCOUNT_ENCODED_LEN;

/// Max Borsh-encoded width of `lifecycle_checks: Vec<(HookableLifecycleEvent,
/// ExternalCheckResult)>` — 4-byte count + up to `MAX_ORACLE_LIFECYCLE_CHECKS`
/// × (1-byte tag + 4-byte flags).
#[cfg(feature = "pinocchio")]
const MAX_LIFECYCLE_CHECKS_ENCODED_LEN: usize = 4 + MAX_ORACLE_LIFECYCLE_CHECKS * 5;

/// Max Borsh-encoded width of `Option<ValidationResultsOffset>` — the
/// `Custom(u64)` variant (tag + 8 bytes) is the widest.
#[cfg(feature = "pinocchio")]
const MAX_VALIDATION_RESULTS_OFFSET_OPTION_LEN: usize = 1 + 1 + 8;

#[cfg(feature = "pinocchio")]
fn encode_seed<S: crate::fixed_buf::ByteSink>(data: &mut S, seed: &SeedArg<'_>) -> Result<()> {
    match seed {
        SeedArg::Collection => data.push(0u8),
        SeedArg::Owner => data.push(1u8),
        SeedArg::Recipient => data.push(2u8),
        SeedArg::Asset => data.push(3u8),
        SeedArg::Address(addr) => {
            data.push(4u8);
            data.extend_from_slice(addr.as_ref());
        }
        SeedArg::Bytes(bytes) => {
            if bytes.len() > MAX_ORACLE_SEED_BYTES_LEN {
                return Err(NaclacError::InvalidInstructionData.err(0));
            }
            data.push(5u8);
            data.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            data.extend_from_slice(bytes);
        }
    }
    Ok(())
}

#[cfg(feature = "pinocchio")]
fn encode_extra_account<S: crate::fixed_buf::ByteSink>(
    data: &mut S,
    ea: &ExtraAccountArg<'_>,
) -> Result<()> {
    match ea {
        ExtraAccountArg::PreconfiguredProgram { is_signer, is_writable } => {
            data.push(0u8);
            data.push(*is_signer as u8);
            data.push(*is_writable as u8);
        }
        ExtraAccountArg::PreconfiguredCollection { is_signer, is_writable } => {
            data.push(1u8);
            data.push(*is_signer as u8);
            data.push(*is_writable as u8);
        }
        ExtraAccountArg::PreconfiguredOwner { is_signer, is_writable } => {
            data.push(2u8);
            data.push(*is_signer as u8);
            data.push(*is_writable as u8);
        }
        ExtraAccountArg::PreconfiguredRecipient { is_signer, is_writable } => {
            data.push(3u8);
            data.push(*is_signer as u8);
            data.push(*is_writable as u8);
        }
        ExtraAccountArg::PreconfiguredAsset { is_signer, is_writable } => {
            data.push(4u8);
            data.push(*is_signer as u8);
            data.push(*is_writable as u8);
        }
        ExtraAccountArg::CustomPda { seeds, custom_program_id, is_signer, is_writable } => {
            if seeds.len() > MAX_ORACLE_CUSTOM_PDA_SEEDS {
                return Err(NaclacError::InvalidInstructionData.err(0));
            }
            data.push(5u8);
            data.extend_from_slice(&(seeds.len() as u32).to_le_bytes());
            for s in *seeds {
                encode_seed(data, s)?;
            }
            match custom_program_id {
                Some(addr) => {
                    data.push(1u8);
                    data.extend_from_slice(addr.as_ref());
                }
                None => data.push(0u8),
            }
            data.push(*is_signer as u8);
            data.push(*is_writable as u8);
        }
        ExtraAccountArg::Address { address, is_signer, is_writable } => {
            data.push(6u8);
            data.extend_from_slice(address.as_ref());
            data.push(*is_signer as u8);
            data.push(*is_writable as u8);
        }
    }
    Ok(())
}

#[cfg(feature = "pinocchio")]
fn encode_validation_results_offset<S: crate::fixed_buf::ByteSink>(
    data: &mut S,
    v: ValidationResultsOffsetArg,
) {
    match v {
        ValidationResultsOffsetArg::NoOffset => data.push(0u8),
        ValidationResultsOffsetArg::Anchor => data.push(1u8),
        ValidationResultsOffsetArg::Custom(n) => {
            data.push(2u8);
            data.extend_from_slice(&n.to_le_bytes());
        }
    }
}

#[cfg(feature = "pinocchio")]
fn encode_lifecycle_checks<S: crate::fixed_buf::ByteSink>(
    data: &mut S,
    checks: &[(HookableLifecycleEventArg, ExternalCheckResultArg)],
) -> Result<()> {
    if checks.len() > MAX_ORACLE_LIFECYCLE_CHECKS {
        return Err(NaclacError::InvalidInstructionData.err(0));
    }
    data.extend_from_slice(&(checks.len() as u32).to_le_bytes());
    for (event, result) in checks {
        data.push(match event {
            HookableLifecycleEventArg::Create => 0u8,
            HookableLifecycleEventArg::Transfer => 1u8,
            HookableLifecycleEventArg::Burn => 2u8,
            HookableLifecycleEventArg::Update => 3u8,
            HookableLifecycleEventArg::Execute => 4u8,
        });
        data.extend_from_slice(&result.flags.to_le_bytes());
    }
    Ok(())
}

// ===========================================================================
// attach / update / remove / write
// ===========================================================================

/// Max total instruction data for `attach_asset_oracle_signed`/
/// `attach_collection_oracle_signed`: discriminator(1), `InitInfo` tag(1),
/// `base_address`(32), `init_plugin_authority`(1, always `None`),
/// `lifecycle_checks`, `base_address_config`, `results_offset`. No trailing
/// `init_authority` byte — unlike `AddPluginV1Args`, the real
/// `AddExternalPluginAdapterV1InstructionArgs` has only the single
/// `init_info` field (verified against the real generated
/// `instructions/add_external_plugin_adapter_v1.rs`).
#[cfg(feature = "pinocchio")]
const ATTACH_ORACLE_IX_LEN: usize = 1
    + 1
    + 32
    + 1
    + MAX_LIFECYCLE_CHECKS_ENCODED_LEN
    + MAX_EXTRA_ACCOUNT_OPTION_LEN
    + MAX_VALIDATION_RESULTS_OFFSET_OPTION_LEN;

/// Max total instruction data for `update_asset_oracle_signed`/
/// `update_collection_oracle_signed`: discriminator(1) + encoded
/// `ExternalPluginAdapterKeyArg::Oracle`(1 + 32) + `UpdateInfo` tag(1) +
/// `lifecycle_checks`/`base_address_config`/`results_offset` (each already
/// `Option`-wrapped in the real type).
#[cfg(feature = "pinocchio")]
const UPDATE_ORACLE_IX_LEN: usize = 1
    + (1 + 32)
    + 1
    + (1 + MAX_LIFECYCLE_CHECKS_ENCODED_LEN)
    + MAX_EXTRA_ACCOUNT_OPTION_LEN
    + MAX_VALIDATION_RESULTS_OFFSET_OPTION_LEN;

/// Attaches `Oracle` to an `Asset` via `AddExternalPluginAdapterV1`.
#[allow(clippy::too_many_arguments)]
pub fn attach_asset_oracle_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    base_address: Address,
    lifecycle_checks: &[(HookableLifecycleEventArg, ExternalCheckResultArg)],
    base_address_config: Option<ExtraAccountArg<'_>>,
    results_offset: Option<ValidationResultsOffsetArg>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = crate::prelude::Vec::new();
        data.push(22u8); // AddExternalPluginAdapterV1 discriminator
        data.push(1u8); // ExternalPluginAdapterInitInfo::Oracle tag
        data.extend_from_slice(base_address.as_ref());
        data.push(0u8); // init_plugin_authority: None
        encode_lifecycle_checks_owned(&mut data, lifecycle_checks)?;
        match &base_address_config {
            Some(ea) => {
                data.push(1u8);
                encode_extra_account_owned(&mut data, ea)?;
            }
            None => data.push(0u8),
        }
        match results_offset {
            Some(v) => {
                data.push(1u8);
                encode_validation_results_offset_owned(&mut data, v);
            }
            None => data.push(0u8),
        }
        add_asset_external_adapter_signed(program, accounts, &data, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mut data = crate::fixed_buf::FixedBuf::<ATTACH_ORACLE_IX_LEN>::new();
        data.push(22u8); // AddExternalPluginAdapterV1 discriminator
        data.push(1u8); // ExternalPluginAdapterInitInfo::Oracle tag
        data.extend_from_slice(base_address.as_ref());
        data.push(0u8); // init_plugin_authority: None
        encode_lifecycle_checks(&mut data, lifecycle_checks)?;
        match &base_address_config {
            Some(ea) => {
                data.push(1u8);
                encode_extra_account(&mut data, ea)?;
            }
            None => data.push(0u8),
        }
        match results_offset {
            Some(v) => {
                data.push(1u8);
                encode_validation_results_offset(&mut data, v);
            }
            None => data.push(0u8),
        }
        add_asset_external_adapter_signed(program, accounts, data.as_slice(), signer_seeds)
    }
}

/// Attaches `Oracle` to a `Collection` via `AddCollectionExternalPluginAdapterV1`.
#[allow(clippy::too_many_arguments)]
pub fn attach_collection_oracle_signed(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    base_address: Address,
    lifecycle_checks: &[(HookableLifecycleEventArg, ExternalCheckResultArg)],
    base_address_config: Option<ExtraAccountArg<'_>>,
    results_offset: Option<ValidationResultsOffsetArg>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = crate::prelude::Vec::new();
        data.push(23u8); // AddCollectionExternalPluginAdapterV1 discriminator
        data.push(1u8); // ExternalPluginAdapterInitInfo::Oracle tag
        data.extend_from_slice(base_address.as_ref());
        data.push(0u8); // init_plugin_authority: None
        encode_lifecycle_checks_owned(&mut data, lifecycle_checks)?;
        match &base_address_config {
            Some(ea) => {
                data.push(1u8);
                encode_extra_account_owned(&mut data, ea)?;
            }
            None => data.push(0u8),
        }
        match results_offset {
            Some(v) => {
                data.push(1u8);
                encode_validation_results_offset_owned(&mut data, v);
            }
            None => data.push(0u8),
        }
        add_collection_external_adapter_signed(program, accounts, &data, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mut data = crate::fixed_buf::FixedBuf::<ATTACH_ORACLE_IX_LEN>::new();
        data.push(23u8); // AddCollectionExternalPluginAdapterV1 discriminator
        data.push(1u8); // ExternalPluginAdapterInitInfo::Oracle tag
        data.extend_from_slice(base_address.as_ref());
        data.push(0u8); // init_plugin_authority: None
        encode_lifecycle_checks(&mut data, lifecycle_checks)?;
        match &base_address_config {
            Some(ea) => {
                data.push(1u8);
                encode_extra_account(&mut data, ea)?;
            }
            None => data.push(0u8),
        }
        match results_offset {
            Some(v) => {
                data.push(1u8);
                encode_validation_results_offset(&mut data, v);
            }
            None => data.push(0u8),
        }
        add_collection_external_adapter_signed(program, accounts, data.as_slice(), signer_seeds)
    }
}

/// Updates `Oracle`'s `lifecycle_checks`/`base_address_config`/
/// `results_offset` on an `Asset` via `UpdateExternalPluginAdapterV1`.
/// `base_address` identifies which instance. Each `new_*` parameter left
/// `None` leaves that field unchanged — matches the real
/// `OracleUpdateInfo`'s own `Option` semantics exactly.
pub fn update_asset_oracle_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    base_address: Address,
    new_lifecycle_checks: Option<&[(HookableLifecycleEventArg, ExternalCheckResultArg)]>,
    new_base_address_config: Option<ExtraAccountArg<'_>>,
    new_results_offset: Option<ValidationResultsOffsetArg>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = crate::prelude::Vec::new();
        data.push(26u8); // UpdateExternalPluginAdapterV1 discriminator
        data.push(1u8); // ExternalPluginAdapterKeyArg::Oracle tag
        data.extend_from_slice(base_address.as_ref());
        data.push(1u8); // ExternalPluginAdapterUpdateInfo::Oracle tag
        match new_lifecycle_checks {
            Some(checks) => {
                data.push(1u8);
                encode_lifecycle_checks_owned(&mut data, checks)?;
            }
            None => data.push(0u8),
        }
        match &new_base_address_config {
            Some(ea) => {
                data.push(1u8);
                encode_extra_account_owned(&mut data, ea)?;
            }
            None => data.push(0u8),
        }
        match new_results_offset {
            Some(v) => {
                data.push(1u8);
                encode_validation_results_offset_owned(&mut data, v);
            }
            None => data.push(0u8),
        }
        update_asset_external_adapter_signed(program, accounts, &data, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mut data = crate::fixed_buf::FixedBuf::<UPDATE_ORACLE_IX_LEN>::new();
        data.push(26u8); // UpdateExternalPluginAdapterV1 discriminator
        data.push(1u8); // ExternalPluginAdapterKeyArg::Oracle tag
        data.extend_from_slice(base_address.as_ref());
        data.push(1u8); // ExternalPluginAdapterUpdateInfo::Oracle tag
        match new_lifecycle_checks {
            Some(checks) => {
                data.push(1u8);
                encode_lifecycle_checks(&mut data, checks)?;
            }
            None => data.push(0u8),
        }
        match &new_base_address_config {
            Some(ea) => {
                data.push(1u8);
                encode_extra_account(&mut data, ea)?;
            }
            None => data.push(0u8),
        }
        match new_results_offset {
            Some(v) => {
                data.push(1u8);
                encode_validation_results_offset(&mut data, v);
            }
            None => data.push(0u8),
        }
        update_asset_external_adapter_signed(program, accounts, data.as_slice(), signer_seeds)
    }
}

/// Updates `Oracle`'s `lifecycle_checks`/`base_address_config`/
/// `results_offset` on a `Collection` via `UpdateCollectionExternalPluginAdapterV1`.
/// See `update_asset_oracle_signed` for the `new_*` parameters' semantics.
pub fn update_collection_oracle_signed(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    base_address: Address,
    new_lifecycle_checks: Option<&[(HookableLifecycleEventArg, ExternalCheckResultArg)]>,
    new_base_address_config: Option<ExtraAccountArg<'_>>,
    new_results_offset: Option<ValidationResultsOffsetArg>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = crate::prelude::Vec::new();
        data.push(27u8); // UpdateCollectionExternalPluginAdapterV1 discriminator
        data.push(1u8); // ExternalPluginAdapterKeyArg::Oracle tag
        data.extend_from_slice(base_address.as_ref());
        data.push(1u8); // ExternalPluginAdapterUpdateInfo::Oracle tag
        match new_lifecycle_checks {
            Some(checks) => {
                data.push(1u8);
                encode_lifecycle_checks_owned(&mut data, checks)?;
            }
            None => data.push(0u8),
        }
        match &new_base_address_config {
            Some(ea) => {
                data.push(1u8);
                encode_extra_account_owned(&mut data, ea)?;
            }
            None => data.push(0u8),
        }
        match new_results_offset {
            Some(v) => {
                data.push(1u8);
                encode_validation_results_offset_owned(&mut data, v);
            }
            None => data.push(0u8),
        }
        update_collection_external_adapter_signed(program, accounts, &data, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mut data = crate::fixed_buf::FixedBuf::<UPDATE_ORACLE_IX_LEN>::new();
        data.push(27u8); // UpdateCollectionExternalPluginAdapterV1 discriminator
        data.push(1u8); // ExternalPluginAdapterKeyArg::Oracle tag
        data.extend_from_slice(base_address.as_ref());
        data.push(1u8); // ExternalPluginAdapterUpdateInfo::Oracle tag
        match new_lifecycle_checks {
            Some(checks) => {
                data.push(1u8);
                encode_lifecycle_checks(&mut data, checks)?;
            }
            None => data.push(0u8),
        }
        match &new_base_address_config {
            Some(ea) => {
                data.push(1u8);
                encode_extra_account(&mut data, ea)?;
            }
            None => data.push(0u8),
        }
        match new_results_offset {
            Some(v) => {
                data.push(1u8);
                encode_validation_results_offset(&mut data, v);
            }
            None => data.push(0u8),
        }
        update_collection_external_adapter_signed(
            program,
            accounts,
            data.as_slice(),
            signer_seeds,
        )
    }
}

/// Removes an `Asset`'s `Oracle` adapter identified by `base_address`, via
/// `RemoveExternalPluginAdapterV1`.
pub fn remove_asset_oracle_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    base_address: Address,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    remove_asset_external_adapter_signed(
        program,
        accounts,
        ExternalPluginAdapterKeyArg::Oracle(base_address),
        signer_seeds,
    )
}

/// Removes a `Collection`'s `Oracle` adapter identified by `base_address`,
/// via `RemoveCollectionExternalPluginAdapterV1`.
pub fn remove_collection_oracle_signed(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    base_address: Address,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    remove_collection_external_adapter_signed(
        program,
        accounts,
        ExternalPluginAdapterKeyArg::Oracle(base_address),
        signer_seeds,
    )
}

// ===========================================================================
// read
// ===========================================================================

/// Reads an `Asset`'s `Oracle` adapter identified by `base_address`, if
/// attached. Callable identically on both backends.
#[cfg(not(feature = "pinocchio"))]
pub fn fetch_asset_oracle(
    info: &AccountInfo,
    base_address: Address,
) -> Result<Option<OracleInfo>> {
    if Owner::program_owner(info) != crate::ID {
        return Err(NaclacError::ConstraintOwner.into());
    }
    let solana_info = unsafe { info.to_lifetime() };
    let raw = solana_info
        .try_borrow_data()
        .map_err(|_| NaclacError::AccountBorrowFailed.err(0))?;
    let asset_view = crate::asset::AssetView::from_bytes(&raw)?;
    let Some(plugin_header_offset) = asset_view.plugin_header_offset() else {
        return Ok(None);
    };
    read_oracle(&raw, plugin_header_offset, base_address)
}

/// Reads a `Collection`'s `Oracle` adapter identified by `base_address`, if
/// attached. Callable identically on both backends.
#[cfg(not(feature = "pinocchio"))]
pub fn fetch_collection_oracle(
    info: &AccountInfo,
    base_address: Address,
) -> Result<Option<OracleInfo>> {
    if Owner::program_owner(info) != crate::ID {
        return Err(NaclacError::ConstraintOwner.into());
    }
    let solana_info = unsafe { info.to_lifetime() };
    let raw = solana_info
        .try_borrow_data()
        .map_err(|_| NaclacError::AccountBorrowFailed.err(0))?;
    let collection_view = crate::collection::CollectionView::from_bytes(&raw)?;
    let Some(plugin_header_offset) = collection_view.plugin_header_offset() else {
        return Ok(None);
    };
    read_oracle(&raw, plugin_header_offset, base_address)
}

/// Shared by `fetch_asset_oracle`/`fetch_collection_oracle` on both
/// backends: `Oracle` is an *external* plugin adapter, so this walks the
/// external registry (`find_external_registry_match`/`external_plugin_type`),
/// not the internal one (`find_plugin_offset`/`plugin_type` — those are for
/// the 19 internal plugins like `Royalties`/`Attributes`, a different
/// registry list entirely).
fn read_oracle(
    data: &[u8],
    plugin_header_offset: usize,
    base_address: Address,
) -> Result<Option<OracleInfo>> {
    let Some(m) = crate::external_plugin_registry::find_external_registry_match(
        data,
        plugin_header_offset,
        crate::external_plugin_registry::external_plugin_type::ORACLE,
        |candidate, data| {
            if data.len() < candidate.header_offset + 32 {
                return Err(NaclacError::AccountDataTooSmall.err(0));
            }
            let bytes: [u8; 32] = data
                [candidate.header_offset..candidate.header_offset + 32]
                .try_into()
                .unwrap();
            Ok(Address::new_from_array(bytes) == base_address)
        },
    )?
    else {
        return Ok(None);
    };
    Ok(Some(read_oracle_header(data, m.header_offset)?))
}

/// Reads a `Seed` at `offset`, shared by both backends. Returns the decoded
/// value and its encoded width. Unbounded on read (see this file's header)
/// — `Bytes` copies out however many bytes are actually there.
fn read_seed(data: &[u8], offset: usize) -> Result<(SeedOwned, usize)> {
    if data.len() <= offset {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    match data[offset] {
        0 => Ok((SeedOwned::Collection, 1)),
        1 => Ok((SeedOwned::Owner, 1)),
        2 => Ok((SeedOwned::Recipient, 1)),
        3 => Ok((SeedOwned::Asset, 1)),
        4 => {
            let end = checked_end(offset, 33)?;
            if data.len() < end {
                return Err(NaclacError::AccountDataTooSmall.err(0));
            }
            let bytes: [u8; 32] = data[offset + 1..end].try_into().unwrap();
            Ok((SeedOwned::Address(Address::new_from_array(bytes)), 33))
        }
        5 => {
            let prefix_end = checked_end(offset, 5)?;
            if data.len() < prefix_end {
                return Err(NaclacError::AccountDataTooSmall.err(0));
            }
            let len =
                u32::from_le_bytes(data[offset + 1..prefix_end].try_into().unwrap()) as usize;
            let payload_end = checked_end(prefix_end, len)?;
            if data.len() < payload_end {
                return Err(NaclacError::AccountDataTooSmall.err(0));
            }
            Ok((
                SeedOwned::Bytes(data[prefix_end..payload_end].to_vec()),
                5 + len,
            ))
        }
        _ => Err(NaclacError::InvalidInstructionData.err(0)),
    }
}

/// Reads an `ExtraAccount` at `offset`, shared by both backends. Returns the
/// decoded value and its encoded width.
fn read_extra_account(data: &[u8], offset: usize) -> Result<(ExtraAccountOwned, usize)> {
    if data.len() <= offset {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let read_bools = |o: usize| -> Result<(bool, bool)> {
        let end = checked_end(o, 2)?;
        if data.len() < end {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        Ok((data[o] != 0, data[o + 1] != 0))
    };
    match data[offset] {
        0 => {
            let (is_signer, is_writable) = read_bools(checked_end(offset, 1)?)?;
            Ok((
                ExtraAccountOwned::PreconfiguredProgram { is_signer, is_writable },
                3,
            ))
        }
        1 => {
            let (is_signer, is_writable) = read_bools(checked_end(offset, 1)?)?;
            Ok((
                ExtraAccountOwned::PreconfiguredCollection { is_signer, is_writable },
                3,
            ))
        }
        2 => {
            let (is_signer, is_writable) = read_bools(checked_end(offset, 1)?)?;
            Ok((
                ExtraAccountOwned::PreconfiguredOwner { is_signer, is_writable },
                3,
            ))
        }
        3 => {
            let (is_signer, is_writable) = read_bools(checked_end(offset, 1)?)?;
            Ok((
                ExtraAccountOwned::PreconfiguredRecipient { is_signer, is_writable },
                3,
            ))
        }
        4 => {
            let (is_signer, is_writable) = read_bools(checked_end(offset, 1)?)?;
            Ok((
                ExtraAccountOwned::PreconfiguredAsset { is_signer, is_writable },
                3,
            ))
        }
        5 => {
            let mut cursor = checked_end(offset, 1)?;
            let count_end = checked_end(cursor, 4)?;
            if data.len() < count_end {
                return Err(NaclacError::AccountDataTooSmall.err(0));
            }
            let count = u32::from_le_bytes(data[cursor..count_end].try_into().unwrap()) as usize;
            cursor = count_end;
            let mut seeds = crate::prelude::Vec::with_capacity(count);
            for _ in 0..count {
                let (seed, width) = read_seed(data, cursor)?;
                seeds.push(seed);
                cursor = checked_end(cursor, width)?;
            }
            if data.len() <= cursor {
                return Err(NaclacError::AccountDataTooSmall.err(0));
            }
            let custom_program_id = if data[cursor] == 1 {
                cursor = checked_end(cursor, 1)?;
                let end = checked_end(cursor, 32)?;
                if data.len() < end {
                    return Err(NaclacError::AccountDataTooSmall.err(0));
                }
                let bytes: [u8; 32] = data[cursor..end].try_into().unwrap();
                cursor = end;
                Some(Address::new_from_array(bytes))
            } else {
                cursor = checked_end(cursor, 1)?;
                None
            };
            let (is_signer, is_writable) = read_bools(cursor)?;
            cursor = checked_end(cursor, 2)?;
            Ok((
                ExtraAccountOwned::CustomPda {
                    seeds,
                    custom_program_id,
                    is_signer,
                    is_writable,
                },
                cursor - offset,
            ))
        }
        6 => {
            let end = checked_end(offset, 33)?;
            if data.len() < end {
                return Err(NaclacError::AccountDataTooSmall.err(0));
            }
            let bytes: [u8; 32] = data[offset + 1..end].try_into().unwrap();
            let (is_signer, is_writable) = read_bools(checked_end(offset, 33)?)?;
            Ok((
                ExtraAccountOwned::Address {
                    address: Address::new_from_array(bytes),
                    is_signer,
                    is_writable,
                },
                35,
            ))
        }
        _ => Err(NaclacError::InvalidInstructionData.err(0)),
    }
}

/// Reads a `ValidationResultsOffset` at `offset`, shared by both backends.
/// Returns the decoded value and its encoded width.
fn read_validation_results_offset(
    data: &[u8],
    offset: usize,
) -> Result<(ValidationResultsOffsetArg, usize)> {
    if data.len() <= offset {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    match data[offset] {
        0 => Ok((ValidationResultsOffsetArg::NoOffset, 1)),
        1 => Ok((ValidationResultsOffsetArg::Anchor, 1)),
        2 => {
            let end = checked_end(offset, 9)?;
            if data.len() < end {
                return Err(NaclacError::AccountDataTooSmall.err(0));
            }
            let n = u64::from_le_bytes(data[offset + 1..end].try_into().unwrap());
            Ok((ValidationResultsOffsetArg::Custom(n), 9))
        }
        _ => Err(NaclacError::InvalidInstructionData.err(0)),
    }
}

/// Reads an `Oracle` header (`{ base_address, base_address_config,
/// results_offset }`, no separate data blob) starting at `offset`, shared by
/// both backends' `fetch_asset_oracle`/`fetch_collection_oracle`.
fn read_oracle_header(data: &[u8], offset: usize) -> Result<OracleInfo> {
    let end = checked_end(offset, 32)?;
    if data.len() < end {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let bytes: [u8; 32] = data[offset..end].try_into().unwrap();
    let base_address = Address::new_from_array(bytes);
    let mut cursor = end;

    if data.len() <= cursor {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let base_address_config = if data[cursor] == 1 {
        cursor = checked_end(cursor, 1)?;
        let (ea, width) = read_extra_account(data, cursor)?;
        cursor = checked_end(cursor, width)?;
        Some(ea)
    } else {
        cursor = checked_end(cursor, 1)?;
        None
    };

    let (results_offset, _) = read_validation_results_offset(data, cursor)?;

    Ok(OracleInfo {
        base_address,
        base_address_config,
        results_offset,
    })
}

/// Reads an `Asset`'s `Oracle` adapter identified by `base_address`, if
/// attached. Callable identically on both backends.
#[cfg(feature = "pinocchio")]
pub fn fetch_asset_oracle(
    info: &AccountInfo,
    base_address: Address,
) -> Result<Option<OracleInfo>> {
    let asset_view = crate::asset::AssetView::try_from(info)?;
    let Some(plugin_header_offset) = asset_view.plugin_header_offset() else {
        return Ok(None);
    };
    read_oracle(info.data(), plugin_header_offset, base_address)
}

/// Reads a `Collection`'s `Oracle` adapter identified by `base_address`, if
/// attached. Callable identically on both backends.
#[cfg(feature = "pinocchio")]
pub fn fetch_collection_oracle(
    info: &AccountInfo,
    base_address: Address,
) -> Result<Option<OracleInfo>> {
    let collection_view = crate::collection::CollectionView::try_from(info)?;
    let Some(plugin_header_offset) = collection_view.plugin_header_offset() else {
        return Ok(None);
    };
    read_oracle(info.data(), plugin_header_offset, base_address)
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Proves `read_seed` never panics for any bytes/offset — including
    /// through its now-`checked_end`-guarded `Bytes` variant (tag 5).
    #[kani::proof]
    fn prove_read_seed_never_panics() {
        let data: [u8; 40] = kani::any();
        let offset: usize = kani::any();
        let _ = read_seed(&data, offset);
    }

    /// Proves `read_extra_account` never panics, including through its
    /// `CustomPda` variant (tag 5)'s nested `read_seed` loop — deliberately
    /// modest buffer/unwind given the nested-loop shape.
    #[kani::proof]
    #[kani::unwind(8)]
    fn prove_read_extra_account_never_panics() {
        let data: [u8; 40] = kani::any();
        let offset: usize = kani::any();
        let _ = read_extra_account(&data, offset);
    }

    /// Proves `read_validation_results_offset` never panics for any
    /// bytes/offset.
    #[kani::proof]
    fn prove_read_validation_results_offset_never_panics() {
        let data: [u8; 16] = kani::any();
        let offset: usize = kani::any();
        let _ = read_validation_results_offset(&data, offset);
    }

    /// Proves `read_oracle_header` never panics for any bytes/offset,
    /// including through its call into `read_extra_account`.
    #[kani::proof]
    #[kani::unwind(8)]
    fn prove_read_oracle_header_never_panics() {
        let data: [u8; 48] = kani::any();
        let offset: usize = kani::any();
        let _ = read_oracle_header(&data, offset);
    }
}
