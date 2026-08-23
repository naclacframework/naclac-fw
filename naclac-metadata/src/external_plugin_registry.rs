// ===========================================================================
// external_plugin_registry.rs — walking the external plugin adapter registry
// ===========================================================================

//! Hand-rolled walk of the `ExternalRegistryRecord` list on `pinocchio` —
//! the external-adapter analogue of `plugin_registry.rs`'s internal-plugin
//! walker. Real layout verified against `mpl-core`'s
//! `generated::types::ExternalRegistryRecord`:
//! `plugin_type: ExternalPluginAdapterType(1) + authority: PluginAuthority(1
//! or 33) + lifecycle_checks: Option<Vec<(HookableLifecycleEvent, ExternalCheckResult)>>(1,
//! +4+N*5 if Some — each `HookableLifecycleEvent` is a 1-byte tag,
//! `ExternalCheckResult` is a 4-byte `flags: u32`) + offset: u64(8,
//! always present — where the adapter's own small header, e.g.
//! `AppData { data_authority, schema }`, lives) + data_offset: Option<u64>(1,
//! +8 if Some) + data_len: Option<u64>(1, +8 if Some)`.
//!
//! Unlike the internal-plugin registry, `record.authority` here is the
//! adapter's *management* authority (who can approve/revoke/update plugin
//! properties) — for adapter types with their own separate identifying
//! field (e.g. `AppData.data_authority`, immutable once set, verified in
//! its own doc comment: "This field cannot be changed after the plugin is
//! added"), that field is what a real `ExternalPluginAdapterKey` actually
//! carries, not `record.authority`. So locating a *specific* instance
//! among several of the same type needs two steps: filter records by
//! `plugin_type`, then read into `record.offset` to check the adapter's
//! own header for a match. `find_external_registry_offset` does the
//! type-only filter (returning `offset`, the header's own location, plus
//! `data_offset`/`data_len` for the payload past the header); per-type
//! files (`external_plugins/app_data.rs`) do the second check themselves,
//! since only they know their own header's shape.

#![cfg(feature = "pinocchio")]

use crate::prelude::*;

const KEY_PLUGIN_HEADER_V1: u8 = 3;
const KEY_PLUGIN_REGISTRY_V1: u8 = 4;

/// Raw `ExternalPluginAdapterType` discriminants (Borsh unit-enum order,
/// verified against `mpl-core`'s real `generated::types::ExternalPluginAdapterType`).
pub mod external_plugin_type {
    pub const LIFECYCLE_HOOK: u8 = 0;
    pub const ORACLE: u8 = 1;
    pub const APP_DATA: u8 = 2;
    pub const LINKED_LIFECYCLE_HOOK: u8 = 3;
    pub const LINKED_APP_DATA: u8 = 4;
    pub const DATA_SECTION: u8 = 5;
    pub const AGENT_IDENTITY: u8 = 6;
}

/// One matching `ExternalRegistryRecord`'s decoded fields — `header_offset`
/// is where the adapter's own small struct lives; `data_offset`/`data_len`
/// (if present) locate the larger, separately-stored payload bytes past
/// that header (e.g. `AppData`'s actual arbitrary data).
#[derive(Clone, Copy, Debug)]
pub struct ExternalRegistryMatch {
    pub header_offset: usize,
    pub data_offset: Option<usize>,
    pub data_len: Option<usize>,
}

/// Walks the `PluginHeaderV1` → `PluginRegistryV1` → `ExternalRegistryRecord`
/// chain starting at `plugin_header_offset` (from `AssetView`/`CollectionView`'s
/// own `plugin_header_offset()`), collecting every record whose
/// `plugin_type` matches `target_type`. There is no early return on first
/// match (unlike `plugin_registry::find_plugin_offset`) since external
/// adapter types can have multiple simultaneous instances.
pub fn find_external_registry_matches(
    data: &[u8],
    plugin_header_offset: usize,
    target_type: u8,
) -> Result<crate::prelude::Vec<ExternalRegistryMatch>> {
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

    // PluginRegistryV1: key(1) + registry: Vec<RegistryRecord>(4-byte count,
    // N variable-width records) + external_registry: Vec<ExternalRegistryRecord>
    // (4-byte count, N variable-width records) — must walk past the
    // *internal* registry first to reach the external one.
    if data.len() < registry_offset + 5 {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    if data[registry_offset] != KEY_PLUGIN_REGISTRY_V1 {
        return Err(NaclacError::InvalidAccountDiscriminator.err(0));
    }
    let internal_count = u32::from_le_bytes(
        data[registry_offset + 1..registry_offset + 5]
            .try_into()
            .unwrap(),
    );

    let mut cursor = registry_offset + 5;
    for _ in 0..internal_count {
        // RegistryRecord: plugin_type(1) + authority: PluginAuthority(1, or
        // 33 if tag==3) + offset: u64(8).
        if data.len() < cursor + 2 {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        let authority_width = if data[cursor + 1] == 3 { 33 } else { 1 };
        let offset_field_start = cursor + 1 + authority_width;
        if data.len() < offset_field_start + 8 {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        cursor = offset_field_start + 8;
    }

    // Now at the external registry's own 4-byte count.
    if data.len() < cursor + 4 {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let external_count =
        u32::from_le_bytes(data[cursor..cursor + 4].try_into().unwrap());
    cursor += 4;

    let mut matches = crate::prelude::Vec::new();
    for _ in 0..external_count {
        // ExternalRegistryRecord: plugin_type(1) + authority: PluginAuthority
        // (1 or 33) + lifecycle_checks: Option<Vec<(HookableLifecycleEvent,
        // ExternalCheckResult)>>(1, +4+N*5 if Some) + offset: u64(8) +
        // data_offset: Option<u64>(1, +8 if Some) + data_len: Option<u64>
        // (1, +8 if Some).
        if data.len() < cursor + 2 {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        let record_type = data[cursor];
        let authority_width = if data[cursor + 1] == 3 { 33 } else { 1 };
        let mut field_cursor = cursor + 1 + authority_width;

        if data.len() <= field_cursor {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        let lifecycle_checks_tag = data[field_cursor];
        field_cursor += 1;
        if lifecycle_checks_tag == 1 {
            if data.len() < field_cursor + 4 {
                return Err(NaclacError::AccountDataTooSmall.err(0));
            }
            let checks_count =
                u32::from_le_bytes(data[field_cursor..field_cursor + 4].try_into().unwrap())
                    as usize;
            field_cursor += 4 + checks_count * 5; // (1-byte event tag + 4-byte flags) each
        }

        if data.len() < field_cursor + 8 {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        let header_offset =
            u64::from_le_bytes(data[field_cursor..field_cursor + 8].try_into().unwrap()) as usize;
        field_cursor += 8;

        if data.len() <= field_cursor {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        let data_offset = if data[field_cursor] == 1 {
            field_cursor += 1;
            if data.len() < field_cursor + 8 {
                return Err(NaclacError::AccountDataTooSmall.err(0));
            }
            let v =
                u64::from_le_bytes(data[field_cursor..field_cursor + 8].try_into().unwrap());
            field_cursor += 8;
            Some(v as usize)
        } else {
            field_cursor += 1;
            None
        };

        if data.len() <= field_cursor {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        let data_len = if data[field_cursor] == 1 {
            field_cursor += 1;
            if data.len() < field_cursor + 8 {
                return Err(NaclacError::AccountDataTooSmall.err(0));
            }
            let v =
                u64::from_le_bytes(data[field_cursor..field_cursor + 8].try_into().unwrap());
            field_cursor += 8;
            Some(v as usize)
        } else {
            field_cursor += 1;
            None
        };

        if record_type == target_type {
            matches.push(ExternalRegistryMatch {
                header_offset,
                data_offset,
                data_len,
            });
        }

        cursor = field_cursor;
    }

    Ok(matches)
}
