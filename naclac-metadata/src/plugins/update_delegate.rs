// ===========================================================================
// plugins/update_delegate.rs — the `UpdateDelegate` plugin
// ===========================================================================

//! `UpdateDelegate` — attaches to an `Asset` or `Collection`, naming extra
//! addresses (beyond the update authority itself) allowed to update it.
//! Real layout verified against `mpl-core`'s
//! `generated::types::UpdateDelegate`: `{ additional_delegates: Vec<Pubkey> }`
//! — a 4-byte little-endian count then that many 32-byte pubkeys, each
//! fixed-width (unlike most other plugins' `Vec`s), so no per-element
//! offset walking is needed here.

use crate::prelude::*;

#[cfg(feature = "pinocchio")]
use crate::plugin_registry::{find_plugin_offset, plugin_type};

/// Attaches `UpdateDelegate` to an `Asset` via `add_plugin`.
pub fn attach_update_delegate_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    additional_delegates: &[Address],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let plugin = ::mpl_core::types::Plugin::UpdateDelegate(::mpl_core::types::UpdateDelegate {
            additional_delegates: additional_delegates.to_vec(),
        });
        add_asset_plugin_signed(program, accounts, plugin, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let payload = encode_additional_delegates(additional_delegates);
        add_asset_plugin_signed_pinocchio(
            program,
            accounts,
            plugin_type::UPDATE_DELEGATE,
            &payload,
            signer_seeds,
        )
    }
}

/// Attaches `UpdateDelegate` to a `Collection` via `add_collection_plugin`.
pub fn attach_collection_update_delegate_signed(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    additional_delegates: &[Address],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let plugin = ::mpl_core::types::Plugin::UpdateDelegate(::mpl_core::types::UpdateDelegate {
            additional_delegates: additional_delegates.to_vec(),
        });
        add_collection_plugin_signed(program, accounts, plugin, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let payload = encode_additional_delegates(additional_delegates);
        add_collection_plugin_signed_pinocchio(
            program,
            accounts,
            plugin_type::UPDATE_DELEGATE,
            &payload,
            signer_seeds,
        )
    }
}

#[cfg(feature = "pinocchio")]
fn encode_additional_delegates(additional_delegates: &[Address]) -> crate::prelude::Vec<u8> {
    let mut data = crate::prelude::Vec::with_capacity(4 + additional_delegates.len() * 32);
    data.extend_from_slice(&(additional_delegates.len() as u32).to_le_bytes());
    for address in additional_delegates {
        data.extend_from_slice(address.as_ref());
    }
    data
}

/// Reads an `Asset`'s `UpdateDelegate` plugin's `additional_delegates`, if
/// attached. Callable identically on both backends.
#[cfg(not(feature = "pinocchio"))]
pub fn fetch_asset_update_delegate(
    info: &AccountInfo,
) -> Result<Option<crate::prelude::Vec<Address>>> {
    if Owner::program_owner(info) != crate::ID {
        return Err(NaclacError::ConstraintOwner.into());
    }
    let solana_info = unsafe { info.to_lifetime() };
    let data = solana_info
        .try_borrow_data()
        .map_err(|_| NaclacError::AccountBorrowFailed.err(0))?;
    let asset = ::mpl_core::Asset::deserialize(&data)
        .map_err(|_| NaclacError::DeserializationFailed.err(0))?;
    Ok(asset
        .plugin_list
        .update_delegate
        .map(|p| p.update_delegate.additional_delegates))
}

/// Reads a `Collection`'s `UpdateDelegate` plugin's `additional_delegates`,
/// if attached. Callable identically on both backends.
#[cfg(not(feature = "pinocchio"))]
pub fn fetch_collection_update_delegate(
    info: &AccountInfo,
) -> Result<Option<crate::prelude::Vec<Address>>> {
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
        .update_delegate
        .map(|p| p.update_delegate.additional_delegates))
}

/// Reads an `Asset`'s `UpdateDelegate` plugin's `additional_delegates`, if
/// attached. Callable identically on both backends.
#[cfg(feature = "pinocchio")]
pub fn fetch_asset_update_delegate(
    info: &AccountInfo,
) -> Result<Option<crate::prelude::Vec<Address>>> {
    let asset_view = crate::asset::AssetView::try_from(info)?;
    let Some(plugin_header_offset) = asset_view.plugin_header_offset() else {
        return Ok(None);
    };
    read_additional_delegates(info.data(), plugin_header_offset)
}

/// Reads a `Collection`'s `UpdateDelegate` plugin's `additional_delegates`,
/// if attached. Callable identically on both backends.
#[cfg(feature = "pinocchio")]
pub fn fetch_collection_update_delegate(
    info: &AccountInfo,
) -> Result<Option<crate::prelude::Vec<Address>>> {
    let collection_view = crate::collection::CollectionView::try_from(info)?;
    let Some(plugin_header_offset) = collection_view.plugin_header_offset() else {
        return Ok(None);
    };
    read_additional_delegates(info.data(), plugin_header_offset)
}

/// Lower-level variant of `fetch_asset_update_delegate`, `pinocchio`-only:
/// see `edition.rs`'s `read_edition` for why this exists alongside the
/// uniform-signature wrapper above.
#[cfg(feature = "pinocchio")]
pub fn read_additional_delegates(
    data: &[u8],
    plugin_header_offset: usize,
) -> Result<Option<crate::prelude::Vec<Address>>> {
    let Some(offset) = find_plugin_offset(data, plugin_header_offset, plugin_type::UPDATE_DELEGATE)?
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
    let mut delegates = crate::prelude::Vec::with_capacity(count);
    for i in 0..count {
        let start = offset + 4 + i * 32;
        let bytes: [u8; 32] = data[start..start + 32].try_into().unwrap();
        delegates.push(Address::new_from_array(bytes));
    }
    Ok(Some(delegates))
}
