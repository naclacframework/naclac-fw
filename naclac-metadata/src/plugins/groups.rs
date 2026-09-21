// ===========================================================================
// plugins/groups.rs — the `Groups` plugin (read-only)
// ===========================================================================

//! `Groups` — an `Asset`/`Collection`'s list of `GroupV1` account
//! memberships. Real layout verified against `mpl-core`'s
//! `generated::types::Groups`: `{ groups: Vec<Pubkey> }` — a 4-byte
//! little-endian count then that many fixed-width 32-byte pubkeys, same
//! shape as `UpdateDelegate.additional_delegates`.
//!
//! **Read-only, deliberately.** Unlike every other plugin in this crate,
//! there is no `attach_groups_signed` here: verified directly against the
//! real processor source (`processor/add_plugin.rs`), both `add_plugin`
//! (asset) and `add_collection_plugin` (collection) unconditionally reject
//! `PluginType::Groups` —
//! `"Groups plugins must be managed exclusively by the dedicated Group
//! instructions (Add/Remove Collections To/From Group, etc.)"` — so this
//! plugin's data is only ever written by the program itself as a side
//! effect of the dedicated Group instructions (`CreateGroupV1`,
//! `AddAssetsToGroupV1`, `AddCollectionsToGroupV1`, ...), not by a direct
//! `add_plugin`/`add_collection_plugin` call. Those instructions aren't
//! implemented yet (see `docs/02-instruction-coverage-checklist.md`,
//! "Groups as accounts") — writing to this plugin will be exposed through
//! them once they exist, not through this file.

use crate::prelude::*;

use crate::plugin_registry::{find_plugin_offset, plugin_type};

/// Reads an `Asset`'s `Groups` plugin, if attached. Callable identically on
/// both backends.
#[cfg(not(feature = "pinocchio"))]
pub fn fetch_asset_groups(info: &AccountInfo) -> Result<Option<crate::prelude::Vec<Address>>> {
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
    Ok(read_groups(&data, plugin_header_offset)?.map(|span| span.iter().collect()))
}

/// Reads an `Asset`'s `Groups` plugin, if attached. Callable identically on
/// both backends. Zero-copy on `pinocchio` — `Address` is `bytemuck::Pod`,
/// so `Span<Address>` views directly into the account's own bytes, no cap
/// needed.
#[cfg(feature = "pinocchio")]
pub fn fetch_asset_groups(info: &AccountInfo) -> Result<Option<Span<Address>>> {
    let asset_view = crate::asset::AssetView::try_from(info)?;
    let Some(plugin_header_offset) = asset_view.plugin_header_offset() else {
        return Ok(None);
    };
    read_groups(info.data(), plugin_header_offset)
}

/// Lower-level variant of `fetch_asset_groups`, shared by both backends: see
/// `edition.rs`'s `read_edition` for why this exists alongside the
/// uniform-signature wrapper above.
pub fn read_groups(data: &[u8], plugin_header_offset: usize) -> Result<Option<Span<Address>>> {
    let Some(offset) = find_plugin_offset(data, plugin_header_offset, plugin_type::GROUPS)?
    else {
        return Ok(None);
    };
    let offset = offset as usize;

    let prefix_end = offset
        .checked_add(4)
        .ok_or_else(|| NaclacError::AccountDataTooSmall.err(0))?;
    if data.len() < prefix_end {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let count = u32::from_le_bytes(data[offset..prefix_end].try_into().unwrap()) as usize;
    let end = count
        .checked_mul(32)
        .and_then(|bytes| prefix_end.checked_add(bytes))
        .ok_or_else(|| NaclacError::AccountDataTooSmall.err(0))?;
    if data.len() < end {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    Ok(Some(Span::from_bytes(&data[prefix_end..end])?))
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Proves `read_groups` never panics end-to-end, including through
    /// `find_plugin_offset` — whose overflow fix (checked_add on its own
    /// internal additions) only guarantees the *addition itself* doesn't
    /// overflow, not that the *returned offset* is small. A confirmed-huge
    /// (but non-overflowing-at-computation-time) return value could still
    /// overflow this function's own unguarded `offset + 4` one level down —
    /// this proves whether that follow-on risk is real or not, rather than
    /// assuming the upstream fix was sufficient everywhere it's used.
    #[kani::proof]
    #[kani::unwind(10)]
    fn prove_read_groups_never_panics() {
        let data: [u8; 40] = kani::any();
        let plugin_header_offset: usize = kani::any();
        let _ = read_groups(&data, plugin_header_offset);
    }
}
