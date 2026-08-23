// ===========================================================================
// plugins/transfer_delegate.rs — the `TransferDelegate` plugin
// ===========================================================================

//! `TransferDelegate` — attaches to an `Asset`, letting its plugin
//! authority transfer the asset independent of its owner. Real layout
//! verified against `mpl-core`'s `generated::types::TransferDelegate`: an
//! empty struct (Borsh encodes it as zero bytes) — presence/absence of the
//! plugin itself is the only state.

use crate::prelude::*;

#[cfg(feature = "pinocchio")]
use crate::plugin_registry::{find_plugin_offset, plugin_type};

/// Attaches `TransferDelegate` to an `Asset` via `add_plugin`.
pub fn attach_transfer_delegate_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let plugin =
            ::mpl_core::types::Plugin::TransferDelegate(::mpl_core::types::TransferDelegate {});
        add_asset_plugin_signed(program, accounts, plugin, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        add_asset_plugin_signed_pinocchio(
            program,
            accounts,
            plugin_type::TRANSFER_DELEGATE,
            &[],
            signer_seeds,
        )
    }
}

/// `true` if an `Asset` has a `TransferDelegate` plugin attached. Callable
/// identically on both backends.
#[cfg(not(feature = "pinocchio"))]
pub fn asset_has_transfer_delegate(info: &AccountInfo) -> Result<bool> {
    if Owner::program_owner(info) != crate::ID {
        return Err(NaclacError::ConstraintOwner.into());
    }
    let solana_info = unsafe { info.to_lifetime() };
    let data = solana_info
        .try_borrow_data()
        .map_err(|_| NaclacError::AccountBorrowFailed.err(0))?;
    let asset = ::mpl_core::Asset::deserialize(&data)
        .map_err(|_| NaclacError::DeserializationFailed.err(0))?;
    Ok(asset.plugin_list.transfer_delegate.is_some())
}

/// `true` if an `Asset` has a `TransferDelegate` plugin attached. Callable
/// identically on both backends.
#[cfg(feature = "pinocchio")]
pub fn asset_has_transfer_delegate(info: &AccountInfo) -> Result<bool> {
    let asset_view = crate::asset::AssetView::try_from(info)?;
    let Some(plugin_header_offset) = asset_view.plugin_header_offset() else {
        return Ok(false);
    };
    Ok(find_plugin_offset(
        info.data(),
        plugin_header_offset,
        plugin_type::TRANSFER_DELEGATE,
    )?
    .is_some())
}
