// ===========================================================================
// external_plugin_adapter.rs — shared external plugin adapter CPI mechanics
// ===========================================================================

//! Shared account structs, key/schema encoding, and low-level CPI mechanics
//! for the 8 external plugin adapter instructions
//! (`AddExternalPluginAdapterV1`=disc `22`/`AddCollectionExternalPluginAdapterV1`=`23`,
//! `RemoveExternalPluginAdapterV1`=`24`/`RemoveCollectionExternalPluginAdapterV1`=`25`,
//! `UpdateExternalPluginAdapterV1`=`26`/`UpdateCollectionExternalPluginAdapterV1`=`27`,
//! `WriteExternalPluginAdapterDataV1`=`28`/`WriteCollectionExternalPluginAdapterDataV1`=`29`).
//! Concrete per-adapter-type files (`external_plugins/app_data.rs`, ...)
//! build on top of this.
//!
//! **Add/Remove/Update's account shape is verified identical to
//! `AddPluginV1`/`AddCollectionPluginV1`** (`plugin.rs`) — same 6/5 fields,
//! same order, same writable/signer flags — so this file reuses
//! `AddAssetPluginAccounts`/`AddCollectionPluginAccounts` directly rather
//! than redeclaring them. `Write` is the one exception: it has an extra
//! `buffer` account (readonly, slotted between `authority` and
//! `system_program`), so it gets its own account struct.
//!
//! **`Write`'s `data`/`buffer` duality — scoped deliberately.** The real
//! instruction accepts *either* inline `data: Some(bytes)` *or* a
//! pre-populated `buffer` account (`data: None`) for writes too large to
//! fit in one transaction's instruction data. This crate does not build
//! any mechanism for *populating* a buffer account — that's no different
//! in kind from any other "stage large data across several transactions"
//! problem, and isn't part of what `WriteExternalPluginAdapterDataV1`
//! itself does either (verified: it only ever *reads* `buffer.data.borrow()`,
//! never writes to it). `write_signed`'s `WriteData` enum exposes both
//! paths from the real instruction; using `Buffer` just means the caller
//! is responsible for having populated that account themselves, by
//! whatever means.
//!
//! **`AddExternalPluginAdapterV1`/`WriteExternalPluginAdapterDataV1` both
//! reject `DataSection`/`LinkedLifecycleHook`/`LinkedAppData` outright** —
//! verified directly in the real processor:
//! ```rust
//! match &args.init_info {
//!     ExternalPluginAdapterInitInfo::LinkedLifecycleHook(_)
//!     | ExternalPluginAdapterInitInfo::LinkedAppData(_) => {
//!         return Err(MplCoreError::InvalidPluginAdapterTarget.into())
//!     }
//!     ExternalPluginAdapterInitInfo::DataSection(_) => {
//!         return Err(MplCoreError::CannotAddDataSection.into())
//!     }
//!     _ => (),
//! }
//! ```
//! — these three are managed exclusively by their "parent" plugin
//! (`LinkedAppData`/`LinkedLifecycleHook` create their own backing
//! `DataSection` automatically), the same "program-managed only" shape
//! already seen for the internal `Groups` plugin. `external_plugins/data_section.rs`
//! is read-only for this reason.

use crate::prelude::*;

/// Accounts consumed by `write_signed` — the one external-adapter
/// instruction with a different shape than `AddPluginV1`
/// (an extra `buffer` account).
pub struct WriteExternalAdapterAccounts<'a> {
    pub asset: CpiHandleMut<'a>,
    pub collection: Option<CpiHandleMut<'a>>,
    pub payer: CpiHandleMut<'a>,
    pub authority: Option<CpiHandle<'a>>,
    pub buffer: Option<CpiHandle<'a>>,
    pub system_program: CpiHandle<'a>,
    pub log_wrapper: Option<CpiHandle<'a>>,
}

/// Accounts consumed by `write_collection_external_adapter_signed` — the
/// collection-level analogue of `WriteExternalAdapterAccounts` (no separate
/// `asset`/`collection` split, just `collection`, matching
/// `AddCollectionPluginAccounts` plus a `buffer` account).
pub struct WriteCollectionExternalAdapterAccounts<'a> {
    pub collection: CpiHandleMut<'a>,
    pub payer: CpiHandleMut<'a>,
    pub authority: Option<CpiHandle<'a>>,
    pub buffer: Option<CpiHandle<'a>>,
    pub system_program: CpiHandle<'a>,
    pub log_wrapper: Option<CpiHandle<'a>>,
}

/// What to write, mirroring the real instruction's `(data, buffer)`
/// duality — see this file's header for why buffer-population isn't
/// built here.
pub enum WriteData<'a> {
    /// Write these bytes directly (must fit in one transaction).
    Inline(&'a [u8]),
    /// Read from an already-populated `buffer` account instead.
    Buffer,
}

/// Backend-neutral mirror of the real `ExternalPluginAdapterSchema` enum
/// (`Binary=0, Json=1, MsgPack=2`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalPluginAdapterSchemaArg {
    Binary,
    Json,
    MsgPack,
}

pub(crate) fn schema_tag(schema: ExternalPluginAdapterSchemaArg) -> u8 {
    match schema {
        ExternalPluginAdapterSchemaArg::Binary => 0,
        ExternalPluginAdapterSchemaArg::Json => 1,
        ExternalPluginAdapterSchemaArg::MsgPack => 2,
    }
}

/// Backend-neutral mirror of the real `LinkedDataKey` enum (`LinkedLifecycleHook(Pubkey)=0,
/// LinkedAppData(PluginAuthority)=1`) — needed by `DataSection`'s key/init
/// info even though `DataSection` itself can't be attached directly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkedDataKeyArg {
    LinkedLifecycleHook(Address),
    LinkedAppData(PluginAuthorityArg),
}

/// Maximum Borsh-encoded width of a `LinkedDataKeyArg`/`LinkedDataKey`
/// value: 1-byte tag + the wider of a 32-byte `Address` or a
/// `PluginAuthority` (`MAX_PLUGIN_AUTHORITY_ENCODED_LEN`).
#[cfg(feature = "pinocchio")]
pub(crate) const MAX_LINKED_DATA_KEY_ENCODED_LEN: usize = 1 + MAX_PLUGIN_AUTHORITY_ENCODED_LEN;

/// `solana`-only mirror of `encode_linked_data_key`, writing directly into
/// a heap `Vec<u8>` instead of through the `pinocchio`-only `ByteSink`
/// machinery.
#[cfg(not(feature = "pinocchio"))]
pub(crate) fn encode_linked_data_key_owned(data: &mut crate::prelude::Vec<u8>, key: LinkedDataKeyArg) {
    match key {
        LinkedDataKeyArg::LinkedLifecycleHook(addr) => {
            data.push(0u8);
            data.extend_from_slice(addr.as_ref());
        }
        LinkedDataKeyArg::LinkedAppData(auth) => {
            data.push(1u8);
            encode_plugin_authority_owned(data, auth);
        }
    }
}

#[cfg(feature = "pinocchio")]
pub(crate) fn encode_linked_data_key<S: crate::fixed_buf::ByteSink>(
    data: &mut S,
    key: LinkedDataKeyArg,
) {
    match key {
        LinkedDataKeyArg::LinkedLifecycleHook(addr) => {
            data.push(0u8);
            data.extend_from_slice(addr.as_ref());
        }
        LinkedDataKeyArg::LinkedAppData(auth) => {
            data.push(1u8);
            encode_plugin_authority(data, auth);
        }
    }
}

/// Backend-neutral mirror of the real `ExternalPluginAdapterKey` enum —
/// identifies *which* attached adapter instance an operation targets.
/// Verified variant order:
/// `LifecycleHook(Pubkey)=0, Oracle(Pubkey)=1, AppData(PluginAuthority)=2,
/// LinkedLifecycleHook(Pubkey)=3, LinkedAppData(PluginAuthority)=4,
/// DataSection(LinkedDataKey)=5, AgentIdentity=6`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalPluginAdapterKeyArg {
    LifecycleHook(Address),
    Oracle(Address),
    AppData(PluginAuthorityArg),
    LinkedLifecycleHook(Address),
    LinkedAppData(PluginAuthorityArg),
    DataSection(LinkedDataKeyArg),
    AgentIdentity,
}

/// Maximum Borsh-encoded width of an `ExternalPluginAdapterKeyArg`/
/// `ExternalPluginAdapterKey` value — the `DataSection` variant (1-byte
/// outer tag + a full `LinkedDataKeyArg`) is the widest.
#[cfg(feature = "pinocchio")]
pub(crate) const MAX_EXTERNAL_KEY_ENCODED_LEN: usize = 1 + MAX_LINKED_DATA_KEY_ENCODED_LEN;

/// `solana`-only mirror of `encode_key`, writing directly into a heap
/// `Vec<u8>` instead of through the `pinocchio`-only `ByteSink` machinery.
#[cfg(not(feature = "pinocchio"))]
pub(crate) fn encode_key_owned(data: &mut crate::prelude::Vec<u8>, key: ExternalPluginAdapterKeyArg) {
    match key {
        ExternalPluginAdapterKeyArg::LifecycleHook(addr) => {
            data.push(0u8);
            data.extend_from_slice(addr.as_ref());
        }
        ExternalPluginAdapterKeyArg::Oracle(addr) => {
            data.push(1u8);
            data.extend_from_slice(addr.as_ref());
        }
        ExternalPluginAdapterKeyArg::AppData(auth) => {
            data.push(2u8);
            encode_plugin_authority_owned(data, auth);
        }
        ExternalPluginAdapterKeyArg::LinkedLifecycleHook(addr) => {
            data.push(3u8);
            data.extend_from_slice(addr.as_ref());
        }
        ExternalPluginAdapterKeyArg::LinkedAppData(auth) => {
            data.push(4u8);
            encode_plugin_authority_owned(data, auth);
        }
        ExternalPluginAdapterKeyArg::DataSection(linked_key) => {
            data.push(5u8);
            encode_linked_data_key_owned(data, linked_key);
        }
        ExternalPluginAdapterKeyArg::AgentIdentity => {
            data.push(6u8);
        }
    }
}

#[cfg(feature = "pinocchio")]
pub(crate) fn encode_key<S: crate::fixed_buf::ByteSink>(
    data: &mut S,
    key: ExternalPluginAdapterKeyArg,
) {
    match key {
        ExternalPluginAdapterKeyArg::LifecycleHook(addr) => {
            data.push(0u8);
            data.extend_from_slice(addr.as_ref());
        }
        ExternalPluginAdapterKeyArg::Oracle(addr) => {
            data.push(1u8);
            data.extend_from_slice(addr.as_ref());
        }
        ExternalPluginAdapterKeyArg::AppData(auth) => {
            data.push(2u8);
            encode_plugin_authority(data, auth);
        }
        ExternalPluginAdapterKeyArg::LinkedLifecycleHook(addr) => {
            data.push(3u8);
            data.extend_from_slice(addr.as_ref());
        }
        ExternalPluginAdapterKeyArg::LinkedAppData(auth) => {
            data.push(4u8);
            encode_plugin_authority(data, auth);
        }
        ExternalPluginAdapterKeyArg::DataSection(linked_key) => {
            data.push(5u8);
            encode_linked_data_key(data, linked_key);
        }
        ExternalPluginAdapterKeyArg::AgentIdentity => {
            data.push(6u8);
        }
    }
}

/// Attaches an external plugin adapter to an `Asset` via a real
/// `AddExternalPluginAdapterV1` CPI (discriminator `22`). `ix_data` is the
/// complete, already-encoded instruction data — discriminator `22` followed
/// by the tag-and-payload-encoded `ExternalPluginAdapterInitInfo` value
/// (same 7-variant tag scheme as `ExternalPluginAdapterKeyArg`, since
/// `InitInfo` and `Key` share the same variant set — verified from the real
/// SDK, unlike `UpdateInfo` which omits `DataSection`) — built by the
/// caller, identically on both backends. See this file's header —
/// `init_info` must not be `LinkedLifecycleHook`/`LinkedAppData`/
/// `DataSection`, the real processor rejects all three.
pub fn add_asset_external_adapter_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    ix_data: &[u8],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        add_asset_plugin_signed_raw(program, accounts, ix_data, signer_seeds)
    }
    #[cfg(feature = "pinocchio")]
    {
        add_asset_plugin_signed_pinocchio_raw(program, accounts, ix_data, signer_seeds)
    }
}

/// Attaches an external plugin adapter to a `Collection` via a real
/// `AddCollectionExternalPluginAdapterV1` CPI (discriminator `23`).
/// `ix_data` is the complete instruction data (discriminator `23` + init
/// info), built by the caller — see `add_asset_external_adapter_signed`.
pub fn add_collection_external_adapter_signed(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    ix_data: &[u8],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        add_collection_plugin_signed_raw(program, accounts, ix_data, signer_seeds)
    }
    #[cfg(feature = "pinocchio")]
    {
        add_collection_plugin_signed_pinocchio_raw(program, accounts, ix_data, signer_seeds)
    }
}

/// Removes an adapter identified by `key` from an `Asset` via a real
/// `RemoveExternalPluginAdapterV1` CPI (discriminator `24`).
pub fn remove_asset_external_adapter_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    key: ExternalPluginAdapterKeyArg,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = crate::prelude::Vec::new();
        data.push(24u8); // RemoveExternalPluginAdapterV1 discriminator
        encode_key_owned(&mut data, key);
        add_asset_plugin_signed_raw(program, accounts, &data, signer_seeds)
    }
    #[cfg(feature = "pinocchio")]
    {
        let mut data = crate::fixed_buf::FixedBuf::<{ 1 + MAX_EXTERNAL_KEY_ENCODED_LEN }>::new();
        data.push(24u8); // RemoveExternalPluginAdapterV1 discriminator
        encode_key(&mut data, key);
        add_asset_plugin_signed_pinocchio_raw(program, accounts, data.as_slice(), signer_seeds)
    }
}

/// Removes an adapter identified by `key` from a `Collection` via a real
/// `RemoveCollectionExternalPluginAdapterV1` CPI (discriminator `25`).
pub fn remove_collection_external_adapter_signed(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    key: ExternalPluginAdapterKeyArg,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = crate::prelude::Vec::new();
        data.push(25u8); // RemoveCollectionExternalPluginAdapterV1 discriminator
        encode_key_owned(&mut data, key);
        add_collection_plugin_signed_raw(program, accounts, &data, signer_seeds)
    }
    #[cfg(feature = "pinocchio")]
    {
        let mut data = crate::fixed_buf::FixedBuf::<{ 1 + MAX_EXTERNAL_KEY_ENCODED_LEN }>::new();
        data.push(25u8); // RemoveCollectionExternalPluginAdapterV1 discriminator
        encode_key(&mut data, key);
        add_collection_plugin_signed_pinocchio_raw(program, accounts, data.as_slice(), signer_seeds)
    }
}

/// Updates an `Asset`'s adapter identified by `key` via a real
/// `UpdateExternalPluginAdapterV1` CPI (discriminator `26`). `ix_data` is
/// the complete instruction data — discriminator `26` + encoded `key` + the
/// Borsh-tag-and-payload-encoded `ExternalPluginAdapterUpdateInfo` value
/// (the tag scheme here has only 6 variants — no `DataSection` — see this
/// file's header for why `DataSection` can't be updated directly either) —
/// built by the caller, identically on both backends.
pub fn update_asset_external_adapter_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    ix_data: &[u8],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        add_asset_plugin_signed_raw(program, accounts, ix_data, signer_seeds)
    }
    #[cfg(feature = "pinocchio")]
    {
        add_asset_plugin_signed_pinocchio_raw(program, accounts, ix_data, signer_seeds)
    }
}

/// Updates a `Collection`'s adapter identified by `key` via a real
/// `UpdateCollectionExternalPluginAdapterV1` CPI (discriminator `27`).
/// `ix_data` is the complete instruction data (discriminator `27` + encoded
/// `key` + update info), built by the caller — see
/// `update_asset_external_adapter_signed`.
pub fn update_collection_external_adapter_signed(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    ix_data: &[u8],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        add_collection_plugin_signed_raw(program, accounts, ix_data, signer_seeds)
    }
    #[cfg(feature = "pinocchio")]
    {
        add_collection_plugin_signed_pinocchio_raw(program, accounts, ix_data, signer_seeds)
    }
}

/// `solana`-only: shared account-list/CPI-invoke mechanics for asset-level
/// external-adapter instructions with already-encoded `data` bytes and the
/// `AddPluginV1`-shaped account list (`AddAssetPluginAccounts`) — same
/// account order/flags as `plugin.rs`'s `add_asset_plugin_signed`.
#[cfg(not(feature = "pinocchio"))]
fn add_asset_plugin_signed_raw(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    data: &[u8],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let accounts_meta = vec![
        solana_program::instruction::AccountMeta::new(accounts.asset.address(), false),
        match &accounts.collection {
            Some(c) => solana_program::instruction::AccountMeta::new(c.address(), false),
            None => solana_program::instruction::AccountMeta::new_readonly(crate::ID, false),
        },
        solana_program::instruction::AccountMeta::new(accounts.payer.address(), true),
        match &accounts.authority {
            Some(a) => {
                solana_program::instruction::AccountMeta::new_readonly(a.address(), true)
            }
            None => solana_program::instruction::AccountMeta::new_readonly(crate::ID, false),
        },
        solana_program::instruction::AccountMeta::new_readonly(
            accounts.system_program.address(),
            false,
        ),
        match &accounts.log_wrapper {
            Some(l) => {
                solana_program::instruction::AccountMeta::new_readonly(l.address(), false)
            }
            None => solana_program::instruction::AccountMeta::new_readonly(crate::ID, false),
        },
    ];
    let ix = solana_program::instruction::Instruction {
        program_id: crate::ID,
        accounts: accounts_meta,
        data: data.to_vec(),
    };

    let cpi_accounts = [
        CpiHandle::from(accounts.asset),
        accounts
            .collection
            .map(CpiHandle::from)
            .unwrap_or_else(|| program.clone()),
        CpiHandle::from(accounts.payer),
        accounts.authority.unwrap_or_else(|| program.clone()),
        accounts.system_program,
        accounts.log_wrapper.unwrap_or_else(|| program.clone()),
        program,
    ];
    crate::cpi::invoke_signed(&ix, &cpi_accounts, signer_seeds)
}

/// `solana`-only: shared account-list/CPI-invoke mechanics for
/// collection-level external-adapter instructions — see
/// `add_asset_plugin_signed_raw`.
#[cfg(not(feature = "pinocchio"))]
fn add_collection_plugin_signed_raw(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    data: &[u8],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let accounts_meta = vec![
        solana_program::instruction::AccountMeta::new(accounts.collection.address(), false),
        solana_program::instruction::AccountMeta::new(accounts.payer.address(), true),
        match &accounts.authority {
            Some(a) => {
                solana_program::instruction::AccountMeta::new_readonly(a.address(), true)
            }
            None => solana_program::instruction::AccountMeta::new_readonly(crate::ID, false),
        },
        solana_program::instruction::AccountMeta::new_readonly(
            accounts.system_program.address(),
            false,
        ),
        match &accounts.log_wrapper {
            Some(l) => {
                solana_program::instruction::AccountMeta::new_readonly(l.address(), false)
            }
            None => solana_program::instruction::AccountMeta::new_readonly(crate::ID, false),
        },
    ];
    let ix = solana_program::instruction::Instruction {
        program_id: crate::ID,
        accounts: accounts_meta,
        data: data.to_vec(),
    };

    let cpi_accounts = [
        CpiHandle::from(accounts.collection),
        CpiHandle::from(accounts.payer),
        accounts.authority.unwrap_or_else(|| program.clone()),
        accounts.system_program,
        accounts.log_wrapper.unwrap_or_else(|| program.clone()),
        program,
    ];
    crate::cpi::invoke_signed(&ix, &cpi_accounts, signer_seeds)
}

/// `pinocchio`-only: shared account-list/CPI-invoke mechanics for
/// asset-level external-adapter instructions with already-encoded `data`
/// bytes and the `AddPluginV1`-shaped account list (`AddAssetPluginAccounts`).
#[cfg(feature = "pinocchio")]
fn add_asset_plugin_signed_pinocchio_raw(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    data: &[u8],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let asset_handle: CpiHandle<'_> = CpiHandle::from(accounts.asset);
    let payer_handle: CpiHandle<'_> = CpiHandle::from(accounts.payer);
    let collection_is_some = accounts.collection.is_some();
    let authority_is_some = accounts.authority.is_some();
    let collection_handle = accounts.collection.map(CpiHandle::from).unwrap_or(program);
    let authority_handle = accounts.authority.unwrap_or(program);
    let log_wrapper_handle = accounts.log_wrapper.unwrap_or(program);

    let ix_accounts = [
        ::pinocchio::instruction::InstructionAccount::writable(asset_handle.info.view.address()),
        if collection_is_some {
            ::pinocchio::instruction::InstructionAccount::writable(
                collection_handle.info.view.address(),
            )
        } else {
            ::pinocchio::instruction::InstructionAccount::readonly(
                collection_handle.info.view.address(),
            )
        },
        ::pinocchio::instruction::InstructionAccount::writable_signer(
            payer_handle.info.view.address(),
        ),
        if authority_is_some {
            ::pinocchio::instruction::InstructionAccount::readonly_signer(
                authority_handle.info.view.address(),
            )
        } else {
            ::pinocchio::instruction::InstructionAccount::readonly(
                authority_handle.info.view.address(),
            )
        },
        ::pinocchio::instruction::InstructionAccount::readonly(
            accounts.system_program.info.view.address(),
        ),
        ::pinocchio::instruction::InstructionAccount::readonly(
            log_wrapper_handle.info.view.address(),
        ),
    ];
    let instruction = ::pinocchio::instruction::InstructionView {
        program_id: program.info.view.address(),
        accounts: &ix_accounts,
        data,
    };
    let handles = [
        asset_handle,
        collection_handle,
        payer_handle,
        authority_handle,
        accounts.system_program,
        log_wrapper_handle,
    ];
    crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles, signer_seeds)
}

/// `pinocchio`-only: shared account-list/CPI-invoke mechanics for
/// collection-level external-adapter instructions — see
/// `add_asset_plugin_signed_pinocchio_raw`.
#[cfg(feature = "pinocchio")]
fn add_collection_plugin_signed_pinocchio_raw(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    data: &[u8],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let collection_handle: CpiHandle<'_> = CpiHandle::from(accounts.collection);
    let payer_handle: CpiHandle<'_> = CpiHandle::from(accounts.payer);
    let authority_is_some = accounts.authority.is_some();
    let authority_handle = accounts.authority.unwrap_or(program);
    let log_wrapper_handle = accounts.log_wrapper.unwrap_or(program);

    let ix_accounts = [
        ::pinocchio::instruction::InstructionAccount::writable(
            collection_handle.info.view.address(),
        ),
        ::pinocchio::instruction::InstructionAccount::writable_signer(
            payer_handle.info.view.address(),
        ),
        if authority_is_some {
            ::pinocchio::instruction::InstructionAccount::readonly_signer(
                authority_handle.info.view.address(),
            )
        } else {
            ::pinocchio::instruction::InstructionAccount::readonly(
                authority_handle.info.view.address(),
            )
        },
        ::pinocchio::instruction::InstructionAccount::readonly(
            accounts.system_program.info.view.address(),
        ),
        ::pinocchio::instruction::InstructionAccount::readonly(
            log_wrapper_handle.info.view.address(),
        ),
    ];
    let instruction = ::pinocchio::instruction::InstructionView {
        program_id: program.info.view.address(),
        accounts: &ix_accounts,
        data,
    };
    let handles = [
        collection_handle,
        payer_handle,
        authority_handle,
        accounts.system_program,
        log_wrapper_handle,
    ];
    crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles, signer_seeds)
}

/// Writes to an `Asset`'s external plugin adapter identified by `key` via a
/// real `WriteExternalPluginAdapterDataV1` CPI (discriminator `28`).
pub fn write_asset_external_adapter_signed(
    program: CpiHandle<'_>,
    accounts: WriteExternalAdapterAccounts<'_>,
    key: ExternalPluginAdapterKeyArg,
    write_data: WriteData<'_>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = crate::prelude::Vec::new();
        data.push(28u8); // WriteExternalPluginAdapterDataV1 discriminator
        encode_key_owned(&mut data, key);
        match write_data {
            WriteData::Inline(bytes) => {
                data.push(1u8);
                data.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                data.extend_from_slice(bytes);
            }
            WriteData::Buffer => data.push(0u8),
        }

        let accounts_meta = vec![
            solana_program::instruction::AccountMeta::new(accounts.asset.address(), false),
            match &accounts.collection {
                Some(c) => solana_program::instruction::AccountMeta::new(c.address(), false),
                None => solana_program::instruction::AccountMeta::new_readonly(crate::ID, false),
            },
            solana_program::instruction::AccountMeta::new(accounts.payer.address(), true),
            match &accounts.authority {
                Some(a) => {
                    solana_program::instruction::AccountMeta::new_readonly(a.address(), true)
                }
                None => solana_program::instruction::AccountMeta::new_readonly(crate::ID, false),
            },
            match &accounts.buffer {
                Some(b) => {
                    solana_program::instruction::AccountMeta::new_readonly(b.address(), false)
                }
                None => solana_program::instruction::AccountMeta::new_readonly(crate::ID, false),
            },
            solana_program::instruction::AccountMeta::new_readonly(
                accounts.system_program.address(),
                false,
            ),
            match &accounts.log_wrapper {
                Some(l) => {
                    solana_program::instruction::AccountMeta::new_readonly(l.address(), false)
                }
                None => solana_program::instruction::AccountMeta::new_readonly(crate::ID, false),
            },
        ];
        let ix = solana_program::instruction::Instruction {
            program_id: crate::ID,
            accounts: accounts_meta,
            data,
        };

        let cpi_accounts = [
            CpiHandle::from(accounts.asset),
            accounts
                .collection
                .map(CpiHandle::from)
                .unwrap_or_else(|| program.clone()),
            CpiHandle::from(accounts.payer),
            accounts.authority.unwrap_or_else(|| program.clone()),
            accounts.buffer.unwrap_or_else(|| program.clone()),
            accounts.system_program,
            accounts.log_wrapper.unwrap_or_else(|| program.clone()),
            program,
        ];
        crate::cpi::invoke_signed(&ix, &cpi_accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mut data = crate::prelude::Vec::new();
        data.push(28u8); // WriteExternalPluginAdapterDataV1 discriminator
        encode_key(&mut data, key);
        match write_data {
            WriteData::Inline(bytes) => {
                data.push(1u8);
                data.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                data.extend_from_slice(bytes);
            }
            WriteData::Buffer => data.push(0u8),
        }

        let asset_handle: CpiHandle<'_> = CpiHandle::from(accounts.asset);
        let payer_handle: CpiHandle<'_> = CpiHandle::from(accounts.payer);
        let collection_is_some = accounts.collection.is_some();
        let authority_is_some = accounts.authority.is_some();
        let collection_handle = accounts.collection.map(CpiHandle::from).unwrap_or(program);
        let authority_handle = accounts.authority.unwrap_or(program);
        let buffer_handle = accounts.buffer.unwrap_or(program);
        let log_wrapper_handle = accounts.log_wrapper.unwrap_or(program);

        let ix_accounts = [
            ::pinocchio::instruction::InstructionAccount::writable(
                asset_handle.info.view.address(),
            ),
            if collection_is_some {
                ::pinocchio::instruction::InstructionAccount::writable(
                    collection_handle.info.view.address(),
                )
            } else {
                ::pinocchio::instruction::InstructionAccount::readonly(
                    collection_handle.info.view.address(),
                )
            },
            ::pinocchio::instruction::InstructionAccount::writable_signer(
                payer_handle.info.view.address(),
            ),
            if authority_is_some {
                ::pinocchio::instruction::InstructionAccount::readonly_signer(
                    authority_handle.info.view.address(),
                )
            } else {
                ::pinocchio::instruction::InstructionAccount::readonly(
                    authority_handle.info.view.address(),
                )
            },
            ::pinocchio::instruction::InstructionAccount::readonly(
                buffer_handle.info.view.address(),
            ),
            ::pinocchio::instruction::InstructionAccount::readonly(
                accounts.system_program.info.view.address(),
            ),
            ::pinocchio::instruction::InstructionAccount::readonly(
                log_wrapper_handle.info.view.address(),
            ),
        ];
        let instruction = ::pinocchio::instruction::InstructionView {
            program_id: program.info.view.address(),
            accounts: &ix_accounts,
            data: &data,
        };
        let handles = [
            asset_handle,
            collection_handle,
            payer_handle,
            authority_handle,
            buffer_handle,
            accounts.system_program,
            log_wrapper_handle,
        ];
        crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles, signer_seeds)
    }
}

/// Writes to a `Collection`'s external plugin adapter identified by `key`
/// via a real `WriteCollectionExternalPluginAdapterDataV1` CPI
/// (discriminator `29`).
pub fn write_collection_external_adapter_signed(
    program: CpiHandle<'_>,
    accounts: WriteCollectionExternalAdapterAccounts<'_>,
    key: ExternalPluginAdapterKeyArg,
    write_data: WriteData<'_>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = crate::prelude::Vec::new();
        data.push(29u8); // WriteCollectionExternalPluginAdapterDataV1 discriminator
        encode_key_owned(&mut data, key);
        match write_data {
            WriteData::Inline(bytes) => {
                data.push(1u8);
                data.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                data.extend_from_slice(bytes);
            }
            WriteData::Buffer => data.push(0u8),
        }

        let accounts_meta = vec![
            solana_program::instruction::AccountMeta::new(accounts.collection.address(), false),
            solana_program::instruction::AccountMeta::new(accounts.payer.address(), true),
            match &accounts.authority {
                Some(a) => {
                    solana_program::instruction::AccountMeta::new_readonly(a.address(), true)
                }
                None => solana_program::instruction::AccountMeta::new_readonly(crate::ID, false),
            },
            match &accounts.buffer {
                Some(b) => {
                    solana_program::instruction::AccountMeta::new_readonly(b.address(), false)
                }
                None => solana_program::instruction::AccountMeta::new_readonly(crate::ID, false),
            },
            solana_program::instruction::AccountMeta::new_readonly(
                accounts.system_program.address(),
                false,
            ),
            match &accounts.log_wrapper {
                Some(l) => {
                    solana_program::instruction::AccountMeta::new_readonly(l.address(), false)
                }
                None => solana_program::instruction::AccountMeta::new_readonly(crate::ID, false),
            },
        ];
        let ix = solana_program::instruction::Instruction {
            program_id: crate::ID,
            accounts: accounts_meta,
            data,
        };

        let cpi_accounts = [
            CpiHandle::from(accounts.collection),
            CpiHandle::from(accounts.payer),
            accounts.authority.unwrap_or_else(|| program.clone()),
            accounts.buffer.unwrap_or_else(|| program.clone()),
            accounts.system_program,
            accounts.log_wrapper.unwrap_or_else(|| program.clone()),
            program,
        ];
        crate::cpi::invoke_signed(&ix, &cpi_accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mut data = crate::prelude::Vec::new();
        data.push(29u8); // WriteCollectionExternalPluginAdapterDataV1 discriminator
        encode_key(&mut data, key);
        match write_data {
            WriteData::Inline(bytes) => {
                data.push(1u8);
                data.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                data.extend_from_slice(bytes);
            }
            WriteData::Buffer => data.push(0u8),
        }

        let collection_handle: CpiHandle<'_> = CpiHandle::from(accounts.collection);
        let payer_handle: CpiHandle<'_> = CpiHandle::from(accounts.payer);
        let authority_is_some = accounts.authority.is_some();
        let authority_handle = accounts.authority.unwrap_or(program);
        let buffer_handle = accounts.buffer.unwrap_or(program);
        let log_wrapper_handle = accounts.log_wrapper.unwrap_or(program);

        let ix_accounts = [
            ::pinocchio::instruction::InstructionAccount::writable(
                collection_handle.info.view.address(),
            ),
            ::pinocchio::instruction::InstructionAccount::writable_signer(
                payer_handle.info.view.address(),
            ),
            if authority_is_some {
                ::pinocchio::instruction::InstructionAccount::readonly_signer(
                    authority_handle.info.view.address(),
                )
            } else {
                ::pinocchio::instruction::InstructionAccount::readonly(
                    authority_handle.info.view.address(),
                )
            },
            ::pinocchio::instruction::InstructionAccount::readonly(
                buffer_handle.info.view.address(),
            ),
            ::pinocchio::instruction::InstructionAccount::readonly(
                accounts.system_program.info.view.address(),
            ),
            ::pinocchio::instruction::InstructionAccount::readonly(
                log_wrapper_handle.info.view.address(),
            ),
        ];
        let instruction = ::pinocchio::instruction::InstructionView {
            program_id: program.info.view.address(),
            accounts: &ix_accounts,
            data: &data,
        };
        let handles = [
            collection_handle,
            payer_handle,
            authority_handle,
            buffer_handle,
            accounts.system_program,
            log_wrapper_handle,
        ];
        crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles, signer_seeds)
    }
}
