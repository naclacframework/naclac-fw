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

use crate::external_plugin_registry::external_plugin_type;

/// Max encoded width of `ExternalPluginAdapterKeyArg::AppData`/
/// `ExternalPluginAdapterInitInfo::AppData`'s tag (1 byte) + a
/// `PluginAuthorityArg`/`PluginAuthority` (`MAX_PLUGIN_AUTHORITY_ENCODED_LEN`).
#[cfg(feature = "pinocchio")]
const APP_DATA_KEY_ENCODED_LEN: usize = 1 + MAX_PLUGIN_AUTHORITY_ENCODED_LEN;

/// Max total instruction data for `attach_asset_app_data_signed`/
/// `attach_collection_app_data_signed`: 1-byte ix discriminator +
/// `AppDataInitInfo` (`APP_DATA_KEY_ENCODED_LEN` for the `data_authority`
/// tag+value, always exactly 1 byte for `init_plugin_authority: None`,
/// always exactly 2 bytes for `schema: Some(..)`).
#[cfg(feature = "pinocchio")]
const ATTACH_APP_DATA_IX_LEN: usize = 1 + APP_DATA_KEY_ENCODED_LEN + 1 + 2;

/// Max total instruction data for `update_asset_app_data_schema_signed`/
/// `update_collection_app_data_schema_signed`: 1-byte ix discriminator +
/// encoded `ExternalPluginAdapterKeyArg::AppData` (`APP_DATA_KEY_ENCODED_LEN`)
/// + `AppDataUpdateInfo` (always exactly 3 bytes: variant tag + `schema:
/// Some(..)`).
#[cfg(feature = "pinocchio")]
const UPDATE_APP_DATA_IX_LEN: usize = 1 + APP_DATA_KEY_ENCODED_LEN + 3;

/// A fully-read `AppData` instance — header fields plus the actual data
/// bytes. On `solana`, `data` is a heap-copied `Vec<u8>` (the account is
/// only borrowed for the duration of the read). On `pinocchio`, `data` is a
/// zero-copy `Span<u8>` view directly into the account's own live bytes —
/// no heap allocation, no artificial size cap, since it reflects however
/// many bytes are actually stored on-chain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppDataInfo {
    pub data_authority: PluginAuthorityArg,
    pub schema: ExternalPluginAdapterSchemaArg,
    #[cfg(not(feature = "pinocchio"))]
    pub data: crate::prelude::Vec<u8>,
    #[cfg(feature = "pinocchio")]
    pub data: Span<u8>,
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
        let mut data = crate::prelude::Vec::new();
        data.push(22u8); // AddExternalPluginAdapterV1 discriminator
        data.push(2u8); // ExternalPluginAdapterInitInfo::AppData tag
        encode_plugin_authority_owned(&mut data, data_authority);
        data.push(0u8); // init_plugin_authority: None
        data.push(1u8); // schema: Some(...)
        data.push(schema_tag(schema));
        add_asset_external_adapter_signed(program, accounts, &data, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mut data = crate::fixed_buf::FixedBuf::<ATTACH_APP_DATA_IX_LEN>::new();
        data.push(22u8); // AddExternalPluginAdapterV1 discriminator
        data.push(2u8); // ExternalPluginAdapterInitInfo::AppData tag
        encode_plugin_authority(&mut data, data_authority);
        data.push(0u8); // init_plugin_authority: None
        data.push(1u8); // schema: Some(...)
        data.push(schema_tag(schema));
        add_asset_external_adapter_signed(program, accounts, data.as_slice(), signer_seeds)
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
        let mut data = crate::prelude::Vec::new();
        data.push(23u8); // AddCollectionExternalPluginAdapterV1 discriminator
        data.push(2u8); // ExternalPluginAdapterInitInfo::AppData tag
        encode_plugin_authority_owned(&mut data, data_authority);
        data.push(0u8); // init_plugin_authority: None
        data.push(1u8); // schema: Some(...)
        data.push(schema_tag(schema));
        add_collection_external_adapter_signed(program, accounts, &data, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mut data = crate::fixed_buf::FixedBuf::<ATTACH_APP_DATA_IX_LEN>::new();
        data.push(23u8); // AddCollectionExternalPluginAdapterV1 discriminator
        data.push(2u8); // ExternalPluginAdapterInitInfo::AppData tag
        encode_plugin_authority(&mut data, data_authority);
        data.push(0u8); // init_plugin_authority: None
        data.push(1u8); // schema: Some(...)
        data.push(schema_tag(schema));
        add_collection_external_adapter_signed(program, accounts, data.as_slice(), signer_seeds)
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
    // `ExternalPluginAdapterUpdateInfo` tag scheme has only 6 variants (no
    // `DataSection`) — `AppData` is index 2 here too, same as `InitInfo`/
    // `Key`, purely coincidentally (verified both enums independently, not
    // assumed identical).
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = crate::prelude::Vec::new();
        data.push(26u8); // UpdateExternalPluginAdapterV1 discriminator
        data.push(2u8); // ExternalPluginAdapterKeyArg::AppData tag
        encode_plugin_authority_owned(&mut data, data_authority);
        data.push(2u8); // ExternalPluginAdapterUpdateInfo::AppData tag
        data.push(1u8); // schema: Some(...)
        data.push(schema_tag(new_schema));
        update_asset_external_adapter_signed(program, accounts, &data, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mut data = crate::fixed_buf::FixedBuf::<UPDATE_APP_DATA_IX_LEN>::new();
        data.push(26u8); // UpdateExternalPluginAdapterV1 discriminator
        data.push(2u8); // ExternalPluginAdapterKeyArg::AppData tag
        encode_plugin_authority(&mut data, data_authority);
        data.push(2u8); // ExternalPluginAdapterUpdateInfo::AppData tag
        data.push(1u8); // schema: Some(...)
        data.push(schema_tag(new_schema));
        update_asset_external_adapter_signed(program, accounts, data.as_slice(), signer_seeds)
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
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = crate::prelude::Vec::new();
        data.push(27u8); // UpdateCollectionExternalPluginAdapterV1 discriminator
        data.push(2u8); // ExternalPluginAdapterKeyArg::AppData tag
        encode_plugin_authority_owned(&mut data, data_authority);
        data.push(2u8); // ExternalPluginAdapterUpdateInfo::AppData tag
        data.push(1u8); // schema: Some(...)
        data.push(schema_tag(new_schema));
        update_collection_external_adapter_signed(program, accounts, &data, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mut data = crate::fixed_buf::FixedBuf::<UPDATE_APP_DATA_IX_LEN>::new();
        data.push(27u8); // UpdateCollectionExternalPluginAdapterV1 discriminator
        data.push(2u8); // ExternalPluginAdapterKeyArg::AppData tag
        encode_plugin_authority(&mut data, data_authority);
        data.push(2u8); // ExternalPluginAdapterUpdateInfo::AppData tag
        data.push(1u8); // schema: Some(...)
        data.push(schema_tag(new_schema));
        update_collection_external_adapter_signed(program, accounts, data.as_slice(), signer_seeds)
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
    let asset_view = crate::asset::AssetView::from_bytes(&raw)?;
    let Some(plugin_header_offset) = asset_view.plugin_header_offset() else {
        return Ok(None);
    };
    read_app_data(&raw, plugin_header_offset, data_authority)
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
    let collection_view = crate::collection::CollectionView::from_bytes(&raw)?;
    let Some(plugin_header_offset) = collection_view.plugin_header_offset() else {
        return Ok(None);
    };
    read_app_data(&raw, plugin_header_offset, data_authority)
}

/// `solana`-only: `AppDataInfo::data` is a heap `Vec<u8>` on this backend
/// (see the struct's own doc comment), so this walks the same external
/// registry `find_external_registry_match` does on `pinocchio`, but copies
/// the matched data bytes into an owned `Vec` instead of a `Span`.
#[cfg(not(feature = "pinocchio"))]
fn read_app_data(
    data: &[u8],
    plugin_header_offset: usize,
    data_authority: PluginAuthorityArg,
) -> Result<Option<AppDataInfo>> {
    let Some(m) = crate::external_plugin_registry::find_external_registry_match(
        data,
        plugin_header_offset,
        external_plugin_type::APP_DATA,
        |candidate, data| {
            let (candidate_authority, _) = read_plugin_authority(data, candidate.header_offset)?;
            Ok(candidate_authority == data_authority)
        },
    )?
    else {
        return Ok(None);
    };

    let (_, authority_width) = read_plugin_authority(data, m.header_offset)?;
    let schema_offset = checked_end(m.header_offset, authority_width)?;
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
    let end = checked_end(data_offset, data_len)?;
    if data.len() < end {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    Ok(Some(AppDataInfo {
        data_authority,
        schema,
        data: data[data_offset..end].to_vec(),
    }))
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
    let Some(m) = crate::external_plugin_registry::find_external_registry_match(
        data,
        plugin_header_offset,
        external_plugin_type::APP_DATA,
        |candidate, data| {
            let (candidate_authority, _) = read_plugin_authority(data, candidate.header_offset)?;
            Ok(candidate_authority == data_authority)
        },
    )?
    else {
        return Ok(None);
    };

    let (_, authority_width) = read_plugin_authority(data, m.header_offset)?;
    let schema_offset = checked_end(m.header_offset, authority_width)?;
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
            data: Span::new(&[]),
        }));
    };
    let end = checked_end(data_offset, data_len)?;
    if data.len() < end {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    Ok(Some(AppDataInfo {
        data_authority,
        schema,
        data: Span::new(&data[data_offset..end]),
    }))
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
    let Some(m) = crate::external_plugin_registry::find_external_registry_match(
        data,
        plugin_header_offset,
        external_plugin_type::APP_DATA,
        |candidate, data| {
            let (candidate_authority, _) = read_plugin_authority(data, candidate.header_offset)?;
            Ok(candidate_authority == data_authority)
        },
    )?
    else {
        return Ok(None);
    };

    let (_, authority_width) = read_plugin_authority(data, m.header_offset)?;
    let schema_offset = checked_end(m.header_offset, authority_width)?;
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
            data: Span::new(&[]),
        }));
    };
    let end = checked_end(data_offset, data_len)?;
    if data.len() < end {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    Ok(Some(AppDataInfo {
        data_authority,
        schema,
        data: Span::new(&data[data_offset..end]),
    }))
}

/// Reads a `PluginAuthority` at `offset`, shared by both backends. Returns
/// the decoded value and its encoded width (1 byte for `None`/`Owner`/
/// `UpdateAuthority`, 33 for `Address { .. }`).
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
            let end = checked_end(offset, 33)?;
            if data.len() < end {
                return Err(NaclacError::AccountDataTooSmall.err(0));
            }
            let bytes: [u8; 32] = data[offset + 1..end].try_into().unwrap();
            Ok((PluginAuthorityArg::Address(Address::new_from_array(bytes)), 33))
        }
        _ => Err(NaclacError::InvalidInstructionData.err(0)),
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Proves `read_plugin_authority` never panics for any bytes/offset,
    /// including through its now-`checked_end`-guarded `Address` variant
    /// (tag 3).
    #[kani::proof]
    fn prove_read_plugin_authority_never_panics() {
        let data: [u8; 40] = kani::any();
        let offset: usize = kani::any();
        let _ = read_plugin_authority(&data, offset);
    }
}
