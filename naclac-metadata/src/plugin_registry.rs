// ===========================================================================
// plugin_registry.rs — locating a plugin's raw bytes on the pinocchio backend
// ===========================================================================

//! Hand-rolled walk of a Metaplex Core `PluginHeaderV1`/`PluginRegistryV1`/
//! `RegistryRecord` chain, `pinocchio`-only.
//!
//! On the `solana` backend there is nothing to hand-roll here: the real
//! `mpl-core` crate's own `Asset::deserialize`/`Collection::deserialize`
//! already walk this exact chain and hand back a fully hydrated,
//! plugin-decoded struct — Tier 2 code on that backend calls those directly
//! rather than going through this file.
//!
//! On `pinocchio` there's no such helper (see `asset.rs`'s header for why),
//! so finding a given plugin's raw byte offset means walking the chain by
//! hand: `PluginHeaderV1` (fixed 9 bytes: a `Key` tag + a `u64` absolute
//! byte offset of `PluginRegistryV1`) → `PluginRegistryV1` (a `Key` tag then
//! a Borsh `Vec<RegistryRecord>`) → linear-scan the `RegistryRecord`s for
//! the target `PluginType`, since each record is variable-width (`authority:
//! PluginAuthority` is 1 byte for `None`/`Owner`/`UpdateAuthority`, 33 bytes
//! for `Address { .. }`) and there is no index — a `RegistryRecord`'s
//! `offset` field is itself the absolute byte offset (from the account's
//! start) of that plugin's own decoded payload. All three layouts verified
//! against the real `mpl-core` crate's `generated::accounts::{PluginHeaderV1,
//! PluginRegistryV1}`/`generated::types::RegistryRecord`.

#![cfg(feature = "pinocchio")]

use crate::prelude::*;

/// Re-exported here so existing `use crate::plugin_registry::{find_plugin_offset,
/// plugin_type}` call sites throughout `plugins/*.rs` don't need updating —
/// the constants themselves now live in `plugin_type.rs` since they're
/// needed on both backends (see that file's header).
pub use crate::plugin_type;

const KEY_PLUGIN_HEADER_V1: u8 = 3;
const KEY_PLUGIN_REGISTRY_V1: u8 = 4;

/// Walks the `PluginHeaderV1` → `PluginRegistryV1` → `RegistryRecord` chain
/// starting at `plugin_header_offset` (from `AssetView`/`CollectionView`'s
/// own `plugin_header_offset()`), and returns the absolute byte offset of
/// `plugin_type`'s own payload within `data`, if present.
pub fn find_plugin_offset(
    data: &[u8],
    plugin_header_offset: usize,
    plugin_type: u8,
) -> Result<Option<u64>> {
    // PluginHeaderV1: key(1) + plugin_registry_offset: u64(8) = 9 bytes.
    if data.len() < plugin_header_offset + 9 {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    if data[plugin_header_offset] != KEY_PLUGIN_HEADER_V1 {
        return Err(NaclacError::InvalidAccountDiscriminator.err(0));
    }
    let registry_offset = u64::from_le_bytes(
        data[plugin_header_offset + 1..plugin_header_offset + 9]
            .try_into()
            .unwrap(),
    ) as usize;

    // PluginRegistryV1: key(1) + registry: Vec<RegistryRecord> (4-byte
    // little-endian count, then that many variable-width records).
    if data.len() < registry_offset + 5 {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    if data[registry_offset] != KEY_PLUGIN_REGISTRY_V1 {
        return Err(NaclacError::InvalidAccountDiscriminator.err(0));
    }
    let record_count = u32::from_le_bytes(
        data[registry_offset + 1..registry_offset + 5]
            .try_into()
            .unwrap(),
    );

    let mut cursor = registry_offset + 5;
    for _ in 0..record_count {
        // RegistryRecord: plugin_type(1) + authority: PluginAuthority(1, or
        // 33 if the tag is `Address { .. }` = 3) + offset: u64(8).
        if data.len() < cursor + 2 {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        let record_plugin_type = data[cursor];
        let authority_tag = data[cursor + 1];
        let authority_width = if authority_tag == 3 { 33 } else { 1 };

        let offset_field_start = cursor + 1 + authority_width;
        if data.len() < offset_field_start + 8 {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        if record_plugin_type == plugin_type {
            let plugin_offset = u64::from_le_bytes(
                data[offset_field_start..offset_field_start + 8]
                    .try_into()
                    .unwrap(),
            );
            return Ok(Some(plugin_offset));
        }
        cursor = offset_field_start + 8;
    }

    Ok(None)
}
