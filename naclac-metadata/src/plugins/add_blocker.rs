// ===========================================================================
// plugins/add_blocker.rs — the `AddBlocker` plugin
// ===========================================================================

//! `AddBlocker` — prevents anyone but the update authority from attaching
//! further plugins. Real layout verified against `mpl-core`'s
//! `generated::types::AddBlocker`: an empty struct (Borsh encodes it as
//! zero bytes). Provided for both `Asset` and `Collection` since nothing in
//! the real crate's generated code pins it to one level.

use crate::prelude::*;

use crate::plugin_registry::{find_plugin_offset, plugin_type};

/// Attaches `AddBlocker` to an `Asset` via `add_plugin`.
pub fn attach_add_blocker_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let data = [2u8, plugin_type::ADD_BLOCKER, 0u8];
        add_asset_plugin_signed(program, accounts, &data, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mut data = crate::fixed_buf::FixedBuf::<3>::new();
        data.push(2u8); // AddPluginV1 discriminator
        data.push(plugin_type::ADD_BLOCKER);
        data.push(0u8); // init_authority: None
        add_asset_plugin_signed_pinocchio(program, accounts, data.as_slice(), signer_seeds)
    }
}

/// Attaches `AddBlocker` to a `Collection` via `add_collection_plugin`.
pub fn attach_collection_add_blocker_signed(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let data = [3u8, plugin_type::ADD_BLOCKER, 0u8];
        add_collection_plugin_signed(program, accounts, &data, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mut data = crate::fixed_buf::FixedBuf::<3>::new();
        data.push(3u8); // AddCollectionPluginV1 discriminator
        data.push(plugin_type::ADD_BLOCKER);
        data.push(0u8); // init_authority: None
        add_collection_plugin_signed_pinocchio(program, accounts, data.as_slice(), signer_seeds)
    }
}

/// `true` if an `Asset` has an `AddBlocker` plugin attached. Callable
/// identically on both backends.
#[cfg(not(feature = "pinocchio"))]
pub fn asset_has_add_blocker(info: &AccountInfo) -> Result<bool> {
    if Owner::program_owner(info) != crate::ID {
        return Err(NaclacError::ConstraintOwner.into());
    }
    let solana_info = unsafe { info.to_lifetime() };
    let data = solana_info
        .try_borrow_data()
        .map_err(|_| NaclacError::AccountBorrowFailed.err(0))?;
    let asset_view = crate::asset::AssetView::from_bytes(&data)?;
    let Some(plugin_header_offset) = asset_view.plugin_header_offset() else {
        return Ok(false);
    };
    Ok(find_plugin_offset(&data, plugin_header_offset, plugin_type::ADD_BLOCKER)?.is_some())
}

/// `true` if a `Collection` has an `AddBlocker` plugin attached. Callable
/// identically on both backends.
#[cfg(not(feature = "pinocchio"))]
pub fn collection_has_add_blocker(info: &AccountInfo) -> Result<bool> {
    if Owner::program_owner(info) != crate::ID {
        return Err(NaclacError::ConstraintOwner.into());
    }
    let solana_info = unsafe { info.to_lifetime() };
    let data = solana_info
        .try_borrow_data()
        .map_err(|_| NaclacError::AccountBorrowFailed.err(0))?;
    let collection_view = crate::collection::CollectionView::from_bytes(&data)?;
    let Some(plugin_header_offset) = collection_view.plugin_header_offset() else {
        return Ok(false);
    };
    Ok(find_plugin_offset(&data, plugin_header_offset, plugin_type::ADD_BLOCKER)?.is_some())
}

/// `true` if an `Asset` has an `AddBlocker` plugin attached. Callable
/// identically on both backends.
#[cfg(feature = "pinocchio")]
pub fn asset_has_add_blocker(info: &AccountInfo) -> Result<bool> {
    let asset_view = crate::asset::AssetView::try_from(info)?;
    let Some(plugin_header_offset) = asset_view.plugin_header_offset() else {
        return Ok(false);
    };
    Ok(
        find_plugin_offset(info.data(), plugin_header_offset, plugin_type::ADD_BLOCKER)?
            .is_some(),
    )
}

/// `true` if a `Collection` has an `AddBlocker` plugin attached. Callable
/// identically on both backends.
#[cfg(feature = "pinocchio")]
pub fn collection_has_add_blocker(info: &AccountInfo) -> Result<bool> {
    let collection_view = crate::collection::CollectionView::try_from(info)?;
    let Some(plugin_header_offset) = collection_view.plugin_header_offset() else {
        return Ok(false);
    };
    Ok(
        find_plugin_offset(info.data(), plugin_header_offset, plugin_type::ADD_BLOCKER)?
            .is_some(),
    )
}
