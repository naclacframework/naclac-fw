// ===========================================================================
// plugins/permanent_freeze_execute.rs — the `PermanentFreezeExecute` plugin
// ===========================================================================

//! `PermanentFreezeExecute` — like `FreezeExecute`, but attached at
//! creation time and permanent (can't be removed) for the life of the
//! asset. Real layout verified against `mpl-core`'s
//! `generated::types::PermanentFreezeExecute`: `{ frozen: bool }`.
//! Collection-level attachment verified against the real program's
//! `PluginType::manager()` (`UpdateAuthority`, not `Owner`) and confirmed
//! not on either collection-plugin ban list — a collection-level plugin
//! here is inherited by every member asset that doesn't set its own
//! (verified in `utils/mod.rs`'s `validate_asset_permissions`).

use crate::prelude::*;

use crate::plugin_registry::{find_plugin_offset, plugin_type};

/// Attaches `PermanentFreezeExecute` to an `Asset` via `add_plugin`.
pub fn attach_permanent_freeze_execute_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    frozen: bool,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let data = [2u8, plugin_type::PERMANENT_FREEZE_EXECUTE, frozen as u8, 0u8];
        add_asset_plugin_signed(program, accounts, &data, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mut data = crate::fixed_buf::FixedBuf::<4>::new();
        data.push(2u8); // AddPluginV1 discriminator
        data.push(plugin_type::PERMANENT_FREEZE_EXECUTE);
        data.push(frozen as u8);
        data.push(0u8); // init_authority: None
        add_asset_plugin_signed_pinocchio(program, accounts, data.as_slice(), signer_seeds)
    }
}

/// Attaches `PermanentFreezeExecute` to a `Collection` via
/// `add_collection_plugin`.
pub fn attach_collection_permanent_freeze_execute_signed(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    frozen: bool,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let data = [3u8, plugin_type::PERMANENT_FREEZE_EXECUTE, frozen as u8, 0u8];
        add_collection_plugin_signed(program, accounts, &data, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mut data = crate::fixed_buf::FixedBuf::<4>::new();
        data.push(3u8); // AddCollectionPluginV1 discriminator
        data.push(plugin_type::PERMANENT_FREEZE_EXECUTE);
        data.push(frozen as u8);
        data.push(0u8); // init_authority: None
        add_collection_plugin_signed_pinocchio(program, accounts, data.as_slice(), signer_seeds)
    }
}

/// Reads an `Asset`'s `PermanentFreezeExecute` plugin (whether it's
/// frozen), if attached. Callable identically on both backends.
#[cfg(not(feature = "pinocchio"))]
pub fn fetch_asset_permanent_freeze_execute(info: &AccountInfo) -> Result<Option<bool>> {
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
    read_permanent_freeze_execute(&data, plugin_header_offset)
}

/// Reads a `Collection`'s `PermanentFreezeExecute` plugin (whether it's
/// frozen), if attached. Callable identically on both backends.
#[cfg(not(feature = "pinocchio"))]
pub fn fetch_collection_permanent_freeze_execute(info: &AccountInfo) -> Result<Option<bool>> {
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
    read_permanent_freeze_execute(&data, plugin_header_offset)
}

/// Reads an `Asset`'s `PermanentFreezeExecute` plugin (whether it's
/// frozen), if attached. Callable identically on both backends.
#[cfg(feature = "pinocchio")]
pub fn fetch_asset_permanent_freeze_execute(info: &AccountInfo) -> Result<Option<bool>> {
    let asset_view = crate::asset::AssetView::try_from(info)?;
    let Some(plugin_header_offset) = asset_view.plugin_header_offset() else {
        return Ok(None);
    };
    read_permanent_freeze_execute(info.data(), plugin_header_offset)
}

/// Reads a `Collection`'s `PermanentFreezeExecute` plugin (whether it's
/// frozen), if attached. Callable identically on both backends.
#[cfg(feature = "pinocchio")]
pub fn fetch_collection_permanent_freeze_execute(info: &AccountInfo) -> Result<Option<bool>> {
    let collection_view = crate::collection::CollectionView::try_from(info)?;
    let Some(plugin_header_offset) = collection_view.plugin_header_offset() else {
        return Ok(None);
    };
    read_permanent_freeze_execute(info.data(), plugin_header_offset)
}

/// Lower-level variant of `fetch_asset_permanent_freeze_execute`/
/// `fetch_collection_permanent_freeze_execute`, shared by both backends:
/// see `edition.rs`'s `read_edition` for why this exists alongside the
/// uniform-signature wrappers above.
pub fn read_permanent_freeze_execute(
    data: &[u8],
    plugin_header_offset: usize,
) -> Result<Option<bool>> {
    match find_plugin_offset(data, plugin_header_offset, plugin_type::PERMANENT_FREEZE_EXECUTE)? {
        Some(offset) => {
            let offset = offset as usize;
            if data.len() <= offset {
                return Err(NaclacError::AccountDataTooSmall.err(0));
            }
            Ok(Some(data[offset] != 0))
        }
        None => Ok(None),
    }
}
