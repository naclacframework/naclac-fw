// ===========================================================================
// external_plugins/linked_app_data.rs — the `LinkedAppData` external plugin adapter
// ===========================================================================

//! `LinkedAppData` — like `AppData`, but **collection-only**: verified in
//! the real processor, `add_external_plugin_adapter` (asset-level) rejects
//! `LinkedAppData`/`LinkedLifecycleHook` outright
//! (`InvalidPluginAdapterTarget`), while `add_collection_external_plugin_adapter`
//! allows them — the opposite asymmetry from every other plugin in this
//! crate, which all attach to both levels.
//!
//! `LinkedAppData`'s own registry record never carries a `data_offset`/
//! `data_len` (`initialize_external_plugin_adapter` always creates it with
//! both `None`) — the record only ever holds config (`data_authority`,
//! `schema`). The actual data bytes live in a separate, auto-created
//! `DataSection` keyed by `LinkedDataKey::LinkedAppData(data_authority)`:
//! verified in `write_external_plugin_adapter_data.rs`'s shared
//! `process_write_external_plugin_data`, the real processor creates that
//! `DataSection` itself on the first `Write` if it doesn't already exist,
//! pulling `data_authority`/`schema` from the already-attached
//! `LinkedAppData` record. So reading is two steps: this record for
//! `schema`/existence, `data_section::fetch_collection_data_section` (same
//! function `DataSection` itself uses) for the actual bytes. `Write` needs
//! no `LinkedAppData`-specific CPI wiring at all — the already-built
//! `write_collection_external_adapter_signed` already funnels through the
//! same real processor path.

use crate::prelude::*;

use crate::external_plugin_registry::external_plugin_type;

/// Max encoded width of `ExternalPluginAdapterKeyArg::LinkedAppData`/
/// `ExternalPluginAdapterInitInfo::LinkedAppData`'s tag (1 byte) + a
/// `PluginAuthorityArg`/`PluginAuthority` — same shape as
/// `app_data::APP_DATA_KEY_ENCODED_LEN`.
#[cfg(feature = "pinocchio")]
const LINKED_APP_DATA_KEY_ENCODED_LEN: usize = 1 + MAX_PLUGIN_AUTHORITY_ENCODED_LEN;

/// Max total instruction data for `attach_collection_linked_app_data_signed`
/// — same shape/size as `app_data::ATTACH_APP_DATA_IX_LEN`.
#[cfg(feature = "pinocchio")]
const ATTACH_LINKED_APP_DATA_IX_LEN: usize = 1 + LINKED_APP_DATA_KEY_ENCODED_LEN + 1 + 2;

/// Max total instruction data for `update_collection_linked_app_data_schema_signed`.
#[cfg(feature = "pinocchio")]
const UPDATE_LINKED_APP_DATA_IX_LEN: usize = 1 + LINKED_APP_DATA_KEY_ENCODED_LEN + 3;

/// A fully-read `LinkedAppData` instance — header fields plus the actual
/// data bytes, fetched from its linked `DataSection`. `data` is empty if
/// the adapter is attached but nothing has been written to it yet (no
/// `DataSection` exists). On `solana`, `data` is a heap-copied `Vec<u8>`.
/// On `pinocchio`, `data` is a zero-copy `Span<u8>` view into the
/// account's own live bytes — see `AppDataInfo`'s doc comment for why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedAppDataInfo {
    pub data_authority: PluginAuthorityArg,
    pub schema: ExternalPluginAdapterSchemaArg,
    #[cfg(not(feature = "pinocchio"))]
    pub data: crate::prelude::Vec<u8>,
    #[cfg(feature = "pinocchio")]
    pub data: Span<u8>,
}

/// Attaches `LinkedAppData` to a `Collection` via
/// `AddCollectionExternalPluginAdapterV1`. **Collection-only** — see this
/// file's header.
pub fn attach_collection_linked_app_data_signed(
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
        data.push(4u8); // ExternalPluginAdapterInitInfo::LinkedAppData tag
        encode_plugin_authority_owned(&mut data, data_authority);
        data.push(0u8); // init_plugin_authority: None
        data.push(1u8); // schema: Some(...)
        data.push(schema_tag(schema));
        add_collection_external_adapter_signed(program, accounts, &data, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mut data = crate::fixed_buf::FixedBuf::<ATTACH_LINKED_APP_DATA_IX_LEN>::new();
        data.push(23u8); // AddCollectionExternalPluginAdapterV1 discriminator
        data.push(4u8); // ExternalPluginAdapterInitInfo::LinkedAppData tag
        encode_plugin_authority(&mut data, data_authority);
        data.push(0u8); // init_plugin_authority: None
        data.push(1u8); // schema: Some(...)
        data.push(schema_tag(schema));
        add_collection_external_adapter_signed(program, accounts, data.as_slice(), signer_seeds)
    }
}

/// Updates a `LinkedAppData` adapter's `schema` on a `Collection` via
/// `UpdateCollectionExternalPluginAdapterV1`. `data_authority` identifies
/// which instance.
pub fn update_collection_linked_app_data_schema_signed(
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
        data.push(4u8); // ExternalPluginAdapterKeyArg::LinkedAppData tag
        encode_plugin_authority_owned(&mut data, data_authority);
        data.push(4u8); // ExternalPluginAdapterUpdateInfo::LinkedAppData tag
        data.push(1u8); // schema: Some(...)
        data.push(schema_tag(new_schema));
        update_collection_external_adapter_signed(program, accounts, &data, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mut data = crate::fixed_buf::FixedBuf::<UPDATE_LINKED_APP_DATA_IX_LEN>::new();
        data.push(27u8); // UpdateCollectionExternalPluginAdapterV1 discriminator
        data.push(4u8); // ExternalPluginAdapterKeyArg::LinkedAppData tag
        encode_plugin_authority(&mut data, data_authority);
        data.push(4u8); // ExternalPluginAdapterUpdateInfo::LinkedAppData tag
        data.push(1u8); // schema: Some(...)
        data.push(schema_tag(new_schema));
        update_collection_external_adapter_signed(program, accounts, data.as_slice(), signer_seeds)
    }
}

/// Writes `bytes` to a `Collection`'s `LinkedAppData` adapter identified by
/// `data_authority`, via `WriteCollectionExternalPluginAdapterDataV1` — the
/// real processor creates the backing `DataSection` automatically on first
/// write (see this file's header), no adapter-specific CPI wiring needed
/// here.
pub fn write_collection_linked_app_data_signed(
    program: CpiHandle<'_>,
    accounts: WriteCollectionExternalAdapterAccounts<'_>,
    data_authority: PluginAuthorityArg,
    bytes: &[u8],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    write_collection_external_adapter_signed(
        program,
        accounts,
        ExternalPluginAdapterKeyArg::LinkedAppData(data_authority),
        WriteData::Inline(bytes),
        signer_seeds,
    )
}

/// Removes a `Collection`'s `LinkedAppData` adapter identified by
/// `data_authority`, via `RemoveCollectionExternalPluginAdapterV1`.
pub fn remove_collection_linked_app_data_signed(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    data_authority: PluginAuthorityArg,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    remove_collection_external_adapter_signed(
        program,
        accounts,
        ExternalPluginAdapterKeyArg::LinkedAppData(data_authority),
        signer_seeds,
    )
}

/// Reads a `Collection`'s `LinkedAppData` adapter identified by
/// `data_authority`, if attached. `data` is empty if attached but nothing's
/// been written yet. Callable identically on both backends.
#[cfg(not(feature = "pinocchio"))]
pub fn fetch_collection_linked_app_data(
    info: &AccountInfo,
    data_authority: PluginAuthorityArg,
) -> Result<Option<LinkedAppDataInfo>> {
    if Owner::program_owner(info) != crate::ID {
        return Err(NaclacError::ConstraintOwner.into());
    }
    let schema = {
        let solana_info = unsafe { info.to_lifetime() };
        let raw = solana_info
            .try_borrow_data()
            .map_err(|_| NaclacError::AccountBorrowFailed.err(0))?;
        let collection_view = crate::collection::CollectionView::from_bytes(&raw)?;
        let Some(plugin_header_offset) = collection_view.plugin_header_offset() else {
            return Ok(None);
        };
        let Some(m) = crate::external_plugin_registry::find_external_registry_match(
            &raw,
            plugin_header_offset,
            external_plugin_type::LINKED_APP_DATA,
            |candidate, data| {
                let (candidate_authority, _) =
                    crate::external_plugins::app_data::read_plugin_authority(
                        data,
                        candidate.header_offset,
                    )?;
                Ok(candidate_authority == data_authority)
            },
        )?
        else {
            return Ok(None);
        };
        let (_, authority_width) = crate::external_plugins::app_data::read_plugin_authority(
            &raw,
            m.header_offset,
        )?;
        let schema_offset = m.header_offset + authority_width;
        if raw.len() <= schema_offset {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        match raw[schema_offset] {
            0 => ExternalPluginAdapterSchemaArg::Binary,
            1 => ExternalPluginAdapterSchemaArg::Json,
            2 => ExternalPluginAdapterSchemaArg::MsgPack,
            _ => return Err(NaclacError::InvalidInstructionData.err(0)),
        }
    };

    let parent_key = LinkedDataKeyArg::LinkedAppData(data_authority);
    let data = match crate::external_plugins::data_section::fetch_collection_data_section(
        info, parent_key,
    )? {
        Some(section) => section.data,
        None => crate::prelude::Vec::new(),
    };
    Ok(Some(LinkedAppDataInfo {
        data_authority,
        schema,
        data,
    }))
}

/// Reads a `Collection`'s `LinkedAppData` adapter identified by
/// `data_authority`, if attached. `data` is empty if attached but nothing's
/// been written yet. Callable identically on both backends.
#[cfg(feature = "pinocchio")]
pub fn fetch_collection_linked_app_data(
    info: &AccountInfo,
    data_authority: PluginAuthorityArg,
) -> Result<Option<LinkedAppDataInfo>> {
    let collection_view = crate::collection::CollectionView::try_from(info)?;
    let Some(plugin_header_offset) = collection_view.plugin_header_offset() else {
        return Ok(None);
    };
    let data = info.data();
    let Some(m) = crate::external_plugin_registry::find_external_registry_match(
        data,
        plugin_header_offset,
        external_plugin_type::LINKED_APP_DATA,
        |candidate, data| {
            let (candidate_authority, _) =
                crate::external_plugins::app_data::read_plugin_authority(
                    data,
                    candidate.header_offset,
                )?;
            Ok(candidate_authority == data_authority)
        },
    )?
    else {
        return Ok(None);
    };

    let (_, authority_width) = crate::external_plugins::app_data::read_plugin_authority(
        data,
        m.header_offset,
    )?;
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

    let parent_key = LinkedDataKeyArg::LinkedAppData(data_authority);
    let section_data = match crate::external_plugins::data_section::fetch_collection_data_section(
        info, parent_key,
    )? {
        Some(section) => section.data,
        None => Span::new(&[]),
    };
    Ok(Some(LinkedAppDataInfo {
        data_authority,
        schema,
        data: section_data,
    }))
}
