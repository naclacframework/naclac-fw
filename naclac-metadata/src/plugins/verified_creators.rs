// ===========================================================================
// plugins/verified_creators.rs — the `VerifiedCreators` plugin
// ===========================================================================

//! `VerifiedCreators` — attaches to an `Asset` or `Collection`, holding a
//! list of creator addresses and whether each has verified their
//! inclusion. Real layout verified against `mpl-core`'s
//! `generated::types::{VerifiedCreators, VerifiedCreatorsSignature}`:
//! `VerifiedCreators { signatures: Vec<VerifiedCreatorsSignature> }`,
//! `VerifiedCreatorsSignature { address: Pubkey, verified: bool }` — fixed
//! 33 bytes each, no per-element offset walk needed (unlike
//! `Attributes`/`Autograph`). Collection-level attachment verified against
//! the real program's `PluginType::manager()` (`UpdateAuthority`, not
//! `Owner`) and confirmed not on either collection-plugin ban list
//! (`add_collection_plugin`'s `Groups`-only check, `create_collection`'s
//! `Edition`/`Groups`/owner-managed check) — a collection-level plugin here
//! is inherited by every member asset that doesn't set its own (verified in
//! `utils/mod.rs`'s `validate_asset_permissions`).

use crate::prelude::*;

#[cfg(feature = "pinocchio")]
use crate::plugin_registry::{find_plugin_offset, plugin_type};

/// One entry of `VerifiedCreators.signatures` — backend-neutral, since the
/// real `mpl-core` `VerifiedCreatorsSignature` type isn't available on
/// `pinocchio`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerifiedCreatorSignature {
    pub address: Address,
    pub verified: bool,
}

/// Attaches `VerifiedCreators` to an `Asset` via `add_plugin`.
pub fn attach_verified_creators_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    signatures: &[VerifiedCreatorSignature],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let plugin = ::mpl_core::types::Plugin::VerifiedCreators(::mpl_core::types::VerifiedCreators {
            signatures: signatures
                .iter()
                .map(|s| ::mpl_core::types::VerifiedCreatorsSignature {
                    address: s.address,
                    verified: s.verified,
                })
                .collect(),
        });
        add_asset_plugin_signed(program, accounts, plugin, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mut payload = crate::prelude::Vec::with_capacity(4 + signatures.len() * 33);
        payload.extend_from_slice(&(signatures.len() as u32).to_le_bytes());
        for sig in signatures {
            payload.extend_from_slice(sig.address.as_ref());
            payload.push(sig.verified as u8);
        }
        add_asset_plugin_signed_pinocchio(
            program,
            accounts,
            plugin_type::VERIFIED_CREATORS,
            &payload,
            signer_seeds,
        )
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
        let plugin = ::mpl_core::types::Plugin::VerifiedCreators(::mpl_core::types::VerifiedCreators {
            signatures: signatures
                .iter()
                .map(|s| ::mpl_core::types::VerifiedCreatorsSignature {
                    address: s.address,
                    verified: s.verified,
                })
                .collect(),
        });
        add_collection_plugin_signed(program, accounts, plugin, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mut payload = crate::prelude::Vec::with_capacity(4 + signatures.len() * 33);
        payload.extend_from_slice(&(signatures.len() as u32).to_le_bytes());
        for sig in signatures {
            payload.extend_from_slice(sig.address.as_ref());
            payload.push(sig.verified as u8);
        }
        add_collection_plugin_signed_pinocchio(
            program,
            accounts,
            plugin_type::VERIFIED_CREATORS,
            &payload,
            signer_seeds,
        )
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
    let asset = ::mpl_core::Asset::deserialize(&data)
        .map_err(|_| NaclacError::DeserializationFailed.err(0))?;
    Ok(asset.plugin_list.verified_creators.map(|p| {
        p.verified_creators
            .signatures
            .into_iter()
            .map(|s| VerifiedCreatorSignature {
                address: s.address,
                verified: s.verified,
            })
            .collect()
    }))
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
    let collection = ::mpl_core::Collection::deserialize(&data)
        .map_err(|_| NaclacError::DeserializationFailed.err(0))?;
    Ok(collection.plugin_list.verified_creators.map(|p| {
        p.verified_creators
            .signatures
            .into_iter()
            .map(|s| VerifiedCreatorSignature {
                address: s.address,
                verified: s.verified,
            })
            .collect()
    }))
}

/// Reads an `Asset`'s `VerifiedCreators` plugin, if attached. Callable
/// identically on both backends.
#[cfg(feature = "pinocchio")]
pub fn fetch_asset_verified_creators(
    info: &AccountInfo,
) -> Result<Option<crate::prelude::Vec<VerifiedCreatorSignature>>> {
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
) -> Result<Option<crate::prelude::Vec<VerifiedCreatorSignature>>> {
    let collection_view = crate::collection::CollectionView::try_from(info)?;
    let Some(plugin_header_offset) = collection_view.plugin_header_offset() else {
        return Ok(None);
    };
    read_verified_creators(info.data(), plugin_header_offset)
}

/// Lower-level variant of `fetch_asset_verified_creators`/
/// `fetch_collection_verified_creators`, `pinocchio`-only: see
/// `edition.rs`'s `read_edition` for why this exists alongside the
/// uniform-signature wrappers above.
#[cfg(feature = "pinocchio")]
pub fn read_verified_creators(
    data: &[u8],
    plugin_header_offset: usize,
) -> Result<Option<crate::prelude::Vec<VerifiedCreatorSignature>>> {
    let Some(offset) =
        find_plugin_offset(data, plugin_header_offset, plugin_type::VERIFIED_CREATORS)?
    else {
        return Ok(None);
    };
    let offset = offset as usize;

    if data.len() < offset + 4 {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let count = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
    let list_start = offset + 4;
    if data.len() < list_start + count * 33 {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let mut signatures = crate::prelude::Vec::with_capacity(count);
    for i in 0..count {
        let start = list_start + i * 33;
        let bytes: [u8; 32] = data[start..start + 32].try_into().unwrap();
        signatures.push(VerifiedCreatorSignature {
            address: Address::new_from_array(bytes),
            verified: data[start + 32] != 0,
        });
    }
    Ok(Some(signatures))
}
