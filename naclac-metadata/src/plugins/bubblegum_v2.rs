// ===========================================================================
// plugins/bubblegum_v2.rs — the `BubblegumV2` plugin
// ===========================================================================

//! `BubblegumV2` — a marker plugin used by Metaplex's Bubblegum
//! (compressed NFT) program's own integration with Core; not something a
//! naclac program would typically attach itself, but implemented for
//! completeness/read access since it's part of the real `PluginType` enum.
//! Real layout verified against `mpl-core`'s
//! `generated::types::BubblegumV2`: an empty struct (Borsh encodes it as
//! zero bytes).

use crate::prelude::*;

#[cfg(feature = "pinocchio")]
use crate::plugin_registry::{find_plugin_offset, plugin_type};

/// Attaches `BubblegumV2` to an `Asset` via `add_plugin`.
pub fn attach_bubblegum_v2_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let plugin = ::mpl_core::types::Plugin::BubblegumV2(::mpl_core::types::BubblegumV2 {});
        add_asset_plugin_signed(program, accounts, plugin, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        add_asset_plugin_signed_pinocchio(
            program,
            accounts,
            plugin_type::BUBBLEGUM_V2,
            &[],
            signer_seeds,
        )
    }
}

/// `true` if an `Asset` has a `BubblegumV2` plugin attached. Callable
/// identically on both backends.
#[cfg(not(feature = "pinocchio"))]
pub fn asset_has_bubblegum_v2(info: &AccountInfo) -> Result<bool> {
    if Owner::program_owner(info) != crate::ID {
        return Err(NaclacError::ConstraintOwner.into());
    }
    let solana_info = unsafe { info.to_lifetime() };
    let data = solana_info
        .try_borrow_data()
        .map_err(|_| NaclacError::AccountBorrowFailed.err(0))?;
    let asset = ::mpl_core::Asset::deserialize(&data)
        .map_err(|_| NaclacError::DeserializationFailed.err(0))?;
    Ok(asset.plugin_list.bubblegum_v2.is_some())
}

/// `true` if an `Asset` has a `BubblegumV2` plugin attached. Callable
/// identically on both backends.
#[cfg(feature = "pinocchio")]
pub fn asset_has_bubblegum_v2(info: &AccountInfo) -> Result<bool> {
    let asset_view = crate::asset::AssetView::try_from(info)?;
    let Some(plugin_header_offset) = asset_view.plugin_header_offset() else {
        return Ok(false);
    };
    Ok(
        find_plugin_offset(info.data(), plugin_header_offset, plugin_type::BUBBLEGUM_V2)?
            .is_some(),
    )
}
