// ===========================================================================
// external_plugins/data_section.rs — the `DataSection` external plugin adapter
// ===========================================================================

//! `DataSection` — the backing data store automatically created by a
//! `LinkedLifecycleHook`/`LinkedAppData` adapter, never attached directly:
//! `AddExternalPluginAdapterV1`/`WriteExternalPluginAdapterDataV1` both
//! reject it outright (see `external_plugin_adapter.rs`'s header — real
//! processor returns `CannotAddDataSection`). Read-only for that reason,
//! matching the internal `Groups` plugin's precedent
//! (`plugins/groups.rs`). Real layout verified against `mpl-core`'s
//! `generated::types::DataSection`: `{ parent_key: LinkedDataKey, schema:
//! ExternalPluginAdapterSchema }`, with the actual data bytes stored
//! separately at `data_offset`/`data_len` (`DataSectionWithData`), same
//! shape as `AppData`.

use crate::prelude::*;

/// A fully-read `DataSection` instance — header fields plus the actual data
/// bytes (owned, copied out of the account).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataSectionInfo {
    pub parent_key: LinkedDataKeyArg,
    pub schema: ExternalPluginAdapterSchemaArg,
    pub data: crate::prelude::Vec<u8>,
}

/// Reads an `Asset`'s `DataSection` adapter whose `parent_key` matches
/// `target_parent_key` (the `LinkedLifecycleHook`/`LinkedAppData` that owns
/// it), if attached. Callable identically on both backends.
#[cfg(not(feature = "pinocchio"))]
pub fn fetch_asset_data_section(
    info: &AccountInfo,
    target_parent_key: LinkedDataKeyArg,
) -> Result<Option<DataSectionInfo>> {
    if Owner::program_owner(info) != crate::ID {
        return Err(NaclacError::ConstraintOwner.into());
    }
    let solana_info = unsafe { info.to_lifetime() };
    let raw = solana_info
        .try_borrow_data()
        .map_err(|_| NaclacError::AccountBorrowFailed.err(0))?;
    let asset = ::mpl_core::Asset::deserialize(&raw)
        .map_err(|_| NaclacError::DeserializationFailed.err(0))?;
    let target = to_real_linked_data_key(target_parent_key);
    for entry in &asset.external_plugin_adapter_list.data_sections {
        if entry.base.parent_key == target {
            let data = raw[entry.data_offset..entry.data_offset + entry.data_len].to_vec();
            return Ok(Some(DataSectionInfo {
                parent_key: target_parent_key,
                schema: from_real_schema(&entry.base.schema),
                data,
            }));
        }
    }
    Ok(None)
}

/// Reads a `Collection`'s `DataSection` adapter whose `parent_key` matches
/// `target_parent_key`, if attached. Callable identically on both backends.
#[cfg(not(feature = "pinocchio"))]
pub fn fetch_collection_data_section(
    info: &AccountInfo,
    target_parent_key: LinkedDataKeyArg,
) -> Result<Option<DataSectionInfo>> {
    if Owner::program_owner(info) != crate::ID {
        return Err(NaclacError::ConstraintOwner.into());
    }
    let solana_info = unsafe { info.to_lifetime() };
    let raw = solana_info
        .try_borrow_data()
        .map_err(|_| NaclacError::AccountBorrowFailed.err(0))?;
    let collection = ::mpl_core::Collection::deserialize(&raw)
        .map_err(|_| NaclacError::DeserializationFailed.err(0))?;
    let target = to_real_linked_data_key(target_parent_key);
    for entry in &collection.external_plugin_adapter_list.data_sections {
        if entry.base.parent_key == target {
            let data = raw[entry.data_offset..entry.data_offset + entry.data_len].to_vec();
            return Ok(Some(DataSectionInfo {
                parent_key: target_parent_key,
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

/// Reads an `Asset`'s `DataSection` adapter whose `parent_key` matches
/// `target_parent_key`, if attached. Callable identically on both backends.
#[cfg(feature = "pinocchio")]
pub fn fetch_asset_data_section(
    info: &AccountInfo,
    target_parent_key: LinkedDataKeyArg,
) -> Result<Option<DataSectionInfo>> {
    let asset_view = crate::asset::AssetView::try_from(info)?;
    let Some(plugin_header_offset) = asset_view.plugin_header_offset() else {
        return Ok(None);
    };
    let data = info.data();
    let matches = crate::external_plugin_registry::find_external_registry_matches(
        data,
        plugin_header_offset,
        crate::external_plugin_registry::external_plugin_type::DATA_SECTION,
    )?;
    for m in matches {
        let (candidate_parent_key, key_width) = read_linked_data_key(data, m.header_offset)?;
        if candidate_parent_key != target_parent_key {
            continue;
        }
        let schema_offset = m.header_offset + key_width;
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
            return Ok(Some(DataSectionInfo {
                parent_key: target_parent_key,
                schema,
                data: crate::prelude::Vec::new(),
            }));
        };
        if data.len() < data_offset + data_len {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        return Ok(Some(DataSectionInfo {
            parent_key: target_parent_key,
            schema,
            data: data[data_offset..data_offset + data_len].to_vec(),
        }));
    }
    Ok(None)
}

/// Reads a `Collection`'s `DataSection` adapter whose `parent_key` matches
/// `target_parent_key`, if attached. Callable identically on both backends.
#[cfg(feature = "pinocchio")]
pub fn fetch_collection_data_section(
    info: &AccountInfo,
    target_parent_key: LinkedDataKeyArg,
) -> Result<Option<DataSectionInfo>> {
    let collection_view = crate::collection::CollectionView::try_from(info)?;
    let Some(plugin_header_offset) = collection_view.plugin_header_offset() else {
        return Ok(None);
    };
    let data = info.data();
    let matches = crate::external_plugin_registry::find_external_registry_matches(
        data,
        plugin_header_offset,
        crate::external_plugin_registry::external_plugin_type::DATA_SECTION,
    )?;
    for m in matches {
        let (candidate_parent_key, key_width) = read_linked_data_key(data, m.header_offset)?;
        if candidate_parent_key != target_parent_key {
            continue;
        }
        let schema_offset = m.header_offset + key_width;
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
            return Ok(Some(DataSectionInfo {
                parent_key: target_parent_key,
                schema,
                data: crate::prelude::Vec::new(),
            }));
        };
        if data.len() < data_offset + data_len {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        return Ok(Some(DataSectionInfo {
            parent_key: target_parent_key,
            schema,
            data: data[data_offset..data_offset + data_len].to_vec(),
        }));
    }
    Ok(None)
}

/// Reads a `LinkedDataKey` at `offset` — `pinocchio`-only. Returns the
/// decoded value and its encoded width.
#[cfg(feature = "pinocchio")]
fn read_linked_data_key(data: &[u8], offset: usize) -> Result<(LinkedDataKeyArg, usize)> {
    if data.len() <= offset {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    match data[offset] {
        0 => {
            if data.len() < offset + 33 {
                return Err(NaclacError::AccountDataTooSmall.err(0));
            }
            let bytes: [u8; 32] = data[offset + 1..offset + 33].try_into().unwrap();
            Ok((
                LinkedDataKeyArg::LinkedLifecycleHook(Address::new_from_array(bytes)),
                33,
            ))
        }
        1 => {
            let (authority, authority_width) =
                crate::external_plugins::app_data::read_plugin_authority(data, offset + 1)?;
            Ok((LinkedDataKeyArg::LinkedAppData(authority), 1 + authority_width))
        }
        _ => Err(NaclacError::InvalidInstructionData.err(0)),
    }
}
