// ===========================================================================
// plugins/master_edition.rs — the `MasterEdition` plugin
// ===========================================================================

//! `MasterEdition` — attaches to a `Collection`, declaring it a
//! print-edition series (`max_supply`, plus an optional display name/uri
//! override shown instead of the collection's own). Real layout verified
//! against `mpl-core`'s `generated::types::MasterEdition`:
//! `{ max_supply: Option<u32>, name: Option<String>, uri: Option<String> }`.
//! See `edition.rs` for the per-`Asset` print-number companion plugin.

use crate::prelude::*;

#[cfg(feature = "pinocchio")]
use crate::plugin_registry::{find_plugin_offset, plugin_type};

/// Backend-neutral `MasterEdition` payload — `fetch_collection_master_edition`
/// returns this on both backends (rather than the real `::mpl_core::types::MasterEdition`
/// solana could return directly) so callers get identical field-access
/// syntax regardless of backend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MasterEditionData {
    pub max_supply: Option<u32>,
    pub name: Option<crate::prelude::String>,
    pub uri: Option<crate::prelude::String>,
}

/// Attaches `MasterEdition` to a `Collection` via `add_collection_plugin`.
pub fn attach_master_edition_signed(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    max_supply: Option<u32>,
    name: Option<&str>,
    uri: Option<&str>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let plugin = ::mpl_core::types::Plugin::MasterEdition(::mpl_core::types::MasterEdition {
            max_supply,
            name: name.map(|s| s.to_string()),
            uri: uri.map(|s| s.to_string()),
        });
        add_collection_plugin_signed(program, accounts, plugin, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let payload = encode_master_edition_payload(max_supply, name, uri);
        add_collection_plugin_signed_pinocchio(
            program,
            accounts,
            plugin_type::MASTER_EDITION,
            &payload,
            signer_seeds,
        )
    }
}

#[cfg(feature = "pinocchio")]
fn encode_master_edition_payload(
    max_supply: Option<u32>,
    name: Option<&str>,
    uri: Option<&str>,
) -> crate::prelude::Vec<u8> {
    let mut data = crate::prelude::Vec::new();
    match max_supply {
        Some(v) => {
            data.push(1u8);
            data.extend_from_slice(&v.to_le_bytes());
        }
        None => data.push(0u8),
    }
    for field in [name, uri] {
        match field {
            Some(s) => {
                data.push(1u8);
                data.extend_from_slice(&(s.len() as u32).to_le_bytes());
                data.extend_from_slice(s.as_bytes());
            }
            None => data.push(0u8),
        }
    }
    data
}

/// Reads a `Collection`'s `MasterEdition` plugin, if attached. Callable
/// identically on both backends.
#[cfg(not(feature = "pinocchio"))]
pub fn fetch_collection_master_edition(info: &AccountInfo) -> Result<Option<MasterEditionData>> {
    if Owner::program_owner(info) != crate::ID {
        return Err(NaclacError::ConstraintOwner.into());
    }
    let solana_info = unsafe { info.to_lifetime() };
    let data = solana_info
        .try_borrow_data()
        .map_err(|_| NaclacError::AccountBorrowFailed.err(0))?;
    let collection = ::mpl_core::Collection::deserialize(&data)
        .map_err(|_| NaclacError::DeserializationFailed.err(0))?;
    Ok(collection
        .plugin_list
        .master_edition
        .map(|p| MasterEditionData {
            max_supply: p.master_edition.max_supply,
            name: p.master_edition.name,
            uri: p.master_edition.uri,
        }))
}

/// Reads a `Collection`'s `MasterEdition` plugin, if attached. Callable
/// identically on both backends.
#[cfg(feature = "pinocchio")]
pub fn fetch_collection_master_edition(info: &AccountInfo) -> Result<Option<MasterEditionData>> {
    let collection_view = crate::collection::CollectionView::try_from(info)?;
    let Some(plugin_header_offset) = collection_view.plugin_header_offset() else {
        return Ok(None);
    };
    match read_master_edition(info.data(), plugin_header_offset)? {
        Some(view) => Ok(Some(MasterEditionData {
            max_supply: view.max_supply(),
            name: view.name().map(|s| s.into()),
            uri: view.uri().map(|s| s.into()),
        })),
        None => Ok(None),
    }
}

/// Zero-copy, sequential-offset view into a `MasterEdition` plugin's raw
/// payload bytes — same walking approach as `AssetView`/`CollectionView`
/// (see `asset.rs`'s header).
#[cfg(feature = "pinocchio")]
#[derive(Clone, Copy)]
pub struct MasterEditionView<'a> {
    data: &'a [u8],
    max_supply_offset: usize,
    name_offset: usize,
    name_len: usize,
    uri_offset: usize,
    uri_len: usize,
}

#[cfg(feature = "pinocchio")]
impl<'a> MasterEditionView<'a> {
    fn from_bytes(data: &'a [u8], offset: usize) -> Result<Self> {
        let max_supply_offset = offset;
        if data.len() <= max_supply_offset {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        let mut cursor = max_supply_offset + 1;
        if data[max_supply_offset] == 1 {
            if data.len() < cursor + 4 {
                return Err(NaclacError::AccountDataTooSmall.err(0));
            }
            cursor += 4;
        }

        let name_offset = cursor;
        let name_len = read_option_string_len(data, name_offset)?;
        cursor = name_offset + 1 + if let Some(len) = name_len { 4 + len } else { 0 };

        let uri_offset = cursor;
        let uri_len = read_option_string_len(data, uri_offset)?;

        Ok(Self {
            data,
            max_supply_offset,
            name_offset,
            name_len: name_len.unwrap_or(0),
            uri_offset,
            uri_len: uri_len.unwrap_or(0),
        })
    }

    pub fn max_supply(&self) -> Option<u32> {
        if self.data[self.max_supply_offset] == 0 {
            return None;
        }
        let start = self.max_supply_offset + 1;
        Some(u32::from_le_bytes(
            self.data[start..start + 4].try_into().unwrap(),
        ))
    }

    pub fn name(&self) -> Option<&str> {
        if self.data[self.name_offset] == 0 {
            return None;
        }
        let start = self.name_offset + 1 + 4;
        // SAFETY: bounds validated in `from_bytes`; Core's own write path
        // always encodes `name` as a valid Borsh (UTF-8) `String`.
        Some(unsafe {
            core::str::from_utf8_unchecked(&self.data[start..start + self.name_len])
        })
    }

    pub fn uri(&self) -> Option<&str> {
        if self.data[self.uri_offset] == 0 {
            return None;
        }
        let start = self.uri_offset + 1 + 4;
        // SAFETY: same as `name`.
        Some(unsafe {
            core::str::from_utf8_unchecked(&self.data[start..start + self.uri_len])
        })
    }
}

/// `Some(len)` if the `Option<String>` tag at `offset` is `Some` (and the
/// full length-prefixed string fits within `data`), `None` if the tag is
/// `None`. Shared shape for `MasterEditionView`'s `name`/`uri` fields.
#[cfg(feature = "pinocchio")]
fn read_option_string_len(data: &[u8], offset: usize) -> Result<Option<usize>> {
    if data.len() <= offset {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    if data[offset] == 0 {
        return Ok(None);
    }
    if data.len() < offset + 5 {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let len = u32::from_le_bytes(data[offset + 1..offset + 5].try_into().unwrap()) as usize;
    if data.len() < offset + 5 + len {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    Ok(Some(len))
}

/// Reads a `Collection`'s `MasterEdition` plugin, if attached. `data` is
/// the collection account's raw bytes; `plugin_header_offset` comes from
/// `CollectionView::plugin_header_offset()`.
#[cfg(feature = "pinocchio")]
pub fn read_master_edition(
    data: &[u8],
    plugin_header_offset: usize,
) -> Result<Option<MasterEditionView<'_>>> {
    match find_plugin_offset(data, plugin_header_offset, plugin_type::MASTER_EDITION)? {
        Some(offset) => Ok(Some(MasterEditionView::from_bytes(data, offset as usize)?)),
        None => Ok(None),
    }
}
