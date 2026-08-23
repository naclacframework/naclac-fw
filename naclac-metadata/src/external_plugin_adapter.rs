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

#[cfg(not(feature = "pinocchio"))]
pub(crate) fn to_real_schema(
    schema: ExternalPluginAdapterSchemaArg,
) -> ::mpl_core::types::ExternalPluginAdapterSchema {
    match schema {
        ExternalPluginAdapterSchemaArg::Binary => {
            ::mpl_core::types::ExternalPluginAdapterSchema::Binary
        }
        ExternalPluginAdapterSchemaArg::Json => {
            ::mpl_core::types::ExternalPluginAdapterSchema::Json
        }
        ExternalPluginAdapterSchemaArg::MsgPack => {
            ::mpl_core::types::ExternalPluginAdapterSchema::MsgPack
        }
    }
}

#[cfg(feature = "pinocchio")]
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

#[cfg(not(feature = "pinocchio"))]
pub(crate) fn to_real_linked_data_key(key: LinkedDataKeyArg) -> ::mpl_core::types::LinkedDataKey {
    match key {
        LinkedDataKeyArg::LinkedLifecycleHook(addr) => {
            ::mpl_core::types::LinkedDataKey::LinkedLifecycleHook(addr)
        }
        LinkedDataKeyArg::LinkedAppData(auth) => {
            ::mpl_core::types::LinkedDataKey::LinkedAppData(to_real_plugin_authority(auth))
        }
    }
}

#[cfg(feature = "pinocchio")]
pub(crate) fn encode_linked_data_key(data: &mut crate::prelude::Vec<u8>, key: LinkedDataKeyArg) {
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

#[cfg(not(feature = "pinocchio"))]
pub(crate) fn to_real_key(
    key: ExternalPluginAdapterKeyArg,
) -> ::mpl_core::types::ExternalPluginAdapterKey {
    match key {
        ExternalPluginAdapterKeyArg::LifecycleHook(addr) => {
            ::mpl_core::types::ExternalPluginAdapterKey::LifecycleHook(addr)
        }
        ExternalPluginAdapterKeyArg::Oracle(addr) => {
            ::mpl_core::types::ExternalPluginAdapterKey::Oracle(addr)
        }
        ExternalPluginAdapterKeyArg::AppData(auth) => {
            ::mpl_core::types::ExternalPluginAdapterKey::AppData(to_real_plugin_authority(auth))
        }
        ExternalPluginAdapterKeyArg::LinkedLifecycleHook(addr) => {
            ::mpl_core::types::ExternalPluginAdapterKey::LinkedLifecycleHook(addr)
        }
        ExternalPluginAdapterKeyArg::LinkedAppData(auth) => {
            ::mpl_core::types::ExternalPluginAdapterKey::LinkedAppData(to_real_plugin_authority(
                auth,
            ))
        }
        ExternalPluginAdapterKeyArg::DataSection(linked_key) => {
            ::mpl_core::types::ExternalPluginAdapterKey::DataSection(to_real_linked_data_key(
                linked_key,
            ))
        }
        ExternalPluginAdapterKeyArg::AgentIdentity => {
            ::mpl_core::types::ExternalPluginAdapterKey::AgentIdentity
        }
    }
}

#[cfg(feature = "pinocchio")]
pub(crate) fn encode_key(data: &mut crate::prelude::Vec<u8>, key: ExternalPluginAdapterKeyArg) {
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

/// `solana`-only: attaches an external plugin adapter to an `Asset` via a
/// real `AddExternalPluginAdapterV1` CPI (discriminator `22`). See this
/// file's header — `init_info` must not be `LinkedLifecycleHook`/
/// `LinkedAppData`/`DataSection`, the real processor rejects all three.
#[cfg(not(feature = "pinocchio"))]
pub fn add_asset_external_adapter_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    init_info: ::mpl_core::types::ExternalPluginAdapterInitInfo,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let ix = ::mpl_core::instructions::AddExternalPluginAdapterV1 {
        asset: accounts.asset.address(),
        collection: accounts.collection.as_ref().map(|c| c.address()),
        payer: accounts.payer.address(),
        authority: accounts.authority.as_ref().map(|a| a.address()),
        system_program: accounts.system_program.address(),
        log_wrapper: accounts.log_wrapper.as_ref().map(|l| l.address()),
    }
    .instruction(::mpl_core::instructions::AddExternalPluginAdapterV1InstructionArgs {
        init_info,
    });

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

/// `pinocchio`-only: attaches an external plugin adapter to an `Asset` via
/// a hand-built `AddExternalPluginAdapterV1` CPI. `init_info_bytes` is the
/// already tag-and-payload-encoded `ExternalPluginAdapterInitInfo` value
/// (same 7-variant tag scheme as `ExternalPluginAdapterKeyArg`, since
/// `InitInfo` and `Key` share the same variant set — verified from the
/// real SDK, unlike `UpdateInfo` which omits `DataSection`).
#[cfg(feature = "pinocchio")]
pub fn add_asset_external_adapter_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    init_info_bytes: &[u8],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let mut data = crate::prelude::Vec::with_capacity(1 + init_info_bytes.len());
    data.push(22u8); // AddExternalPluginAdapterV1 discriminator
    data.extend_from_slice(init_info_bytes);
    add_asset_plugin_signed_pinocchio_raw(program, accounts, &data, signer_seeds)
}

/// `solana`-only: attaches an external plugin adapter to a `Collection`
/// via a real `AddCollectionExternalPluginAdapterV1` CPI (discriminator
/// `23`).
#[cfg(not(feature = "pinocchio"))]
pub fn add_collection_external_adapter_signed(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    init_info: ::mpl_core::types::ExternalPluginAdapterInitInfo,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let ix = ::mpl_core::instructions::AddCollectionExternalPluginAdapterV1 {
        collection: accounts.collection.address(),
        payer: accounts.payer.address(),
        authority: accounts.authority.as_ref().map(|a| a.address()),
        system_program: accounts.system_program.address(),
        log_wrapper: accounts.log_wrapper.as_ref().map(|l| l.address()),
    }
    .instruction(
        ::mpl_core::instructions::AddCollectionExternalPluginAdapterV1InstructionArgs {
            init_info,
        },
    );

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

/// `pinocchio`-only: attaches an external plugin adapter to a `Collection`
/// via a hand-built `AddCollectionExternalPluginAdapterV1` CPI.
#[cfg(feature = "pinocchio")]
pub fn add_collection_external_adapter_signed(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    init_info_bytes: &[u8],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let mut data = crate::prelude::Vec::with_capacity(1 + init_info_bytes.len());
    data.push(23u8); // AddCollectionExternalPluginAdapterV1 discriminator
    data.extend_from_slice(init_info_bytes);
    add_collection_plugin_signed_pinocchio_raw(program, accounts, &data, signer_seeds)
}

/// `solana`-only: removes an adapter identified by `key` from an `Asset`
/// via a real `RemoveExternalPluginAdapterV1` CPI.
#[cfg(not(feature = "pinocchio"))]
pub fn remove_asset_external_adapter_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    key: ExternalPluginAdapterKeyArg,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let ix = ::mpl_core::instructions::RemoveExternalPluginAdapterV1 {
        asset: accounts.asset.address(),
        collection: accounts.collection.as_ref().map(|c| c.address()),
        payer: accounts.payer.address(),
        authority: accounts.authority.as_ref().map(|a| a.address()),
        system_program: accounts.system_program.address(),
        log_wrapper: accounts.log_wrapper.as_ref().map(|l| l.address()),
    }
    .instruction(::mpl_core::instructions::RemoveExternalPluginAdapterV1InstructionArgs {
        key: to_real_key(key),
    });

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

/// `pinocchio`-only: removes an adapter identified by `key` from an
/// `Asset` via a hand-built `RemoveExternalPluginAdapterV1` CPI.
#[cfg(feature = "pinocchio")]
pub fn remove_asset_external_adapter_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    key: ExternalPluginAdapterKeyArg,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let mut data = crate::prelude::Vec::with_capacity(35);
    data.push(24u8); // RemoveExternalPluginAdapterV1 discriminator
    encode_key(&mut data, key);
    add_asset_plugin_signed_pinocchio_raw(program, accounts, &data, signer_seeds)
}

/// `solana`-only: removes an adapter identified by `key` from a
/// `Collection` via a real `RemoveCollectionExternalPluginAdapterV1` CPI.
#[cfg(not(feature = "pinocchio"))]
pub fn remove_collection_external_adapter_signed(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    key: ExternalPluginAdapterKeyArg,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let ix = ::mpl_core::instructions::RemoveCollectionExternalPluginAdapterV1 {
        collection: accounts.collection.address(),
        payer: accounts.payer.address(),
        authority: accounts.authority.as_ref().map(|a| a.address()),
        system_program: accounts.system_program.address(),
        log_wrapper: accounts.log_wrapper.as_ref().map(|l| l.address()),
    }
    .instruction(
        ::mpl_core::instructions::RemoveCollectionExternalPluginAdapterV1InstructionArgs {
            key: to_real_key(key),
        },
    );

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

/// `pinocchio`-only: removes an adapter identified by `key` from a
/// `Collection` via a hand-built `RemoveCollectionExternalPluginAdapterV1`
/// CPI.
#[cfg(feature = "pinocchio")]
pub fn remove_collection_external_adapter_signed(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    key: ExternalPluginAdapterKeyArg,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let mut data = crate::prelude::Vec::with_capacity(35);
    data.push(25u8); // RemoveCollectionExternalPluginAdapterV1 discriminator
    encode_key(&mut data, key);
    add_collection_plugin_signed_pinocchio_raw(program, accounts, &data, signer_seeds)
}

/// `solana`-only: updates an `Asset`'s adapter identified by `key` via a
/// real `UpdateExternalPluginAdapterV1` CPI (discriminator `26`).
#[cfg(not(feature = "pinocchio"))]
pub fn update_asset_external_adapter_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    key: ExternalPluginAdapterKeyArg,
    update_info: ::mpl_core::types::ExternalPluginAdapterUpdateInfo,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let ix = ::mpl_core::instructions::UpdateExternalPluginAdapterV1 {
        asset: accounts.asset.address(),
        collection: accounts.collection.as_ref().map(|c| c.address()),
        payer: accounts.payer.address(),
        authority: accounts.authority.as_ref().map(|a| a.address()),
        system_program: accounts.system_program.address(),
        log_wrapper: accounts.log_wrapper.as_ref().map(|l| l.address()),
    }
    .instruction(::mpl_core::instructions::UpdateExternalPluginAdapterV1InstructionArgs {
        key: to_real_key(key),
        update_info,
    });

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

/// `pinocchio`-only: updates an `Asset`'s adapter identified by `key` via a
/// hand-built `UpdateExternalPluginAdapterV1` CPI. `update_info_bytes` is
/// the already Borsh-tag-and-payload-encoded `ExternalPluginAdapterUpdateInfo`
/// value (the tag scheme here has only 6 variants — no `DataSection` — see
/// this file's header for why `DataSection` can't be updated directly
/// either).
#[cfg(feature = "pinocchio")]
pub fn update_asset_external_adapter_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    key: ExternalPluginAdapterKeyArg,
    update_info_bytes: &[u8],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let mut data = crate::prelude::Vec::with_capacity(35 + update_info_bytes.len());
    data.push(26u8); // UpdateExternalPluginAdapterV1 discriminator
    encode_key(&mut data, key);
    data.extend_from_slice(update_info_bytes);
    add_asset_plugin_signed_pinocchio_raw(program, accounts, &data, signer_seeds)
}

/// `solana`-only: updates a `Collection`'s adapter identified by `key` via
/// a real `UpdateCollectionExternalPluginAdapterV1` CPI (discriminator
/// `27`).
#[cfg(not(feature = "pinocchio"))]
pub fn update_collection_external_adapter_signed(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    key: ExternalPluginAdapterKeyArg,
    update_info: ::mpl_core::types::ExternalPluginAdapterUpdateInfo,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let ix = ::mpl_core::instructions::UpdateCollectionExternalPluginAdapterV1 {
        collection: accounts.collection.address(),
        payer: accounts.payer.address(),
        authority: accounts.authority.as_ref().map(|a| a.address()),
        system_program: accounts.system_program.address(),
        log_wrapper: accounts.log_wrapper.as_ref().map(|l| l.address()),
    }
    .instruction(
        ::mpl_core::instructions::UpdateCollectionExternalPluginAdapterV1InstructionArgs {
            key: to_real_key(key),
            update_info,
        },
    );

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

/// `pinocchio`-only: updates a `Collection`'s adapter identified by `key`
/// via a hand-built `UpdateCollectionExternalPluginAdapterV1` CPI.
#[cfg(feature = "pinocchio")]
pub fn update_collection_external_adapter_signed(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    key: ExternalPluginAdapterKeyArg,
    update_info_bytes: &[u8],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let mut data = crate::prelude::Vec::with_capacity(35 + update_info_bytes.len());
    data.push(27u8); // UpdateCollectionExternalPluginAdapterV1 discriminator
    encode_key(&mut data, key);
    data.extend_from_slice(update_info_bytes);
    add_collection_plugin_signed_pinocchio_raw(program, accounts, &data, signer_seeds)
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
        let ix = ::mpl_core::instructions::WriteExternalPluginAdapterDataV1 {
            asset: accounts.asset.address(),
            collection: accounts.collection.as_ref().map(|c| c.address()),
            payer: accounts.payer.address(),
            authority: accounts.authority.as_ref().map(|a| a.address()),
            buffer: accounts.buffer.as_ref().map(|b| b.address()),
            system_program: accounts.system_program.address(),
            log_wrapper: accounts.log_wrapper.as_ref().map(|l| l.address()),
        }
        .instruction(::mpl_core::instructions::WriteExternalPluginAdapterDataV1InstructionArgs {
            key: to_real_key(key),
            data: match write_data {
                WriteData::Inline(bytes) => Some(bytes.to_vec()),
                WriteData::Buffer => None,
            },
        });

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
        let ix = ::mpl_core::instructions::WriteCollectionExternalPluginAdapterDataV1 {
            collection: accounts.collection.address(),
            payer: accounts.payer.address(),
            authority: accounts.authority.as_ref().map(|a| a.address()),
            buffer: accounts.buffer.as_ref().map(|b| b.address()),
            system_program: accounts.system_program.address(),
            log_wrapper: accounts.log_wrapper.as_ref().map(|l| l.address()),
        }
        .instruction(
            ::mpl_core::instructions::WriteCollectionExternalPluginAdapterDataV1InstructionArgs {
                key: to_real_key(key),
                data: match write_data {
                    WriteData::Inline(bytes) => Some(bytes.to_vec()),
                    WriteData::Buffer => None,
                },
            },
        );

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
