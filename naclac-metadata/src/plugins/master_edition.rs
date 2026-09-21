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

/// Maximum `name`/`uri` byte length `attach_master_edition_signed` accepts
/// on `pinocchio` — no real protocol maximum exists on this plugin-level
/// override, so this reuses the same cap agreed for the base `Asset`/
/// `Collection` `name`/`uri` fields (`create_asset_signed`/
/// `create_collection_signed`).
#[cfg(feature = "pinocchio")]
pub const MAX_MASTER_EDITION_NAME_LEN: usize = 32;
#[cfg(feature = "pinocchio")]
pub const MAX_MASTER_EDITION_URI_LEN: usize = 200;

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
        let mut data = crate::prelude::Vec::new();
        data.push(3u8); // AddCollectionPluginV1 discriminator
        data.push(plugin_type::MASTER_EDITION);
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
        data.push(0u8); // init_authority: None
        add_collection_plugin_signed(program, accounts, &data, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        if name.is_some_and(|s| s.len() > MAX_MASTER_EDITION_NAME_LEN)
            || uri.is_some_and(|s| s.len() > MAX_MASTER_EDITION_URI_LEN)
        {
            return Err(NaclacError::InvalidInstructionData.err(0));
        }
        let mut data = crate::fixed_buf::FixedBuf::<
            { 3 + 5 + 5 + MAX_MASTER_EDITION_NAME_LEN + 5 + MAX_MASTER_EDITION_URI_LEN },
        >::new();
        data.push(3u8); // AddCollectionPluginV1 discriminator
        data.push(plugin_type::MASTER_EDITION);
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
        data.push(0u8); // init_authority: None
        add_collection_plugin_signed_pinocchio(program, accounts, data.as_slice(), signer_seeds)
    }
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
    let collection_view = crate::collection::CollectionView::from_bytes(&data)?;
    let Some(plugin_header_offset) = collection_view.plugin_header_offset() else {
        return Ok(None);
    };
    match read_master_edition(&data, plugin_header_offset)? {
        Some(view) => Ok(Some(MasterEditionData {
            max_supply: view.max_supply(),
            name: view.name().map(|s| s.into()),
            uri: view.uri().map(|s| s.into()),
        })),
        None => Ok(None),
    }
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
/// (see `asset.rs`'s header). Shared by both backends.
#[derive(Clone, Copy)]
pub struct MasterEditionView<'a> {
    data: &'a [u8],
    max_supply_offset: usize,
    name_offset: usize,
    name_len: usize,
    uri_offset: usize,
    uri_len: usize,
}

impl<'a> MasterEditionView<'a> {
    fn from_bytes(data: &'a [u8], offset: usize) -> Result<Self> {
        let max_supply_offset = offset;
        if data.len() <= max_supply_offset {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        let mut cursor = checked_end(max_supply_offset, 1)?;
        if data[max_supply_offset] == 1 {
            let end = checked_end(cursor, 4)?;
            if data.len() < end {
                return Err(NaclacError::AccountDataTooSmall.err(0));
            }
            cursor = end;
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
fn read_option_string_len(data: &[u8], offset: usize) -> Result<Option<usize>> {
    if data.len() <= offset {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    if data[offset] == 0 {
        return Ok(None);
    }
    let prefix_end = checked_end(offset, 5)?;
    if data.len() < prefix_end {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let len = u32::from_le_bytes(data[offset + 1..prefix_end].try_into().unwrap()) as usize;
    let payload_end = checked_end(prefix_end, len)?;
    if data.len() < payload_end {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    Ok(Some(len))
}

/// Reads a `Collection`'s `MasterEdition` plugin, if attached. `data` is
/// the collection account's raw bytes; `plugin_header_offset` comes from
/// `CollectionView::plugin_header_offset()`. Shared by both backends.
pub fn read_master_edition(
    data: &[u8],
    plugin_header_offset: usize,
) -> Result<Option<MasterEditionView<'_>>> {
    match find_plugin_offset(data, plugin_header_offset, plugin_type::MASTER_EDITION)? {
        Some(offset) => Ok(Some(MasterEditionView::from_bytes(data, offset as usize)?)),
        None => Ok(None),
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Proves `read_master_edition` never panics end-to-end, including
    /// through `find_plugin_offset` and `MasterEditionView::from_bytes`'s
    /// now-`checked_end`-guarded offset arithmetic.
    #[kani::proof]
    #[kani::unwind(10)]
    fn prove_read_master_edition_never_panics() {
        let data: [u8; 32] = kani::any();
        let plugin_header_offset: usize = kani::any();
        let _ = read_master_edition(&data, plugin_header_offset);
    }
}
