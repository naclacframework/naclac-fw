// ===========================================================================
// plugins/attributes.rs — the `Attributes` plugin
// ===========================================================================

//! `Attributes` — attaches to an `Asset` or `Collection`, holding a list of
//! arbitrary key/value string pairs (traits, metadata, ...). Real layout
//! verified against `mpl-core`'s `generated::types::{Attributes, Attribute}`:
//! `Attributes { attribute_list: Vec<Attribute> }`,
//! `Attribute { key: String, value: String }` — genuinely variable-width
//! per element (unlike `Royalties`' `Vec<Creator>`), so reading requires a
//! linear scan, one entry at a time, same shape as `plugin_registry.rs`'s
//! `RegistryRecord` walk.

use crate::prelude::*;

use crate::plugin_registry::{find_plugin_offset, plugin_type};

/// One `key`/`value` entry — backend-neutral, owned (borrowed from the
/// account's own byte buffer on `pinocchio`, allocated by the real crate's
/// own `Attribute` on `solana`; owning `String`s here keeps
/// `fetch_asset_attributes`'s return type identical either way).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttributeEntry {
    pub key: crate::prelude::String,
    pub value: crate::prelude::String,
}

/// Attaches `Attributes` to an `Asset` via `add_plugin`.
pub fn attach_attributes_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    attributes: &[(&str, &str)],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let mut data = crate::prelude::Vec::new();
    data.push(2u8); // AddPluginV1 discriminator
    data.push(plugin_type::ATTRIBUTES);
    encode_attributes_payload(&mut data, attributes);
    data.push(0u8); // init_authority: None

    #[cfg(not(feature = "pinocchio"))]
    {
        add_asset_plugin_signed(program, accounts, &data, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        add_asset_plugin_signed_pinocchio(program, accounts, &data, signer_seeds)
    }
}

/// Attaches `Attributes` to a `Collection` via `add_collection_plugin`.
pub fn attach_collection_attributes_signed(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    attributes: &[(&str, &str)],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let mut data = crate::prelude::Vec::new();
    data.push(3u8); // AddCollectionPluginV1 discriminator
    data.push(plugin_type::ATTRIBUTES);
    encode_attributes_payload(&mut data, attributes);
    data.push(0u8); // init_authority: None

    #[cfg(not(feature = "pinocchio"))]
    {
        add_collection_plugin_signed(program, accounts, &data, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        add_collection_plugin_signed_pinocchio(program, accounts, &data, signer_seeds)
    }
}

fn encode_attributes_payload(data: &mut crate::prelude::Vec<u8>, attributes: &[(&str, &str)]) {
    data.extend_from_slice(&(attributes.len() as u32).to_le_bytes());
    for (key, value) in attributes {
        data.extend_from_slice(&(key.len() as u32).to_le_bytes());
        data.extend_from_slice(key.as_bytes());
        data.extend_from_slice(&(value.len() as u32).to_le_bytes());
        data.extend_from_slice(value.as_bytes());
    }
}

/// Reads an `Asset`'s `Attributes` plugin, if attached. Callable identically
/// on both backends.
#[cfg(not(feature = "pinocchio"))]
pub fn fetch_asset_attributes(
    info: &AccountInfo,
) -> Result<Option<crate::prelude::Vec<AttributeEntry>>> {
    if Owner::program_owner(info) != crate::ID {
        return Err(NaclacError::ConstraintOwner.into());
    }
    let solana_info = unsafe { info.to_lifetime() };
    let data = solana_info
        .try_borrow_data()
        .map_err(|_| NaclacError::AccountBorrowFailed.err(0))?;
    let asset_view = crate::asset::AssetView::from_bytes(&data)?;
    let Some(plugin_header_offset) = asset_view.plugin_header_offset() else {
        return Ok(None);
    };
    read_attributes(&data, plugin_header_offset)
}

/// Reads an `Asset`'s `Attributes` plugin, if attached. Callable identically
/// on both backends.
#[cfg(feature = "pinocchio")]
pub fn fetch_asset_attributes(
    info: &AccountInfo,
) -> Result<Option<crate::prelude::Vec<AttributeEntry>>> {
    let asset_view = crate::asset::AssetView::try_from(info)?;
    let Some(plugin_header_offset) = asset_view.plugin_header_offset() else {
        return Ok(None);
    };
    read_attributes(info.data(), plugin_header_offset)
}

/// Reads a `Collection`'s `Attributes` plugin, if attached. Callable
/// identically on both backends.
#[cfg(not(feature = "pinocchio"))]
pub fn fetch_collection_attributes(
    info: &AccountInfo,
) -> Result<Option<crate::prelude::Vec<AttributeEntry>>> {
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
    read_attributes(&data, plugin_header_offset)
}

/// Reads a `Collection`'s `Attributes` plugin, if attached. Callable
/// identically on both backends.
#[cfg(feature = "pinocchio")]
pub fn fetch_collection_attributes(
    info: &AccountInfo,
) -> Result<Option<crate::prelude::Vec<AttributeEntry>>> {
    let collection_view = crate::collection::CollectionView::try_from(info)?;
    let Some(plugin_header_offset) = collection_view.plugin_header_offset() else {
        return Ok(None);
    };
    read_attributes(info.data(), plugin_header_offset)
}

/// Lower-level variant of `fetch_asset_attributes`, shared by both backends:
/// see `edition.rs`'s `read_edition` for why this exists alongside the
/// uniform-signature wrapper above.
pub fn read_attributes(
    data: &[u8],
    plugin_header_offset: usize,
) -> Result<Option<crate::prelude::Vec<AttributeEntry>>> {
    let Some(offset) = find_plugin_offset(data, plugin_header_offset, plugin_type::ATTRIBUTES)?
    else {
        return Ok(None);
    };
    let offset = offset as usize;

    let count_end = checked_end(offset, 4)?;
    if data.len() < count_end {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let count = u32::from_le_bytes(data[offset..count_end].try_into().unwrap());

    let mut entries = crate::prelude::Vec::with_capacity(count as usize);
    let mut cursor = count_end;
    for _ in 0..count {
        let key_len = read_borsh_string_len(data, cursor)?;
        let key_start = cursor + 4;
        // SAFETY: bounds validated by `read_borsh_string_len`; Core's own
        // write path always encodes these as valid Borsh (UTF-8) `String`s.
        let key = unsafe {
            core::str::from_utf8_unchecked(&data[key_start..key_start + key_len])
        };
        cursor = key_start + key_len;

        let value_len = read_borsh_string_len(data, cursor)?;
        let value_start = cursor + 4;
        let value = unsafe {
            core::str::from_utf8_unchecked(&data[value_start..value_start + value_len])
        };
        cursor = value_start + value_len;

        entries.push(AttributeEntry {
            key: key.into(),
            value: value.into(),
        });
    }

    Ok(Some(entries))
}

fn read_borsh_string_len(data: &[u8], offset: usize) -> Result<usize> {
    let prefix_end = checked_end(offset, 4)?;
    if data.len() < prefix_end {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let len = u32::from_le_bytes(data[offset..prefix_end].try_into().unwrap()) as usize;
    let payload_end = checked_end(prefix_end, len)?;
    if data.len() < payload_end {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    Ok(len)
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Proves `read_attributes` never panics end-to-end, including through
    /// `find_plugin_offset` and the now-`checked_end`-guarded entry point —
    /// same class of overflow this crate's audit found repeated across ~15
    /// files (see docs/plan/kani-audit.md).
    #[kani::proof]
    #[kani::unwind(10)]
    fn prove_read_attributes_never_panics() {
        let data: [u8; 32] = kani::any();
        let plugin_header_offset: usize = kani::any();
        let _ = read_attributes(&data, plugin_header_offset);
    }
}
