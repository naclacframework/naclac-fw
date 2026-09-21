// ===========================================================================
// plugins/update_delegate.rs — the `UpdateDelegate` plugin
// ===========================================================================

//! `UpdateDelegate` — attaches to an `Asset` or `Collection`, naming extra
//! addresses (beyond the update authority itself) allowed to update it.
//! Real layout verified against `mpl-core`'s
//! `generated::types::UpdateDelegate`: `{ additional_delegates: Vec<Pubkey> }`
//! — a 4-byte little-endian count then that many 32-byte pubkeys, each
//! fixed-width (unlike most other plugins' `Vec`s), so no per-element
//! offset walking is needed here.

use crate::prelude::*;

use crate::plugin_registry::{find_plugin_offset, plugin_type};

/// Maximum `additional_delegates` entries `attach_update_delegate_signed`/
/// `attach_collection_update_delegate_signed` accept on `pinocchio` — no
/// real protocol maximum exists, so this is a fixed cap sized for a stack
/// buffer rather than a heap `Vec` (same reasoning as
/// `verified_creators::MAX_VERIFIED_CREATOR_SIGNATURES`).
#[cfg(feature = "pinocchio")]
pub const MAX_ADDITIONAL_DELEGATES: usize = 16;

/// Attaches `UpdateDelegate` to an `Asset` via `add_plugin`.
pub fn attach_update_delegate_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    additional_delegates: &[Address],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = crate::prelude::Vec::new();
        data.push(2u8); // AddPluginV1 discriminator
        data.push(plugin_type::UPDATE_DELEGATE);
        data.extend_from_slice(&(additional_delegates.len() as u32).to_le_bytes());
        for address in additional_delegates {
            data.extend_from_slice(address.as_ref());
        }
        data.push(0u8); // init_authority: None
        add_asset_plugin_signed(program, accounts, &data, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        if additional_delegates.len() > MAX_ADDITIONAL_DELEGATES {
            return Err(NaclacError::InvalidInstructionData.err(0));
        }
        let mut data =
            crate::fixed_buf::FixedBuf::<{ 3 + 4 + MAX_ADDITIONAL_DELEGATES * 32 }>::new();
        data.push(2u8); // AddPluginV1 discriminator
        data.push(plugin_type::UPDATE_DELEGATE);
        data.extend_from_slice(&(additional_delegates.len() as u32).to_le_bytes());
        for address in additional_delegates {
            data.extend_from_slice(address.as_ref());
        }
        data.push(0u8); // init_authority: None
        add_asset_plugin_signed_pinocchio(program, accounts, data.as_slice(), signer_seeds)
    }
}

/// Attaches `UpdateDelegate` to a `Collection` via `add_collection_plugin`.
pub fn attach_collection_update_delegate_signed(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    additional_delegates: &[Address],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = crate::prelude::Vec::new();
        data.push(3u8); // AddCollectionPluginV1 discriminator
        data.push(plugin_type::UPDATE_DELEGATE);
        data.extend_from_slice(&(additional_delegates.len() as u32).to_le_bytes());
        for address in additional_delegates {
            data.extend_from_slice(address.as_ref());
        }
        data.push(0u8); // init_authority: None
        add_collection_plugin_signed(program, accounts, &data, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        if additional_delegates.len() > MAX_ADDITIONAL_DELEGATES {
            return Err(NaclacError::InvalidInstructionData.err(0));
        }
        let mut data =
            crate::fixed_buf::FixedBuf::<{ 3 + 4 + MAX_ADDITIONAL_DELEGATES * 32 }>::new();
        data.push(3u8); // AddCollectionPluginV1 discriminator
        data.push(plugin_type::UPDATE_DELEGATE);
        data.extend_from_slice(&(additional_delegates.len() as u32).to_le_bytes());
        for address in additional_delegates {
            data.extend_from_slice(address.as_ref());
        }
        data.push(0u8); // init_authority: None
        add_collection_plugin_signed_pinocchio(program, accounts, data.as_slice(), signer_seeds)
    }
}

/// Reads an `Asset`'s `UpdateDelegate` plugin's `additional_delegates`, if
/// attached. Callable identically on both backends.
#[cfg(not(feature = "pinocchio"))]
pub fn fetch_asset_update_delegate(
    info: &AccountInfo,
) -> Result<Option<crate::prelude::Vec<Address>>> {
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
    Ok(read_additional_delegates(&data, plugin_header_offset)?.map(|span| span.iter().collect()))
}

/// Reads a `Collection`'s `UpdateDelegate` plugin's `additional_delegates`,
/// if attached. Callable identically on both backends.
#[cfg(not(feature = "pinocchio"))]
pub fn fetch_collection_update_delegate(
    info: &AccountInfo,
) -> Result<Option<crate::prelude::Vec<Address>>> {
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
    Ok(read_additional_delegates(&data, plugin_header_offset)?.map(|span| span.iter().collect()))
}

/// Reads an `Asset`'s `UpdateDelegate` plugin's `additional_delegates`, if
/// attached. Callable identically on both backends.
#[cfg(feature = "pinocchio")]
pub fn fetch_asset_update_delegate(info: &AccountInfo) -> Result<Option<Span<Address>>> {
    let asset_view = crate::asset::AssetView::try_from(info)?;
    let Some(plugin_header_offset) = asset_view.plugin_header_offset() else {
        return Ok(None);
    };
    read_additional_delegates(info.data(), plugin_header_offset)
}

/// Reads a `Collection`'s `UpdateDelegate` plugin's `additional_delegates`,
/// if attached. Callable identically on both backends.
#[cfg(feature = "pinocchio")]
pub fn fetch_collection_update_delegate(info: &AccountInfo) -> Result<Option<Span<Address>>> {
    let collection_view = crate::collection::CollectionView::try_from(info)?;
    let Some(plugin_header_offset) = collection_view.plugin_header_offset() else {
        return Ok(None);
    };
    read_additional_delegates(info.data(), plugin_header_offset)
}

/// Lower-level variant of `fetch_asset_update_delegate`, shared by both
/// backends: see `edition.rs`'s `read_edition` for why this exists alongside
/// the uniform-signature wrapper above. Zero-copy — `Span<Address>` views
/// directly into `data`, no heap allocation, no artificial cap (reflects
/// however many delegates are actually stored on-chain); `solana`-side
/// callers collect it into an owned `Vec` to keep their existing signature.
pub fn read_additional_delegates(
    data: &[u8],
    plugin_header_offset: usize,
) -> Result<Option<Span<Address>>> {
    let Some(offset) = find_plugin_offset(data, plugin_header_offset, plugin_type::UPDATE_DELEGATE)?
    else {
        return Ok(None);
    };
    let offset = offset as usize;
    let list_start = checked_end(offset, 4)?;
    if data.len() < list_start {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let count = u32::from_le_bytes(data[offset..list_start].try_into().unwrap()) as usize;
    let list_end = count
        .checked_mul(32)
        .and_then(|bytes| list_start.checked_add(bytes))
        .ok_or_else(|| NaclacError::AccountDataTooSmall.err(0))?;
    if data.len() < list_end {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    Ok(Some(Span::from_bytes(&data[list_start..list_end])?))
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Proves `read_additional_delegates` never panics end-to-end,
    /// including through `find_plugin_offset` and the now-`checked_end`-
    /// guarded count×stride arithmetic.
    #[kani::proof]
    #[kani::unwind(10)]
    fn prove_read_additional_delegates_never_panics() {
        let data: [u8; 40] = kani::any();
        let plugin_header_offset: usize = kani::any();
        let _ = read_additional_delegates(&data, plugin_header_offset);
    }
}
