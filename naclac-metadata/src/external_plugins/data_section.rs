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
/// bytes. On `solana`, `data` is a heap-copied `Vec<u8>`. On `pinocchio`,
/// `data` is a zero-copy `Span<u8>` view into the account's own live bytes
/// — see `AppDataInfo`'s doc comment for why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataSectionInfo {
    pub parent_key: LinkedDataKeyArg,
    pub schema: ExternalPluginAdapterSchemaArg,
    #[cfg(not(feature = "pinocchio"))]
    pub data: crate::prelude::Vec<u8>,
    #[cfg(feature = "pinocchio")]
    pub data: Span<u8>,
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
    let asset_view = crate::asset::AssetView::from_bytes(&raw)?;
    let Some(plugin_header_offset) = asset_view.plugin_header_offset() else {
        return Ok(None);
    };
    read_data_section(&raw, plugin_header_offset, target_parent_key)
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
    let collection_view = crate::collection::CollectionView::from_bytes(&raw)?;
    let Some(plugin_header_offset) = collection_view.plugin_header_offset() else {
        return Ok(None);
    };
    read_data_section(&raw, plugin_header_offset, target_parent_key)
}

/// `solana`-only: `DataSectionInfo::data` is a heap `Vec<u8>` on this
/// backend (see the struct's own doc comment), so this walks the same
/// external registry `find_external_registry_match` does on `pinocchio`,
/// but copies the matched data bytes into an owned `Vec` instead of a
/// `Span`.
#[cfg(not(feature = "pinocchio"))]
fn read_data_section(
    data: &[u8],
    plugin_header_offset: usize,
    target_parent_key: LinkedDataKeyArg,
) -> Result<Option<DataSectionInfo>> {
    let Some(m) = crate::external_plugin_registry::find_external_registry_match(
        data,
        plugin_header_offset,
        crate::external_plugin_registry::external_plugin_type::DATA_SECTION,
        |candidate, data| {
            let (candidate_parent_key, _) = read_linked_data_key(data, candidate.header_offset)?;
            Ok(candidate_parent_key == target_parent_key)
        },
    )?
    else {
        return Ok(None);
    };

    let (_, key_width) = read_linked_data_key(data, m.header_offset)?;
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
    let end = checked_end(data_offset, data_len)?;
    if data.len() < end {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    Ok(Some(DataSectionInfo {
        parent_key: target_parent_key,
        schema,
        data: data[data_offset..end].to_vec(),
    }))
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
    let Some(m) = crate::external_plugin_registry::find_external_registry_match(
        data,
        plugin_header_offset,
        crate::external_plugin_registry::external_plugin_type::DATA_SECTION,
        |candidate, data| {
            let (candidate_parent_key, _) = read_linked_data_key(data, candidate.header_offset)?;
            Ok(candidate_parent_key == target_parent_key)
        },
    )?
    else {
        return Ok(None);
    };

    let (_, key_width) = read_linked_data_key(data, m.header_offset)?;
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
            data: Span::new(&[]),
        }));
    };
    let end = checked_end(data_offset, data_len)?;
    if data.len() < end {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    Ok(Some(DataSectionInfo {
        parent_key: target_parent_key,
        schema,
        data: Span::new(&data[data_offset..end]),
    }))
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
    let Some(m) = crate::external_plugin_registry::find_external_registry_match(
        data,
        plugin_header_offset,
        crate::external_plugin_registry::external_plugin_type::DATA_SECTION,
        |candidate, data| {
            let (candidate_parent_key, _) = read_linked_data_key(data, candidate.header_offset)?;
            Ok(candidate_parent_key == target_parent_key)
        },
    )?
    else {
        return Ok(None);
    };

    let (_, key_width) = read_linked_data_key(data, m.header_offset)?;
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
            data: Span::new(&[]),
        }));
    };
    let end = checked_end(data_offset, data_len)?;
    if data.len() < end {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    Ok(Some(DataSectionInfo {
        parent_key: target_parent_key,
        schema,
        data: Span::new(&data[data_offset..end]),
    }))
}

/// Reads a `LinkedDataKey` at `offset`, shared by both backends. Returns the
/// decoded value and its encoded width.
fn read_linked_data_key(data: &[u8], offset: usize) -> Result<(LinkedDataKeyArg, usize)> {
    if data.len() <= offset {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    match data[offset] {
        0 => {
            let end = checked_end(offset, 33)?;
            if data.len() < end {
                return Err(NaclacError::AccountDataTooSmall.err(0));
            }
            let bytes: [u8; 32] = data[offset + 1..end].try_into().unwrap();
            Ok((
                LinkedDataKeyArg::LinkedLifecycleHook(Address::new_from_array(bytes)),
                33,
            ))
        }
        1 => {
            let (authority, authority_width) = crate::external_plugins::app_data::read_plugin_authority(
                data,
                checked_end(offset, 1)?,
            )?;
            Ok((LinkedDataKeyArg::LinkedAppData(authority), 1 + authority_width))
        }
        _ => Err(NaclacError::InvalidInstructionData.err(0)),
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Proves `read_linked_data_key` never panics for any bytes/offset,
    /// including through its now-`checked_end`-guarded variant-0 branch and
    /// its call into `read_plugin_authority`.
    #[kani::proof]
    fn prove_read_linked_data_key_never_panics() {
        let data: [u8; 40] = kani::any();
        let offset: usize = kani::any();
        let _ = read_linked_data_key(&data, offset);
    }
}
