// ===========================================================================
// plugins/freeze_execute.rs — the `FreezeExecute` plugin
// ===========================================================================

//! `FreezeExecute` — gates the `execute` lifecycle action (running
//! arbitrary CPIs as the asset) behind a frozen flag, mirroring
//! `FreezeDelegate`'s shape for transfers. Real layout verified against
//! `mpl-core`'s `generated::types::FreezeExecute`: `{ frozen: bool }`.
//! Attached at the `Asset` level, by the same structural parallel to
//! `FreezeDelegate` — not separately verified against a real `execute`-CPI
//! usage example.

use crate::prelude::*;

use crate::plugin_registry::{find_plugin_offset, plugin_type};

/// Attaches `FreezeExecute` to an `Asset` via `add_plugin`.
pub fn attach_freeze_execute_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    frozen: bool,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let data = [2u8, plugin_type::FREEZE_EXECUTE, frozen as u8, 0u8];
        add_asset_plugin_signed(program, accounts, &data, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mut data = crate::fixed_buf::FixedBuf::<4>::new();
        data.push(2u8); // AddPluginV1 discriminator
        data.push(plugin_type::FREEZE_EXECUTE);
        data.push(frozen as u8);
        data.push(0u8); // init_authority: None
        add_asset_plugin_signed_pinocchio(program, accounts, data.as_slice(), signer_seeds)
    }
}

/// Reads an `Asset`'s `FreezeExecute` plugin (whether it's frozen), if
/// attached. Callable identically on both backends.
#[cfg(not(feature = "pinocchio"))]
pub fn fetch_asset_freeze_execute(info: &AccountInfo) -> Result<Option<bool>> {
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
    match find_plugin_offset(&data, plugin_header_offset, plugin_type::FREEZE_EXECUTE)? {
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

/// Reads an `Asset`'s `FreezeExecute` plugin (whether it's frozen), if
/// attached. Callable identically on both backends.
#[cfg(feature = "pinocchio")]
pub fn fetch_asset_freeze_execute(info: &AccountInfo) -> Result<Option<bool>> {
    let asset_view = crate::asset::AssetView::try_from(info)?;
    let Some(plugin_header_offset) = asset_view.plugin_header_offset() else {
        return Ok(None);
    };
    match find_plugin_offset(info.data(), plugin_header_offset, plugin_type::FREEZE_EXECUTE)? {
        Some(offset) => {
            let offset = offset as usize;
            if info.data().len() <= offset {
                return Err(NaclacError::AccountDataTooSmall.err(0));
            }
            Ok(Some(info.data()[offset] != 0))
        }
        None => Ok(None),
    }
}
