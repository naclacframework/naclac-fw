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

#[cfg(feature = "pinocchio")]
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
    #[cfg(not(feature = "pinocchio"))]
    {
        let plugin = ::mpl_core::types::Plugin::Attributes(build_attributes(attributes));
        add_asset_plugin_signed(program, accounts, plugin, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let payload = encode_attributes_payload(attributes);
        add_asset_plugin_signed_pinocchio(
            program,
            accounts,
            plugin_type::ATTRIBUTES,
            &payload,
            signer_seeds,
        )
    }
}

/// Attaches `Attributes` to a `Collection` via `add_collection_plugin`.
pub fn attach_collection_attributes_signed(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    attributes: &[(&str, &str)],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let plugin = ::mpl_core::types::Plugin::Attributes(build_attributes(attributes));
        add_collection_plugin_signed(program, accounts, plugin, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let payload = encode_attributes_payload(attributes);
        add_collection_plugin_signed_pinocchio(
            program,
            accounts,
            plugin_type::ATTRIBUTES,
            &payload,
            signer_seeds,
        )
    }
}

#[cfg(not(feature = "pinocchio"))]
fn build_attributes(attributes: &[(&str, &str)]) -> ::mpl_core::types::Attributes {
    ::mpl_core::types::Attributes {
        attribute_list: attributes
            .iter()
            .map(|(k, v)| ::mpl_core::types::Attribute {
                key: k.to_string(),
                value: v.to_string(),
            })
            .collect(),
    }
}

#[cfg(feature = "pinocchio")]
fn encode_attributes_payload(attributes: &[(&str, &str)]) -> crate::prelude::Vec<u8> {
    let mut data = crate::prelude::Vec::new();
    data.extend_from_slice(&(attributes.len() as u32).to_le_bytes());
    for (key, value) in attributes {
        data.extend_from_slice(&(key.len() as u32).to_le_bytes());
        data.extend_from_slice(key.as_bytes());
        data.extend_from_slice(&(value.len() as u32).to_le_bytes());
        data.extend_from_slice(value.as_bytes());
    }
    data
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
    let asset = ::mpl_core::Asset::deserialize(&data)
        .map_err(|_| NaclacError::DeserializationFailed.err(0))?;
    Ok(asset.plugin_list.attributes.map(|p| {
        p.attributes
            .attribute_list
            .into_iter()
            .map(|a| AttributeEntry {
                key: a.key,
                value: a.value,
            })
            .collect()
    }))
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
    let collection = ::mpl_core::Collection::deserialize(&data)
        .map_err(|_| NaclacError::DeserializationFailed.err(0))?;
    Ok(collection.plugin_list.attributes.map(|p| {
        p.attributes
            .attribute_list
            .into_iter()
            .map(|a| AttributeEntry {
                key: a.key,
                value: a.value,
            })
            .collect()
    }))
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

/// Lower-level variant of `fetch_asset_attributes`, `pinocchio`-only: see
/// `edition.rs`'s `read_edition` for why this exists alongside the
/// uniform-signature wrapper above.
#[cfg(feature = "pinocchio")]
pub fn read_attributes(
    data: &[u8],
    plugin_header_offset: usize,
) -> Result<Option<crate::prelude::Vec<AttributeEntry>>> {
    let Some(offset) = find_plugin_offset(data, plugin_header_offset, plugin_type::ATTRIBUTES)?
    else {
        return Ok(None);
    };
    let offset = offset as usize;

    if data.len() < offset + 4 {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let count = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());

    let mut entries = crate::prelude::Vec::with_capacity(count as usize);
    let mut cursor = offset + 4;
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

#[cfg(feature = "pinocchio")]
fn read_borsh_string_len(data: &[u8], offset: usize) -> Result<usize> {
    if data.len() < offset + 4 {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let len = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
    if data.len() < offset + 4 + len {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    Ok(len)
}
