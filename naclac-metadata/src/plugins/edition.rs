// ===========================================================================
// plugins/edition.rs — the `Edition` plugin
// ===========================================================================

//! `Edition` — attaches to an `Asset`, recording its print number within a
//! `MasterEdition`-bearing `Collection`. Real layout verified against
//! `mpl-core`'s `generated::types::Edition`: `{ number: u32 }`. See
//! `master_edition.rs` for the per-`Collection` series-declaration
//! companion plugin.

use crate::prelude::*;

use crate::plugin_registry::{find_plugin_offset, plugin_type};

/// Attaches `Edition` to an `Asset` via `add_plugin`.
pub fn attach_edition_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    number: u32,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = crate::prelude::Vec::with_capacity(7);
        data.push(2u8); // AddPluginV1 discriminator
        data.push(plugin_type::EDITION);
        data.extend_from_slice(&number.to_le_bytes());
        data.push(0u8); // init_authority: None
        add_asset_plugin_signed(program, accounts, &data, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mut data = crate::fixed_buf::FixedBuf::<7>::new();
        data.push(2u8); // AddPluginV1 discriminator
        data.push(plugin_type::EDITION);
        data.extend_from_slice(&number.to_le_bytes());
        data.push(0u8); // init_authority: None
        add_asset_plugin_signed_pinocchio(program, accounts, data.as_slice(), signer_seeds)
    }
}

/// Reads an `Asset`'s `Edition` plugin (its print number), if attached.
/// Callable identically on both backends.
#[cfg(not(feature = "pinocchio"))]
pub fn fetch_asset_edition(info: &AccountInfo) -> Result<Option<u32>> {
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
    read_edition(&data, plugin_header_offset)
}

/// Reads an `Asset`'s `Edition` plugin (its print number), if attached.
/// Callable identically on both backends.
#[cfg(feature = "pinocchio")]
pub fn fetch_asset_edition(info: &AccountInfo) -> Result<Option<u32>> {
    let asset_view = crate::asset::AssetView::try_from(info)?;
    let Some(plugin_header_offset) = asset_view.plugin_header_offset() else {
        return Ok(None);
    };
    read_edition(info.data(), plugin_header_offset)
}

/// Lower-level variant of `fetch_asset_edition`, shared by both backends:
/// reads directly from an already-known `data`/`plugin_header_offset` pair
/// (`AssetView::plugin_header_offset()`) rather than re-parsing the base
/// `Asset` struct — useful when the caller already has both from other
/// work.
pub fn read_edition(data: &[u8], plugin_header_offset: usize) -> Result<Option<u32>> {
    match find_plugin_offset(data, plugin_header_offset, plugin_type::EDITION)? {
        Some(offset) => {
            let offset = offset as usize;
            let end = checked_end(offset, 4)?;
            if data.len() < end {
                return Err(NaclacError::AccountDataTooSmall.err(0));
            }
            Ok(Some(u32::from_le_bytes(
                data[offset..end].try_into().unwrap(),
            )))
        }
        None => Ok(None),
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Proves `read_edition` never panics end-to-end, including through
    /// `find_plugin_offset` and the now-`checked_end`-guarded entry point.
    #[kani::proof]
    #[kani::unwind(10)]
    fn prove_read_edition_never_panics() {
        let data: [u8; 24] = kani::any();
        let plugin_header_offset: usize = kani::any();
        let _ = read_edition(&data, plugin_header_offset);
    }
}
