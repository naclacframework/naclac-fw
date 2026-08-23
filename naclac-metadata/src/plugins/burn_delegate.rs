// ===========================================================================
// plugins/burn_delegate.rs — the `BurnDelegate` plugin
// ===========================================================================

//! `BurnDelegate` — attaches to an `Asset`, letting its plugin authority
//! burn the asset independent of its owner. Real layout verified against
//! `mpl-core`'s `generated::types::BurnDelegate`: an empty struct (Borsh
//! encodes it as zero bytes) — presence/absence of the plugin itself is the
//! only state.

use crate::prelude::*;

#[cfg(feature = "pinocchio")]
use crate::plugin_registry::{find_plugin_offset, plugin_type};

/// Attaches `BurnDelegate` to an `Asset` via `add_plugin`.
pub fn attach_burn_delegate_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let plugin = ::mpl_core::types::Plugin::BurnDelegate(::mpl_core::types::BurnDelegate {});
        add_asset_plugin_signed(program, accounts, plugin, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        add_asset_plugin_signed_pinocchio(
            program,
            accounts,
            plugin_type::BURN_DELEGATE,
            &[],
            signer_seeds,
        )
    }
}

/// `true` if an `Asset` has a `BurnDelegate` plugin attached. Callable
/// identically on both backends.
#[cfg(not(feature = "pinocchio"))]
pub fn asset_has_burn_delegate(info: &AccountInfo) -> Result<bool> {
    if Owner::program_owner(info) != crate::ID {
        return Err(NaclacError::ConstraintOwner.into());
    }
    let solana_info = unsafe { info.to_lifetime() };
    let data = solana_info
        .try_borrow_data()
        .map_err(|_| NaclacError::AccountBorrowFailed.err(0))?;
    let asset = ::mpl_core::Asset::deserialize(&data)
        .map_err(|_| NaclacError::DeserializationFailed.err(0))?;
    Ok(asset.plugin_list.burn_delegate.is_some())
}

/// `true` if an `Asset` has a `BurnDelegate` plugin attached. Callable
/// identically on both backends.
#[cfg(feature = "pinocchio")]
pub fn asset_has_burn_delegate(info: &AccountInfo) -> Result<bool> {
    let asset_view = crate::asset::AssetView::try_from(info)?;
    let Some(plugin_header_offset) = asset_view.plugin_header_offset() else {
        return Ok(false);
    };
    Ok(
        find_plugin_offset(info.data(), plugin_header_offset, plugin_type::BURN_DELEGATE)?
            .is_some(),
    )
}
