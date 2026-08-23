// ===========================================================================
// plugins/royalties.rs — the `Royalties` plugin
// ===========================================================================

//! `Royalties` — attaches to an `Asset` or `Collection`, declaring a
//! basis-points royalty split across creators and an optional program
//! allow/deny list enforcing it. Real layout verified against `mpl-core`'s
//! `generated::types::{Royalties, Creator, RuleSet}`:
//! `Royalties { basis_points: u16, creators: Vec<Creator>, rule_set: RuleSet }`,
//! `Creator { address: Pubkey, percentage: u8 }` (fixed 33 bytes each — no
//! per-element offset walk needed, unlike `Attributes`), `RuleSet` (tag 1:
//! `None=0` / `ProgramAllowList=1(Vec<Pubkey>)` / `ProgramDenyList=2(Vec<Pubkey>)`,
//! each pubkey fixed-width too).

use crate::prelude::*;

#[cfg(feature = "pinocchio")]
use crate::plugin_registry::{find_plugin_offset, plugin_type};

/// One entry of `Royalties.creators` — backend-neutral, since the real
/// `mpl-core` `Creator` type isn't available on `pinocchio`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoyaltyCreator {
    pub address: Address,
    pub percentage: u8,
}

/// Backend-neutral mirror of the real `RuleSet` enum.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuleSetData {
    None,
    ProgramAllowList(crate::prelude::Vec<Address>),
    ProgramDenyList(crate::prelude::Vec<Address>),
}

/// Backend-neutral `Royalties` payload — `fetch_asset_royalties`/
/// `fetch_collection_royalties` return this on both backends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoyaltiesData {
    pub basis_points: u16,
    pub creators: crate::prelude::Vec<RoyaltyCreator>,
    pub rule_set: RuleSetData,
}

/// Argument form of `RuleSet` for `attach_royalties_signed` — borrows
/// rather than owns, since it only needs to live for the CPI call.
pub enum RuleSetArg<'a> {
    None,
    ProgramAllowList(&'a [Address]),
    ProgramDenyList(&'a [Address]),
}

/// Attaches `Royalties` to an `Asset` via `add_plugin`.
pub fn attach_royalties_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    basis_points: u16,
    creators: &[RoyaltyCreator],
    rule_set: RuleSetArg<'_>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let plugin = ::mpl_core::types::Plugin::Royalties(build_royalties(
            basis_points,
            creators,
            &rule_set,
        ));
        add_asset_plugin_signed(program, accounts, plugin, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let payload = encode_royalties_payload(basis_points, creators, &rule_set);
        add_asset_plugin_signed_pinocchio(
            program,
            accounts,
            plugin_type::ROYALTIES,
            &payload,
            signer_seeds,
        )
    }
}

/// Attaches `Royalties` to a `Collection` via `add_collection_plugin`.
pub fn attach_collection_royalties_signed(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    basis_points: u16,
    creators: &[RoyaltyCreator],
    rule_set: RuleSetArg<'_>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let plugin = ::mpl_core::types::Plugin::Royalties(build_royalties(
            basis_points,
            creators,
            &rule_set,
        ));
        add_collection_plugin_signed(program, accounts, plugin, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let payload = encode_royalties_payload(basis_points, creators, &rule_set);
        add_collection_plugin_signed_pinocchio(
            program,
            accounts,
            plugin_type::ROYALTIES,
            &payload,
            signer_seeds,
        )
    }
}

#[cfg(not(feature = "pinocchio"))]
fn build_royalties(
    basis_points: u16,
    creators: &[RoyaltyCreator],
    rule_set: &RuleSetArg<'_>,
) -> ::mpl_core::types::Royalties {
    ::mpl_core::types::Royalties {
        basis_points,
        creators: creators
            .iter()
            .map(|c| ::mpl_core::types::Creator {
                address: c.address,
                percentage: c.percentage,
            })
            .collect(),
        rule_set: match rule_set {
            RuleSetArg::None => ::mpl_core::types::RuleSet::None,
            RuleSetArg::ProgramAllowList(list) => {
                ::mpl_core::types::RuleSet::ProgramAllowList(list.to_vec())
            }
            RuleSetArg::ProgramDenyList(list) => {
                ::mpl_core::types::RuleSet::ProgramDenyList(list.to_vec())
            }
        },
    }
}

#[cfg(feature = "pinocchio")]
fn encode_royalties_payload(
    basis_points: u16,
    creators: &[RoyaltyCreator],
    rule_set: &RuleSetArg<'_>,
) -> crate::prelude::Vec<u8> {
    let mut data = crate::prelude::Vec::new();
    data.extend_from_slice(&basis_points.to_le_bytes());
    data.extend_from_slice(&(creators.len() as u32).to_le_bytes());
    for creator in creators {
        data.extend_from_slice(creator.address.as_ref());
        data.push(creator.percentage);
    }
    match rule_set {
        RuleSetArg::None => data.push(0u8),
        RuleSetArg::ProgramAllowList(list) => {
            data.push(1u8);
            data.extend_from_slice(&(list.len() as u32).to_le_bytes());
            for address in *list {
                data.extend_from_slice(address.as_ref());
            }
        }
        RuleSetArg::ProgramDenyList(list) => {
            data.push(2u8);
            data.extend_from_slice(&(list.len() as u32).to_le_bytes());
            for address in *list {
                data.extend_from_slice(address.as_ref());
            }
        }
    }
    data
}

/// Reads an `Asset`'s `Royalties` plugin, if attached. Callable identically
/// on both backends.
#[cfg(not(feature = "pinocchio"))]
pub fn fetch_asset_royalties(info: &AccountInfo) -> Result<Option<RoyaltiesData>> {
    if Owner::program_owner(info) != crate::ID {
        return Err(NaclacError::ConstraintOwner.into());
    }
    let solana_info = unsafe { info.to_lifetime() };
    let data = solana_info
        .try_borrow_data()
        .map_err(|_| NaclacError::AccountBorrowFailed.err(0))?;
    let asset = ::mpl_core::Asset::deserialize(&data)
        .map_err(|_| NaclacError::DeserializationFailed.err(0))?;
    Ok(asset.plugin_list.royalties.map(|p| from_real_royalties(p.royalties)))
}

/// Reads a `Collection`'s `Royalties` plugin, if attached. Callable
/// identically on both backends.
#[cfg(not(feature = "pinocchio"))]
pub fn fetch_collection_royalties(info: &AccountInfo) -> Result<Option<RoyaltiesData>> {
    if Owner::program_owner(info) != crate::ID {
        return Err(NaclacError::ConstraintOwner.into());
    }
    let solana_info = unsafe { info.to_lifetime() };
    let data = solana_info
        .try_borrow_data()
        .map_err(|_| NaclacError::AccountBorrowFailed.err(0))?;
    let collection = ::mpl_core::Collection::deserialize(&data)
        .map_err(|_| NaclacError::DeserializationFailed.err(0))?;
    Ok(collection
        .plugin_list
        .royalties
        .map(|p| from_real_royalties(p.royalties)))
}

/// Reads an `Asset`'s `Royalties` plugin, if attached. Callable identically
/// on both backends.
#[cfg(feature = "pinocchio")]
pub fn fetch_asset_royalties(info: &AccountInfo) -> Result<Option<RoyaltiesData>> {
    let asset_view = crate::asset::AssetView::try_from(info)?;
    let Some(plugin_header_offset) = asset_view.plugin_header_offset() else {
        return Ok(None);
    };
    read_royalties(info.data(), plugin_header_offset)
}

/// Reads a `Collection`'s `Royalties` plugin, if attached. Callable
/// identically on both backends.
#[cfg(feature = "pinocchio")]
pub fn fetch_collection_royalties(info: &AccountInfo) -> Result<Option<RoyaltiesData>> {
    let collection_view = crate::collection::CollectionView::try_from(info)?;
    let Some(plugin_header_offset) = collection_view.plugin_header_offset() else {
        return Ok(None);
    };
    read_royalties(info.data(), plugin_header_offset)
}

#[cfg(not(feature = "pinocchio"))]
fn from_real_royalties(r: ::mpl_core::types::Royalties) -> RoyaltiesData {
    RoyaltiesData {
        basis_points: r.basis_points,
        creators: r
            .creators
            .into_iter()
            .map(|c| RoyaltyCreator {
                address: c.address,
                percentage: c.percentage,
            })
            .collect(),
        rule_set: match r.rule_set {
            ::mpl_core::types::RuleSet::None => RuleSetData::None,
            ::mpl_core::types::RuleSet::ProgramAllowList(list) => {
                RuleSetData::ProgramAllowList(list)
            }
            ::mpl_core::types::RuleSet::ProgramDenyList(list) => {
                RuleSetData::ProgramDenyList(list)
            }
        },
    }
}

/// Lower-level variant of `fetch_asset_royalties`, `pinocchio`-only: see
/// `edition.rs`'s `read_edition` for why this exists alongside the
/// uniform-signature wrapper above.
#[cfg(feature = "pinocchio")]
pub fn read_royalties(data: &[u8], plugin_header_offset: usize) -> Result<Option<RoyaltiesData>> {
    let Some(offset) = find_plugin_offset(data, plugin_header_offset, plugin_type::ROYALTIES)?
    else {
        return Ok(None);
    };
    let offset = offset as usize;

    if data.len() < offset + 6 {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let basis_points = u16::from_le_bytes(data[offset..offset + 2].try_into().unwrap());
    let creator_count = u32::from_le_bytes(data[offset + 2..offset + 6].try_into().unwrap()) as usize;

    let creators_start = offset + 6;
    if data.len() < creators_start + creator_count * 33 {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let mut creators = crate::prelude::Vec::with_capacity(creator_count);
    for i in 0..creator_count {
        let start = creators_start + i * 33;
        let bytes: [u8; 32] = data[start..start + 32].try_into().unwrap();
        creators.push(RoyaltyCreator {
            address: Address::new_from_array(bytes),
            percentage: data[start + 32],
        });
    }

    let rule_set_offset = creators_start + creator_count * 33;
    if data.len() <= rule_set_offset {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let rule_set = match data[rule_set_offset] {
        0 => RuleSetData::None,
        tag @ (1 | 2) => {
            let count_offset = rule_set_offset + 1;
            if data.len() < count_offset + 4 {
                return Err(NaclacError::AccountDataTooSmall.err(0));
            }
            let count =
                u32::from_le_bytes(data[count_offset..count_offset + 4].try_into().unwrap())
                    as usize;
            let list_start = count_offset + 4;
            if data.len() < list_start + count * 32 {
                return Err(NaclacError::AccountDataTooSmall.err(0));
            }
            let mut list = crate::prelude::Vec::with_capacity(count);
            for i in 0..count {
                let start = list_start + i * 32;
                let bytes: [u8; 32] = data[start..start + 32].try_into().unwrap();
                list.push(Address::new_from_array(bytes));
            }
            if tag == 1 {
                RuleSetData::ProgramAllowList(list)
            } else {
                RuleSetData::ProgramDenyList(list)
            }
        }
        _ => return Err(NaclacError::InvalidInstructionData.err(0)),
    };

    Ok(Some(RoyaltiesData {
        basis_points,
        creators,
        rule_set,
    }))
}
