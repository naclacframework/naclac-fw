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

use crate::plugin_registry::{find_plugin_offset, plugin_type};

/// Attaches `BubblegumV2` to an `Asset` via `add_plugin`.
pub fn attach_bubblegum_v2_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let data = [2u8, plugin_type::BUBBLEGUM_V2, 0u8];
        add_asset_plugin_signed(program, accounts, &data, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mut data = crate::fixed_buf::FixedBuf::<3>::new();
        data.push(2u8); // AddPluginV1 discriminator
        data.push(plugin_type::BUBBLEGUM_V2);
        data.push(0u8); // init_authority: None
        add_asset_plugin_signed_pinocchio(program, accounts, data.as_slice(), signer_seeds)
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
    let asset_view = crate::asset::AssetView::from_bytes(&data)?;
    let Some(plugin_header_offset) = asset_view.plugin_header_offset() else {
        return Ok(false);
    };
    Ok(find_plugin_offset(&data, plugin_header_offset, plugin_type::BUBBLEGUM_V2)?.is_some())
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
