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

#[cfg(feature = "pinocchio")]
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
    let asset = ::mpl_core::Asset::deserialize(&data)
        .map_err(|_| NaclacError::DeserializationFailed.err(0))?;
    Ok(asset.plugin_list.groups.map(|p| p.groups.groups))
}

/// Reads an `Asset`'s `Groups` plugin, if attached. Callable identically on
/// both backends.
#[cfg(feature = "pinocchio")]
pub fn fetch_asset_groups(info: &AccountInfo) -> Result<Option<crate::prelude::Vec<Address>>> {
    let asset_view = crate::asset::AssetView::try_from(info)?;
    let Some(plugin_header_offset) = asset_view.plugin_header_offset() else {
        return Ok(None);
    };
    let data = info.data();
    let Some(offset) = find_plugin_offset(data, plugin_header_offset, plugin_type::GROUPS)?
    else {
        return Ok(None);
    };
    let offset = offset as usize;

    if data.len() < offset + 4 {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let count = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
    if data.len() < offset + 4 + count * 32 {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let mut groups = crate::prelude::Vec::with_capacity(count);
    for i in 0..count {
        let start = offset + 4 + i * 32;
        let bytes: [u8; 32] = data[start..start + 32].try_into().unwrap();
        groups.push(Address::new_from_array(bytes));
    }
    Ok(Some(groups))
}
