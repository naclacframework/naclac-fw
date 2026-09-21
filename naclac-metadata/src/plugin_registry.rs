// ===========================================================================
// plugin_registry.rs — locating a plugin's raw bytes, both backends
// ===========================================================================

//! Hand-rolled walk of a Metaplex Core `PluginHeaderV1`/`PluginRegistryV1`/
//! `RegistryRecord` chain, shared by both backends.
//!
//! The real `mpl-core` crate's own `Asset::deserialize`/`Collection::deserialize`
//! also walk this exact chain, but decode **all** plugin types into one
//! `PluginsList` struct in a single function — a stack frame that exceeds
//! Solana's 4096-byte per-function BPF limit when compiled for the SBF
//! target (real, verified `cargo build-sbf` linker errors; that code was
//! written for `mpl-core`'s off-chain client package, not for being
//! compiled into another program's own on-chain binary). Both backends walk
//! the chain by hand instead, decoding only the one requested plugin's own
//! payload: `PluginHeaderV1` (fixed 9 bytes: a `Key` tag + a `u64` absolute
//! byte offset of `PluginRegistryV1`) → `PluginRegistryV1` (a `Key` tag then
//! a Borsh `Vec<RegistryRecord>`) → linear-scan the `RegistryRecord`s for
//! the target `PluginType`, since each record is variable-width (`authority:
//! PluginAuthority` is 1 byte for `None`/`Owner`/`UpdateAuthority`, 33 bytes
//! for `Address { .. }`) and there is no index — a `RegistryRecord`'s
//! `offset` field is itself the absolute byte offset (from the account's
//! start) of that plugin's own decoded payload. All three layouts verified
//! against the real `mpl-core` crate's `generated::accounts::{PluginHeaderV1,
//! PluginRegistryV1}`/`generated::types::RegistryRecord`.

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
/// `plugin_type`'s own *fields* within `data`, if present — i.e. one byte
/// past `RegistryRecord.offset` itself, which points to the start of the
/// real `Plugin` enum's Borsh encoding (a 1-byte variant discriminant, then
/// the fields), not directly to the fields. Confirmed byte-for-byte against
/// a real on-chain account: a `FreezeDelegate` plugin's stored bytes were
/// `[1, 0]` at `RegistryRecord.offset` — `1` is `Plugin::FreezeDelegate`'s
/// own tag (numerically identical to `PluginType::FREEZE_DELEGATE`, which
/// is what made this invisible), `0` is the real `frozen` field one byte
/// later. Every caller in `plugins/*.rs` wants the fields' start, never the
/// tag byte (already known separately, via `plugin_type` itself).
pub fn find_plugin_offset(
    data: &[u8],
    plugin_header_offset: usize,
    plugin_type: u8,
) -> Result<Option<u64>> {
    // PluginHeaderV1: key(1) + plugin_registry_offset: u64(8) = 9 bytes.
    // `checked_add`, not a bare `+`: `plugin_header_offset` traces back to
    // account bytes, so an adversarial value near `usize::MAX` must be
    // rejected by this length check, not overflow the check's own addition
    // into a panic first (confirmed reachable via Kani proof
    // `prove_find_plugin_offset_never_panics` before this fix).
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

    // PluginRegistryV1: key(1) + registry: Vec<RegistryRecord> (4-byte
    // little-endian count, then that many variable-width records).
    // `checked_add`: `registry_offset` is read directly from raw account
    // bytes (line above), so it's fully attacker-controlled — same overflow
    // risk and same fix as the header-offset check above.
    let registry_end = registry_offset
        .checked_add(5)
        .ok_or_else(|| NaclacError::AccountDataTooSmall.err(0))?;
    if data.len() < registry_end {
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
            // Skip the `Plugin` enum's own 1-byte variant tag — see this
            // function's doc comment. `checked_add`: `plugin_offset` is
            // also read directly from raw account bytes, so a crafted
            // `u64::MAX` value here must not overflow past a clean error.
            let fields_offset = plugin_offset
                .checked_add(1)
                .ok_or_else(|| NaclacError::AccountDataTooSmall.err(0))?;
            return Ok(Some(fields_offset));
        }
        cursor = offset_field_start + 8;
    }

    Ok(None)
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Proves `find_plugin_offset` never panics for any account bytes,
    /// including adversarially large offset fields — `registry_offset` (and
    /// every subsequent `cursor`/`offset_field_start`) is read straight from
    /// raw, unvalidated account bytes with no upper bound, so this is
    /// exactly the input shape a malicious/corrupted account can produce.
    /// The guards in the function (`data.len() < offset + N`) are meant to
    /// reject an out-of-range offset gracefully — this proves whether the
    /// addition inside each guard can itself overflow `usize` before the
    /// comparison ever runs, which would panic instead of cleanly erroring.
    #[kani::proof]
    #[kani::unwind(10)]
    fn prove_find_plugin_offset_never_panics() {
        let data: [u8; 32] = kani::any();
        let plugin_header_offset: usize = kani::any();
        let plugin_type: u8 = kani::any();
        let _ = find_plugin_offset(&data, plugin_header_offset, plugin_type);
    }
}
