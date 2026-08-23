// ===========================================================================
// plugins/immutable_metadata.rs — the `ImmutableMetadata` plugin
// ===========================================================================

//! `ImmutableMetadata` — freezes `name`/`uri` against future `update_v2`
//! calls. Real layout verified against `mpl-core`'s
//! `generated::types::ImmutableMetadata`: an empty struct (Borsh encodes it
//! as zero bytes). Provided for both `Asset` and `Collection` since nothing
//! in the real crate's generated code pins it to one level.

use crate::prelude::*;

#[cfg(feature = "pinocchio")]
use crate::plugin_registry::{find_plugin_offset, plugin_type};

/// Attaches `ImmutableMetadata` to an `Asset` via `add_plugin`.
pub fn attach_immutable_metadata_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let plugin =
            ::mpl_core::types::Plugin::ImmutableMetadata(::mpl_core::types::ImmutableMetadata {});
        add_asset_plugin_signed(program, accounts, plugin, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        add_asset_plugin_signed_pinocchio(
            program,
            accounts,
            plugin_type::IMMUTABLE_METADATA,
            &[],
            signer_seeds,
        )
    }
}

/// Attaches `ImmutableMetadata` to a `Collection` via `add_collection_plugin`.
pub fn attach_collection_immutable_metadata_signed(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let plugin =
            ::mpl_core::types::Plugin::ImmutableMetadata(::mpl_core::types::ImmutableMetadata {});
        add_collection_plugin_signed(program, accounts, plugin, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        add_collection_plugin_signed_pinocchio(
            program,
            accounts,
            plugin_type::IMMUTABLE_METADATA,
            &[],
            signer_seeds,
        )
    }
}

/// `true` if an `Asset` has an `ImmutableMetadata` plugin attached.
/// Callable identically on both backends.
#[cfg(not(feature = "pinocchio"))]
pub fn asset_has_immutable_metadata(info: &AccountInfo) -> Result<bool> {
    if Owner::program_owner(info) != crate::ID {
        return Err(NaclacError::ConstraintOwner.into());
    }
    let solana_info = unsafe { info.to_lifetime() };
    let data = solana_info
        .try_borrow_data()
        .map_err(|_| NaclacError::AccountBorrowFailed.err(0))?;
    let asset = ::mpl_core::Asset::deserialize(&data)
        .map_err(|_| NaclacError::DeserializationFailed.err(0))?;
    Ok(asset.plugin_list.immutable_metadata.is_some())
}

/// `true` if a `Collection` has an `ImmutableMetadata` plugin attached.
/// Callable identically on both backends.
#[cfg(not(feature = "pinocchio"))]
pub fn collection_has_immutable_metadata(info: &AccountInfo) -> Result<bool> {
    if Owner::program_owner(info) != crate::ID {
        return Err(NaclacError::ConstraintOwner.into());
    }
    let solana_info = unsafe { info.to_lifetime() };
    let data = solana_info
        .try_borrow_data()
        .map_err(|_| NaclacError::AccountBorrowFailed.err(0))?;
    let collection = ::mpl_core::Collection::deserialize(&data)
        .map_err(|_| NaclacError::DeserializationFailed.err(0))?;
    Ok(collection.plugin_list.immutable_metadata.is_some())
}

/// `true` if an `Asset` has an `ImmutableMetadata` plugin attached.
/// Callable identically on both backends.
#[cfg(feature = "pinocchio")]
pub fn asset_has_immutable_metadata(info: &AccountInfo) -> Result<bool> {
    let asset_view = crate::asset::AssetView::try_from(info)?;
    let Some(plugin_header_offset) = asset_view.plugin_header_offset() else {
        return Ok(false);
    };
    Ok(find_plugin_offset(
        info.data(),
        plugin_header_offset,
        plugin_type::IMMUTABLE_METADATA,
    )?
    .is_some())
}

/// `true` if a `Collection` has an `ImmutableMetadata` plugin attached.
/// Callable identically on both backends.
#[cfg(feature = "pinocchio")]
pub fn collection_has_immutable_metadata(info: &AccountInfo) -> Result<bool> {
    let collection_view = crate::collection::CollectionView::try_from(info)?;
    let Some(plugin_header_offset) = collection_view.plugin_header_offset() else {
        return Ok(false);
    };
    Ok(find_plugin_offset(
        info.data(),
        plugin_header_offset,
        plugin_type::IMMUTABLE_METADATA,
    )?
    .is_some())
}
