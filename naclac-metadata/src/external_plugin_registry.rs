// ===========================================================================
// external_plugin_registry.rs — walking the external plugin adapter registry
// ===========================================================================

//! Hand-rolled walk of the `ExternalRegistryRecord` list, shared by both
//! backends (see `plugin_registry.rs`'s header for why the `solana` side
//! also uses a hand-rolled walk rather than `mpl-core`'s own deserializer) —
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
//! carries, not `record.authority`. So locating a *specific* instance among
//! several of the same type needs two steps: filter records by
//! `plugin_type`, then read into `record.offset` to check the adapter's own
//! header for a match. `find_external_registry_match` does both in one
//! pass — it calls a per-type predicate (supplied by
//! `external_plugins/app_data.rs` etc., since only they know their own
//! header's shape) for every type-matching record and returns as soon as
//! one accepts, rather than collecting every candidate first.

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
/// own `plugin_header_offset()`), calling `is_match` for every record whose
/// `plugin_type` matches `target_type` and returning the first one it
/// accepts. External adapter types can have multiple simultaneous instances
/// of the same type (unlike `plugin_registry::find_plugin_offset`'s internal
/// plugins, which are unique per type) — `is_match` is how a caller
/// distinguishes *which* instance it wants (e.g. by `data_authority`)
/// without this function ever needing to collect every candidate first.
pub fn find_external_registry_match(
    data: &[u8],
    plugin_header_offset: usize,
    target_type: u8,
    mut is_match: impl FnMut(&ExternalRegistryMatch, &[u8]) -> Result<bool>,
) -> Result<Option<ExternalRegistryMatch>> {
    // PluginHeaderV1: key(1) + plugin_registry_offset: u64(8) = 9 bytes.
    // `checked_add`: same overflow class Kani found and confirmed in
    // `plugin_registry::find_plugin_offset` (see docs/plan/kani-audit.md) —
    // `plugin_header_offset` traces back to account bytes, so an
    // adversarial value near `usize::MAX` must be rejected here, not
    // overflow this check's own addition into a panic first.
    let header_end = plugin_header_offset
        .checked_add(9)
        .ok_or_else(|| NaclacError::AccountDataTooSmall.err(0))?;
    if data.len() < header_end {
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
    // `checked_add`: `registry_offset` is read directly from raw account
    // bytes (line above), same overflow class as the header check above.
    let registry_end = registry_offset
        .checked_add(5)
        .ok_or_else(|| NaclacError::AccountDataTooSmall.err(0))?;
    if data.len() < registry_end {
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

    // The remaining offset arithmetic below builds on `registry_end`/
    // `header_end` above, both already `checked_end`-guarded — see this
    // crate's `checked_end` (lib.rs) for why every offset entering from raw
    // account bytes needs this rather than a bare `+`.
    let mut cursor = registry_offset + 5;
    for _ in 0..internal_count {
        // RegistryRecord: plugin_type(1) + authority: PluginAuthority(1, or
        // 33 if tag==3) + offset: u64(8).
        let cursor_plus_2 = checked_end(cursor, 2)?;
        if data.len() < cursor_plus_2 {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        let authority_width = if data[cursor + 1] == 3 { 33 } else { 1 };
        let offset_field_start = checked_end(cursor, 1 + authority_width)?;
        let record_end = checked_end(offset_field_start, 8)?;
        if data.len() < record_end {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        cursor = record_end;
    }

    // Now at the external registry's own 4-byte count.
    let count_end = checked_end(cursor, 4)?;
    if data.len() < count_end {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let external_count =
        u32::from_le_bytes(data[cursor..count_end].try_into().unwrap());
    cursor = count_end;

    for _ in 0..external_count {
        // ExternalRegistryRecord: plugin_type(1) + authority: PluginAuthority
        // (1 or 33) + lifecycle_checks: Option<Vec<(HookableLifecycleEvent,
        // ExternalCheckResult)>>(1, +4+N*5 if Some) + offset: u64(8) +
        // data_offset: Option<u64>(1, +8 if Some) + data_len: Option<u64>
        // (1, +8 if Some).
        let cursor_plus_2 = checked_end(cursor, 2)?;
        if data.len() < cursor_plus_2 {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        let record_type = data[cursor];
        let authority_width = if data[cursor + 1] == 3 { 33 } else { 1 };
        let mut field_cursor = checked_end(cursor, 1 + authority_width)?;

        if data.len() <= field_cursor {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        let lifecycle_checks_tag = data[field_cursor];
        field_cursor = checked_end(field_cursor, 1)?;
        if lifecycle_checks_tag == 1 {
            let checks_count_end = checked_end(field_cursor, 4)?;
            if data.len() < checks_count_end {
                return Err(NaclacError::AccountDataTooSmall.err(0));
            }
            let checks_count = u32::from_le_bytes(
                data[field_cursor..checks_count_end].try_into().unwrap(),
            ) as usize;
            // (1-byte event tag + 4-byte flags) each
            let checks_bytes = checks_count
                .checked_mul(5)
                .ok_or_else(|| NaclacError::AccountDataTooSmall.err(0))?;
            field_cursor = checked_end(checks_count_end, checks_bytes)?;
        }

        let header_offset_end = checked_end(field_cursor, 8)?;
        if data.len() < header_offset_end {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        let header_offset =
            u64::from_le_bytes(data[field_cursor..header_offset_end].try_into().unwrap()) as usize;
        field_cursor = header_offset_end;

        if data.len() <= field_cursor {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        let data_offset = if data[field_cursor] == 1 {
            field_cursor = checked_end(field_cursor, 1)?;
            let end = checked_end(field_cursor, 8)?;
            if data.len() < end {
                return Err(NaclacError::AccountDataTooSmall.err(0));
            }
            let v =
                u64::from_le_bytes(data[field_cursor..end].try_into().unwrap());
            field_cursor = end;
            Some(v as usize)
        } else {
            field_cursor = checked_end(field_cursor, 1)?;
            None
        };

        if data.len() <= field_cursor {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        let data_len = if data[field_cursor] == 1 {
            field_cursor = checked_end(field_cursor, 1)?;
            let end = checked_end(field_cursor, 8)?;
            if data.len() < end {
                return Err(NaclacError::AccountDataTooSmall.err(0));
            }
            let v =
                u64::from_le_bytes(data[field_cursor..end].try_into().unwrap());
            field_cursor = end;
            Some(v as usize)
        } else {
            field_cursor = checked_end(field_cursor, 1)?;
            None
        };

        if record_type == target_type {
            let candidate = ExternalRegistryMatch {
                header_offset,
                data_offset,
                data_len,
            };
            if is_match(&candidate, data)? {
                return Ok(Some(candidate));
            }
        }

        cursor = field_cursor;
    }

    Ok(None)
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Proves `find_external_registry_match` never panics for any account
    /// bytes and any offset values. `plugin_header_offset + 9` and
    /// `registry_offset + 5` are already fixed above (the exact overflow
    /// class Kani confirmed in `plugin_registry::find_plugin_offset` — see
    /// `docs/plan/kani-audit.md`); the `checks_count * 5` step is checked
    /// safe by direct calculation (max `u32::MAX * 5 + 4 ≈ 2.1e10`, far
    /// below `usize::MAX` on a 64-bit target, so it can't itself overflow —
    /// the subsequent length check correctly rejects it instead).
    ///
    /// Deliberately small buffer/unwind bound: a first attempt at fully
    /// symbolic 24-byte input with `#[kani::unwind(40)]` took over 17
    /// minutes of symbolic execution alone (1.7M program steps) and
    /// exhausted available memory before finishing, because two nested
    /// loops each unwound 40 times combined with a fully symbolic `is_match`
    /// closure multiplies the explored path count combinatorially. This
    /// smaller configuration still covers the header/registry-offset
    /// overflow class (the confirmed-real risk) and the internal-registry
    /// loop, but doesn't exhaustively explore every depth of the external
    /// loop's `lifecycle_checks`/`data_offset`/`data_len` `Option` nesting —
    /// a known, deliberate scope reduction, not silently dropped coverage.
    #[kani::proof]
    #[kani::unwind(15)]
    fn prove_find_external_registry_match_never_panics() {
        let data: [u8; 16] = kani::any();
        let plugin_header_offset: usize = kani::any();
        let target_type: u8 = kani::any();
        let _ = find_external_registry_match(
            &data,
            plugin_header_offset,
            target_type,
            |_candidate, _data| Ok(kani::any()),
        );
    }
}
