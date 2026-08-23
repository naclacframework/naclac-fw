// ===========================================================================
// external_plugins/app_data.rs — the `AppData` external plugin adapter
// ===========================================================================

//! `AppData` — arbitrary data storage, writable by its own `data_authority`
//! (immutable once set, separate from the adapter's management authority).
//! Real layout verified against `mpl-core`'s `generated::types::AppData`/
//! `AppDataInitInfo`/`AppDataUpdateInfo`: header
//! `{ data_authority: PluginAuthority, schema: ExternalPluginAdapterSchema }`
//! stored at the `ExternalRegistryRecord`'s `offset`, with the actual data
//! bytes stored separately at `data_offset`/`data_len` (verified in the
//! real client crate's `AppDataWithData { base: AppData, data_offset: usize,
//! data_len: usize }` — no inline bytes, just an offset/length pair the
//! caller reads separately, matching this file's own design).
//!
//! Since multiple `AppData` instances can coexist (keyed by their own
//! `data_authority`, not the registry record's management `authority` —
//! see `external_plugin_registry.rs`'s header), locating one means
//! filtering the external registry by type first, then reading each
//! candidate's own header to find the matching `data_authority`.

use crate::prelude::*;

#[cfg(feature = "pinocchio")]
use crate::external_plugin_registry::external_plugin_type;

/// A fully-read `AppData` instance — header fields plus the actual data
/// bytes (owned, copied out of the account).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppDataInfo {
    pub data_authority: PluginAuthorityArg,
    pub schema: ExternalPluginAdapterSchemaArg,
    pub data: crate::prelude::Vec<u8>,
}

/// Attaches an `AppData` adapter to an `Asset` via `AddExternalPluginAdapterV1`.
pub fn attach_asset_app_data_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    data_authority: PluginAuthorityArg,
    schema: ExternalPluginAdapterSchemaArg,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let init_info =
            ::mpl_core::types::ExternalPluginAdapterInitInfo::AppData(
                ::mpl_core::types::AppDataInitInfo {
                    data_authority: to_real_plugin_authority(data_authority),
                    init_plugin_authority: None,
                    schema: Some(to_real_schema(schema)),
                },
            );
        add_asset_external_adapter_signed(program, accounts, init_info, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mut payload = crate::prelude::Vec::new();
        payload.push(2u8); // ExternalPluginAdapterInitInfo::AppData tag
        encode_plugin_authority(&mut payload, data_authority);
        payload.push(0u8); // init_plugin_authority: None
        payload.push(1u8); // schema: Some(...)
        payload.push(schema_tag(schema));
        add_asset_external_adapter_signed(program, accounts, &payload, signer_seeds)
    }
}

/// Attaches an `AppData` adapter to a `Collection` via
/// `AddCollectionExternalPluginAdapterV1`.
pub fn attach_collection_app_data_signed(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    data_authority: PluginAuthorityArg,
    schema: ExternalPluginAdapterSchemaArg,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let init_info =
            ::mpl_core::types::ExternalPluginAdapterInitInfo::AppData(
                ::mpl_core::types::AppDataInitInfo {
                    data_authority: to_real_plugin_authority(data_authority),
                    init_plugin_authority: None,
                    schema: Some(to_real_schema(schema)),
                },
            );
        add_collection_external_adapter_signed(program, accounts, init_info, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mut payload = crate::prelude::Vec::new();
        payload.push(2u8); // ExternalPluginAdapterInitInfo::AppData tag
        encode_plugin_authority(&mut payload, data_authority);
        payload.push(0u8); // init_plugin_authority: None
        payload.push(1u8); // schema: Some(...)
        payload.push(schema_tag(schema));
        add_collection_external_adapter_signed(program, accounts, &payload, signer_seeds)
    }
}

/// Updates an `AppData` adapter's `schema` on an `Asset` via
/// `UpdateExternalPluginAdapterV1`. `data_authority` identifies which
/// instance (see this file's header).
pub fn update_asset_app_data_schema_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    data_authority: PluginAuthorityArg,
    new_schema: ExternalPluginAdapterSchemaArg,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let key = ExternalPluginAdapterKeyArg::AppData(data_authority);
    #[cfg(not(feature = "pinocchio"))]
    {
        let update_info = ::mpl_core::types::ExternalPluginAdapterUpdateInfo::AppData(
            ::mpl_core::types::AppDataUpdateInfo {
                schema: Some(to_real_schema(new_schema)),
            },
        );
        update_asset_external_adapter_signed(program, accounts, key, update_info, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        // ExternalPluginAdapterUpdateInfo tag scheme has only 6 variants (no
        // DataSection) — AppData is index 2 here too, same as InitInfo/Key,
        // purely coincidentally (verified both enums independently, not
        // assumed identical).
        let update_info_bytes = crate::prelude::vec![
            2u8, // ExternalPluginAdapterUpdateInfo::AppData tag
            1u8, // schema: Some(...)
            schema_tag(new_schema),
        ];
        update_asset_external_adapter_signed(
            program,
            accounts,
            key,
            &update_info_bytes,
            signer_seeds,
        )
    }
}

/// Writes `bytes` to an `Asset`'s `AppData` adapter identified by
/// `data_authority`, via `WriteExternalPluginAdapterDataV1` (inline —
/// see `external_plugin_adapter.rs`'s header for the `buffer` alternative).
pub fn write_asset_app_data_signed(
    program: CpiHandle<'_>,
    accounts: WriteExternalAdapterAccounts<'_>,
    data_authority: PluginAuthorityArg,
    bytes: &[u8],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    write_asset_external_adapter_signed(
        program,
        accounts,
        ExternalPluginAdapterKeyArg::AppData(data_authority),
        WriteData::Inline(bytes),
        signer_seeds,
    )
}

/// Updates an `AppData` adapter's `schema` on a `Collection` via
/// `UpdateCollectionExternalPluginAdapterV1`.
pub fn update_collection_app_data_schema_signed(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    data_authority: PluginAuthorityArg,
    new_schema: ExternalPluginAdapterSchemaArg,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let key = ExternalPluginAdapterKeyArg::AppData(data_authority);
    #[cfg(not(feature = "pinocchio"))]
    {
        let update_info = ::mpl_core::types::ExternalPluginAdapterUpdateInfo::AppData(
            ::mpl_core::types::AppDataUpdateInfo {
                schema: Some(to_real_schema(new_schema)),
            },
        );
        update_collection_external_adapter_signed(program, accounts, key, update_info, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let update_info_bytes = crate::prelude::vec![
            2u8, // ExternalPluginAdapterUpdateInfo::AppData tag
            1u8, // schema: Some(...)
            schema_tag(new_schema),
        ];
        update_collection_external_adapter_signed(
            program,
            accounts,
            key,
            &update_info_bytes,
            signer_seeds,
        )
    }
}

/// Writes `bytes` to a `Collection`'s `AppData` adapter identified by
/// `data_authority`, via `WriteCollectionExternalPluginAdapterDataV1`.
pub fn write_collection_app_data_signed(
    program: CpiHandle<'_>,
    accounts: WriteCollectionExternalAdapterAccounts<'_>,
    data_authority: PluginAuthorityArg,
    bytes: &[u8],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    write_collection_external_adapter_signed(
        program,
        accounts,
        ExternalPluginAdapterKeyArg::AppData(data_authority),
        WriteData::Inline(bytes),
        signer_seeds,
    )
}

/// Removes an `Asset`'s `AppData` adapter identified by `data_authority`,
/// via `RemoveExternalPluginAdapterV1`.
pub fn remove_asset_app_data_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    data_authority: PluginAuthorityArg,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    remove_asset_external_adapter_signed(
        program,
        accounts,
        ExternalPluginAdapterKeyArg::AppData(data_authority),
        signer_seeds,
    )
}

/// Removes a `Collection`'s `AppData` adapter identified by
/// `data_authority`, via `RemoveCollectionExternalPluginAdapterV1`.
pub fn remove_collection_app_data_signed(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    data_authority: PluginAuthorityArg,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    remove_collection_external_adapter_signed(
        program,
        accounts,
        ExternalPluginAdapterKeyArg::AppData(data_authority),
        signer_seeds,
    )
}

/// Reads an `Asset`'s `AppData` adapter identified by `data_authority`, if
/// attached. Callable identically on both backends.
#[cfg(not(feature = "pinocchio"))]
pub fn fetch_asset_app_data(
    info: &AccountInfo,
    data_authority: PluginAuthorityArg,
) -> Result<Option<AppDataInfo>> {
    if Owner::program_owner(info) != crate::ID {
        return Err(NaclacError::ConstraintOwner.into());
    }
    let solana_info = unsafe { info.to_lifetime() };
    let raw = solana_info
        .try_borrow_data()
        .map_err(|_| NaclacError::AccountBorrowFailed.err(0))?;
    let asset = ::mpl_core::Asset::deserialize(&raw)
        .map_err(|_| NaclacError::DeserializationFailed.err(0))?;
    let target = to_real_plugin_authority(data_authority);
    for entry in &asset.external_plugin_adapter_list.app_data {
        if entry.base.data_authority == target {
            let data = raw[entry.data_offset..entry.data_offset + entry.data_len].to_vec();
            return Ok(Some(AppDataInfo {
                data_authority,
                schema: from_real_schema(&entry.base.schema),
                data,
            }));
        }
    }
    Ok(None)
}

/// Reads a `Collection`'s `AppData` adapter identified by `data_authority`,
/// if attached. Callable identically on both backends.
#[cfg(not(feature = "pinocchio"))]
pub fn fetch_collection_app_data(
    info: &AccountInfo,
    data_authority: PluginAuthorityArg,
) -> Result<Option<AppDataInfo>> {
    if Owner::program_owner(info) != crate::ID {
        return Err(NaclacError::ConstraintOwner.into());
    }
    let solana_info = unsafe { info.to_lifetime() };
    let raw = solana_info
        .try_borrow_data()
        .map_err(|_| NaclacError::AccountBorrowFailed.err(0))?;
    let collection = ::mpl_core::Collection::deserialize(&raw)
        .map_err(|_| NaclacError::DeserializationFailed.err(0))?;
    let target = to_real_plugin_authority(data_authority);
    for entry in &collection.external_plugin_adapter_list.app_data {
        if entry.base.data_authority == target {
            let data = raw[entry.data_offset..entry.data_offset + entry.data_len].to_vec();
            return Ok(Some(AppDataInfo {
                data_authority,
                schema: from_real_schema(&entry.base.schema),
                data,
            }));
        }
    }
    Ok(None)
}

#[cfg(not(feature = "pinocchio"))]
fn from_real_schema(
    schema: &::mpl_core::types::ExternalPluginAdapterSchema,
) -> ExternalPluginAdapterSchemaArg {
    match schema {
        ::mpl_core::types::ExternalPluginAdapterSchema::Binary => {
            ExternalPluginAdapterSchemaArg::Binary
        }
        ::mpl_core::types::ExternalPluginAdapterSchema::Json => {
            ExternalPluginAdapterSchemaArg::Json
        }
        ::mpl_core::types::ExternalPluginAdapterSchema::MsgPack => {
            ExternalPluginAdapterSchemaArg::MsgPack
        }
    }
}

/// Reads an `Asset`'s `AppData` adapter identified by `data_authority`, if
/// attached. Callable identically on both backends.
#[cfg(feature = "pinocchio")]
pub fn fetch_asset_app_data(
    info: &AccountInfo,
    data_authority: PluginAuthorityArg,
) -> Result<Option<AppDataInfo>> {
    let asset_view = crate::asset::AssetView::try_from(info)?;
    let Some(plugin_header_offset) = asset_view.plugin_header_offset() else {
        return Ok(None);
    };
    let data = info.data();
    let matches = crate::external_plugin_registry::find_external_registry_matches(
        data,
        plugin_header_offset,
        external_plugin_type::APP_DATA,
    )?;
    for m in matches {
        let (candidate_authority, authority_width) =
            read_plugin_authority(data, m.header_offset)?;
        if candidate_authority != data_authority {
            continue;
        }
        let schema_offset = m.header_offset + authority_width;
        if data.len() <= schema_offset {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        let schema = match data[schema_offset] {
            0 => ExternalPluginAdapterSchemaArg::Binary,
            1 => ExternalPluginAdapterSchemaArg::Json,
            2 => ExternalPluginAdapterSchemaArg::MsgPack,
            _ => return Err(NaclacError::InvalidInstructionData.err(0)),
        };
        let (Some(data_offset), Some(data_len)) = (m.data_offset, m.data_len) else {
            return Ok(Some(AppDataInfo {
                data_authority,
                schema,
                data: crate::prelude::Vec::new(),
            }));
        };
        if data.len() < data_offset + data_len {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        return Ok(Some(AppDataInfo {
            data_authority,
            schema,
            data: data[data_offset..data_offset + data_len].to_vec(),
        }));
    }
    Ok(None)
}

/// Reads a `Collection`'s `AppData` adapter identified by `data_authority`,
/// if attached. Callable identically on both backends.
#[cfg(feature = "pinocchio")]
pub fn fetch_collection_app_data(
    info: &AccountInfo,
    data_authority: PluginAuthorityArg,
) -> Result<Option<AppDataInfo>> {
    let collection_view = crate::collection::CollectionView::try_from(info)?;
    let Some(plugin_header_offset) = collection_view.plugin_header_offset() else {
        return Ok(None);
    };
    let data = info.data();
    let matches = crate::external_plugin_registry::find_external_registry_matches(
        data,
        plugin_header_offset,
        external_plugin_type::APP_DATA,
    )?;
    for m in matches {
        let (candidate_authority, authority_width) =
            read_plugin_authority(data, m.header_offset)?;
        if candidate_authority != data_authority {
            continue;
        }
        let schema_offset = m.header_offset + authority_width;
        if data.len() <= schema_offset {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        let schema = match data[schema_offset] {
            0 => ExternalPluginAdapterSchemaArg::Binary,
            1 => ExternalPluginAdapterSchemaArg::Json,
            2 => ExternalPluginAdapterSchemaArg::MsgPack,
            _ => return Err(NaclacError::InvalidInstructionData.err(0)),
        };
        let (Some(data_offset), Some(data_len)) = (m.data_offset, m.data_len) else {
            return Ok(Some(AppDataInfo {
                data_authority,
                schema,
                data: crate::prelude::Vec::new(),
            }));
        };
        if data.len() < data_offset + data_len {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        return Ok(Some(AppDataInfo {
            data_authority,
            schema,
            data: data[data_offset..data_offset + data_len].to_vec(),
        }));
    }
    Ok(None)
}

/// Reads a `PluginAuthority` at `offset` — `pinocchio`-only. Returns the
/// decoded value and its encoded width (1 byte for `None`/`Owner`/
/// `UpdateAuthority`, 33 for `Address { .. }`).
#[cfg(feature = "pinocchio")]
pub(crate) fn read_plugin_authority(
    data: &[u8],
    offset: usize,
) -> Result<(PluginAuthorityArg, usize)> {
    if data.len() <= offset {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    match data[offset] {
        0 => Ok((PluginAuthorityArg::None, 1)),
        1 => Ok((PluginAuthorityArg::Owner, 1)),
        2 => Ok((PluginAuthorityArg::UpdateAuthority, 1)),
        3 => {
            if data.len() < offset + 33 {
                return Err(NaclacError::AccountDataTooSmall.err(0));
            }
            let bytes: [u8; 32] = data[offset + 1..offset + 33].try_into().unwrap();
            Ok((PluginAuthorityArg::Address(Address::new_from_array(bytes)), 33))
        }
        _ => Err(NaclacError::InvalidInstructionData.err(0)),
    }
}
