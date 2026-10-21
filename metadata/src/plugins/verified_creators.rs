// ===========================================================================
// plugins/verified_creators.rs — the `VerifiedCreators` plugin
// ===========================================================================

//! `VerifiedCreators` — attaches to an `Asset` or `Collection`, holding a
//! list of creator addresses and whether each has verified their
//! inclusion. Real layout verified against `mpl-core`'s
//! `generated::types::{VerifiedCreators, VerifiedCreatorsSignature}`:
//! `VerifiedCreators { signatures: Vec<VerifiedCreatorsSignature> }`,
//! `VerifiedCreatorsSignature { address: Address, verified: bool }` — fixed
//! 33 bytes each, no per-element offset walk needed (unlike
//! `Attributes`/`Autograph`). Collection-level attachment verified against
//! the real program's `PluginType::manager()` (`UpdateAuthority`, not
//! `Owner`) and confirmed not on either collection-plugin ban list
//! (`add_collection_plugin`'s `Groups`-only check, `create_collection`'s
//! `Edition`/`Groups`/owner-managed check) — a collection-level plugin here
//! is inherited by every member asset that doesn't set its own (verified in
//! `utils/mod.rs`'s `validate_asset_permissions`).

use crate::{
    plugin_registry::{find_plugin_offset, plugin_type},
    prelude::*,
};

/// One entry of `VerifiedCreators.signatures` — backend-neutral, since the
/// real `mpl-core` `VerifiedCreatorsSignature` type isn't available on
/// `pinocchio`. Stored on-chain as the address (32 bytes) followed by one
/// byte for `verified`; any nonzero byte reads as `true`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerifiedCreatorSignature {
    pub address: Address,
    pub verified: bool,
}

impl Record for VerifiedCreatorSignature {
    const SIZE: usize = 33;

    fn decode(bytes: &[u8]) -> Self {
        Self {
            address: <Address as Record>::decode(bytes.get(..32).unwrap_or(&[])),
            verified: bytes.get(32).copied().unwrap_or(0) != 0,
        }
    }
}

/// Maximum `signatures` entries `attach_verified_creators_signed`/
/// `attach_collection_verified_creators_signed` accept on `pinocchio` — no
/// real protocol maximum exists (verified: `mpl-core`'s processor enforces
/// none), so this is a fixed cap sized for a stack buffer rather than a
/// heap `Vec`.
#[cfg(feature = "pinocchio")]
pub const MAX_VERIFIED_CREATOR_SIGNATURES: usize = 16;

/// Attaches `VerifiedCreators` to an `Asset` via `add_plugin`.
pub fn attach_verified_creators_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    signatures: &[VerifiedCreatorSignature],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = crate::prelude::Vec::new();
        data.push(2u8); // AddPluginV1 discriminator
        data.push(plugin_type::VERIFIED_CREATORS);
        data.extend_from_slice(&(signatures.len() as u32).to_le_bytes());
        for sig in signatures {
            data.extend_from_slice(sig.address.as_ref());
            data.push(u8::from(sig.verified));
        }
        data.push(0u8); // init_authority: None
        add_asset_plugin_signed(program, accounts, &data, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        if signatures.len() > MAX_VERIFIED_CREATOR_SIGNATURES {
            return Err(NaclacError::InvalidInstructionData.err(0));
        }
        let mut data =
            crate::fixed_buf::FixedBuf::<{ 3 + 4 + MAX_VERIFIED_CREATOR_SIGNATURES * 33 }>::new();
        data.push(2u8); // AddPluginV1 discriminator
        data.push(plugin_type::VERIFIED_CREATORS);
        data.extend_from_slice(&(signatures.len() as u32).to_le_bytes());
        for sig in signatures {
            data.extend_from_slice(sig.address.as_ref());
            data.push(u8::from(sig.verified));
        }
        data.push(0u8); // init_authority: None
        add_asset_plugin_signed_pinocchio(program, accounts, data.as_slice(), signer_seeds)
    }
}

/// Attaches `VerifiedCreators` to a `Collection` via `add_collection_plugin`.
pub fn attach_collection_verified_creators_signed(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    signatures: &[VerifiedCreatorSignature],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = crate::prelude::Vec::new();
        data.push(3u8); // AddCollectionPluginV1 discriminator
        data.push(plugin_type::VERIFIED_CREATORS);
        data.extend_from_slice(&(signatures.len() as u32).to_le_bytes());
        for sig in signatures {
            data.extend_from_slice(sig.address.as_ref());
            data.push(u8::from(sig.verified));
        }
        data.push(0u8); // init_authority: None
        add_collection_plugin_signed(program, accounts, &data, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        if signatures.len() > MAX_VERIFIED_CREATOR_SIGNATURES {
            return Err(NaclacError::InvalidInstructionData.err(0));
        }
        let mut data =
            crate::fixed_buf::FixedBuf::<{ 3 + 4 + MAX_VERIFIED_CREATOR_SIGNATURES * 33 }>::new();
        data.push(3u8); // AddCollectionPluginV1 discriminator
        data.push(plugin_type::VERIFIED_CREATORS);
        data.extend_from_slice(&(signatures.len() as u32).to_le_bytes());
        for sig in signatures {
            data.extend_from_slice(sig.address.as_ref());
            data.push(u8::from(sig.verified));
        }
        data.push(0u8); // init_authority: None
        add_collection_plugin_signed_pinocchio(program, accounts, data.as_slice(), signer_seeds)
    }
}

/// Reads an `Asset`'s `VerifiedCreators` plugin, if attached. Callable
/// identically on both backends.
#[cfg(not(feature = "pinocchio"))]
pub fn fetch_asset_verified_creators(
    info: &AccountInfo,
) -> Result<Option<crate::prelude::Vec<VerifiedCreatorSignature>>> {
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
    read_verified_creators_owned(&data, plugin_header_offset)
}

/// Reads a `Collection`'s `VerifiedCreators` plugin, if attached. Callable
/// identically on both backends.
#[cfg(not(feature = "pinocchio"))]
pub fn fetch_collection_verified_creators(
    info: &AccountInfo,
) -> Result<Option<crate::prelude::Vec<VerifiedCreatorSignature>>> {
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
    read_verified_creators_owned(&data, plugin_header_offset)
}

/// `solana`-only: walks the same fixed 33-byte (32-byte `address` + 1-byte
/// `verified`) records `read_verified_creators` does, but builds owned
/// entries instead of a borrowed `RecordList`.
#[cfg(not(feature = "pinocchio"))]
fn read_verified_creators_owned(
    data: &[u8],
    plugin_header_offset: usize,
) -> Result<Option<crate::prelude::Vec<VerifiedCreatorSignature>>> {
    let Some(offset) =
        find_plugin_offset(data, plugin_header_offset, plugin_type::VERIFIED_CREATORS)?
    else {
        return Ok(None);
    };
    let offset = offset as usize;

    let list_start = checked_end(offset, 4)?;
    if data.len() < list_start {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let count = read_u32(data, offset)? as usize;
    let list_end = count
        .checked_mul(33)
        .and_then(|bytes| list_start.checked_add(bytes))
        .ok_or_else(|| NaclacError::AccountDataTooSmall.err(0))?;
    if data.len() < list_end {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }

    let mut entries = crate::prelude::Vec::with_capacity(count);
    for i in 0..count {
        let entry_start = list_start + i * 33;
        let bytes: [u8; 32] = read_array(data, entry_start)?;
        entries.push(VerifiedCreatorSignature {
            address: Address::new_from_array(bytes),
            verified: data[entry_start + 32] != 0,
        });
    }
    Ok(Some(entries))
}

/// Reads an `Asset`'s `VerifiedCreators` plugin, if attached. Callable
/// identically on both backends.
#[cfg(feature = "pinocchio")]
pub fn fetch_asset_verified_creators(
    info: &AccountInfo,
) -> Result<Option<RecordList<'_, VerifiedCreatorSignature>>> {
    let asset_view = crate::asset::AssetView::try_from(info)?;
    let Some(plugin_header_offset) = asset_view.plugin_header_offset() else {
        return Ok(None);
    };
    read_verified_creators(info.data(), plugin_header_offset)
}

/// Reads a `Collection`'s `VerifiedCreators` plugin, if attached. Callable
/// identically on both backends.
#[cfg(feature = "pinocchio")]
pub fn fetch_collection_verified_creators(
    info: &AccountInfo,
) -> Result<Option<RecordList<'_, VerifiedCreatorSignature>>> {
    let collection_view = crate::collection::CollectionView::try_from(info)?;
    let Some(plugin_header_offset) = collection_view.plugin_header_offset() else {
        return Ok(None);
    };
    read_verified_creators(info.data(), plugin_header_offset)
}

/// Lower-level variant of `fetch_asset_verified_creators`/
/// `fetch_collection_verified_creators`, `pinocchio`-only: see
/// `edition.rs`'s `read_edition` for why this exists alongside the
/// uniform-signature wrappers above. Zero-copy — the `RecordList` views
/// directly into `data`, no heap allocation, no artificial cap
/// (reflects however many signatures are actually stored on-chain).
#[cfg(feature = "pinocchio")]
pub fn read_verified_creators(
    data: &[u8],
    plugin_header_offset: usize,
) -> Result<Option<RecordList<'_, VerifiedCreatorSignature>>> {
    let Some(offset) =
        find_plugin_offset(data, plugin_header_offset, plugin_type::VERIFIED_CREATORS)?
    else {
        return Ok(None);
    };
    let offset = offset as usize;

    let list_start = checked_end(offset, 4)?;
    if data.len() < list_start {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let count = read_u32(data, offset)? as usize;
    let list_end = count
        .checked_mul(33)
        .and_then(|bytes| list_start.checked_add(bytes))
        .ok_or_else(|| NaclacError::AccountDataTooSmall.err(0))?;
    if data.len() < list_end {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    Ok(Some(RecordList::from_bytes(&data[list_start..list_end])?))
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Proves the non-pinocchio (`solana`) variant never panics end-to-end,
    /// including through `find_plugin_offset` and the now-`checked_end`-
    /// guarded count×stride arithmetic.
    #[cfg(not(feature = "pinocchio"))]
    #[kani::proof]
    #[kani::unwind(10)]
    fn prove_read_verified_creators_owned_never_panics() {
        let data: [u8; 40] = kani::any();
        let plugin_header_offset: usize = kani::any();
        let _ = read_verified_creators_owned(&data, plugin_header_offset);
    }

    /// Same proof for the pinocchio (zero-copy) variant.
    #[cfg(feature = "pinocchio")]
    #[kani::proof]
    #[kani::unwind(10)]
    fn prove_read_verified_creators_never_panics() {
        let data: [u8; 40] = kani::any();
        let plugin_header_offset: usize = kani::any();
        let _ = read_verified_creators(&data, plugin_header_offset);
    }
}
