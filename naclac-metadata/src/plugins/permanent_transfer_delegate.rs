// ===========================================================================
// plugins/permanent_transfer_delegate.rs — the `PermanentTransferDelegate` plugin
// ===========================================================================

//! `PermanentTransferDelegate` — like `TransferDelegate`, but attached at
//! creation time and permanent (can't be removed) for the life of the
//! asset. Real layout verified against `mpl-core`'s
//! `generated::types::PermanentTransferDelegate`: an empty struct (Borsh
//! encodes it as zero bytes). Collection-level attachment verified against
//! the real program's `PluginType::manager()` (`UpdateAuthority`, not
//! `Owner`) and confirmed not on either collection-plugin ban list — a
//! collection-level plugin here is inherited by every member asset that
//! doesn't set its own (verified in `utils/mod.rs`'s
//! `validate_asset_permissions`).

use crate::prelude::*;

#[cfg(feature = "pinocchio")]
use crate::plugin_registry::{find_plugin_offset, plugin_type};

/// Attaches `PermanentTransferDelegate` to an `Asset` via `add_plugin`.
pub fn attach_permanent_transfer_delegate_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let plugin = ::mpl_core::types::Plugin::PermanentTransferDelegate(
            ::mpl_core::types::PermanentTransferDelegate {},
        );
        add_asset_plugin_signed(program, accounts, plugin, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        add_asset_plugin_signed_pinocchio(
            program,
            accounts,
            plugin_type::PERMANENT_TRANSFER_DELEGATE,
            &[],
            signer_seeds,
        )
    }
}

/// Attaches `PermanentTransferDelegate` to a `Collection` via
/// `add_collection_plugin`.
pub fn attach_collection_permanent_transfer_delegate_signed(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let plugin = ::mpl_core::types::Plugin::PermanentTransferDelegate(
            ::mpl_core::types::PermanentTransferDelegate {},
        );
        add_collection_plugin_signed(program, accounts, plugin, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        add_collection_plugin_signed_pinocchio(
            program,
            accounts,
            plugin_type::PERMANENT_TRANSFER_DELEGATE,
            &[],
            signer_seeds,
        )
    }
}

/// `true` if an `Asset` has a `PermanentTransferDelegate` plugin attached.
/// Callable identically on both backends.
#[cfg(not(feature = "pinocchio"))]
pub fn asset_has_permanent_transfer_delegate(info: &AccountInfo) -> Result<bool> {
    if Owner::program_owner(info) != crate::ID {
        return Err(NaclacError::ConstraintOwner.into());
    }
    let solana_info = unsafe { info.to_lifetime() };
    let data = solana_info
        .try_borrow_data()
        .map_err(|_| NaclacError::AccountBorrowFailed.err(0))?;
    let asset = ::mpl_core::Asset::deserialize(&data)
        .map_err(|_| NaclacError::DeserializationFailed.err(0))?;
    Ok(asset.plugin_list.permanent_transfer_delegate.is_some())
}

/// `true` if a `Collection` has a `PermanentTransferDelegate` plugin
/// attached. Callable identically on both backends.
#[cfg(not(feature = "pinocchio"))]
pub fn collection_has_permanent_transfer_delegate(info: &AccountInfo) -> Result<bool> {
    if Owner::program_owner(info) != crate::ID {
        return Err(NaclacError::ConstraintOwner.into());
    }
    let solana_info = unsafe { info.to_lifetime() };
    let data = solana_info
        .try_borrow_data()
        .map_err(|_| NaclacError::AccountBorrowFailed.err(0))?;
    let collection = ::mpl_core::Collection::deserialize(&data)
        .map_err(|_| NaclacError::DeserializationFailed.err(0))?;
    Ok(collection.plugin_list.permanent_transfer_delegate.is_some())
}

/// `true` if an `Asset` has a `PermanentTransferDelegate` plugin attached.
/// Callable identically on both backends.
#[cfg(feature = "pinocchio")]
pub fn asset_has_permanent_transfer_delegate(info: &AccountInfo) -> Result<bool> {
    let asset_view = crate::asset::AssetView::try_from(info)?;
    let Some(plugin_header_offset) = asset_view.plugin_header_offset() else {
        return Ok(false);
    };
    Ok(find_plugin_offset(
        info.data(),
        plugin_header_offset,
        plugin_type::PERMANENT_TRANSFER_DELEGATE,
    )?
    .is_some())
}

/// `true` if a `Collection` has a `PermanentTransferDelegate` plugin
/// attached. Callable identically on both backends.
#[cfg(feature = "pinocchio")]
pub fn collection_has_permanent_transfer_delegate(info: &AccountInfo) -> Result<bool> {
    let collection_view = crate::collection::CollectionView::try_from(info)?;
    let Some(plugin_header_offset) = collection_view.plugin_header_offset() else {
        return Ok(false);
    };
    Ok(find_plugin_offset(
        info.data(),
        plugin_header_offset,
        plugin_type::PERMANENT_TRANSFER_DELEGATE,
    )?
    .is_some())
}
