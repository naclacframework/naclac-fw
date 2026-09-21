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

use crate::plugin_registry::{find_plugin_offset, plugin_type};

/// One entry of `Royalties.creators` — backend-neutral, since the real
/// `mpl-core` `Creator` type isn't available on `pinocchio`. `#[repr(C)]`
/// over `Address`(32, align 1) + `u8`(1, align 1) produces zero padding,
/// matching the real on-chain Borsh layout exactly — so this is safely
/// `Span`-able for zero-copy reads on `pinocchio` (the `Pod`/`Zeroable`
/// derive itself is `pinocchio`-only since `Address` is only `Pod` there).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "pinocchio", repr(C))]
#[cfg_attr(feature = "pinocchio", derive(bytemuck::Pod, bytemuck::Zeroable))]
pub struct RoyaltyCreator {
    pub address: Address,
    pub percentage: u8,
}

/// `RoyaltiesData.creators`'s element list — a heap `Vec` on `solana`, a
/// zero-copy `Span` on `pinocchio` (no cap needed, reflects whatever's
/// actually on-chain). A type alias rather than a per-backend struct field
/// so `RoyaltiesData`/`RuleSetData` stay single, shared definitions instead
/// of being duplicated one-per-backend.
#[cfg(not(feature = "pinocchio"))]
pub type RoyaltyCreatorList = crate::prelude::Vec<RoyaltyCreator>;
#[cfg(feature = "pinocchio")]
pub type RoyaltyCreatorList = Span<RoyaltyCreator>;

/// `RuleSetData::ProgramAllowList`/`ProgramDenyList`'s element list — same
/// reasoning as `RoyaltyCreatorList`.
#[cfg(not(feature = "pinocchio"))]
pub type RuleSetAddressList = crate::prelude::Vec<Address>;
#[cfg(feature = "pinocchio")]
pub type RuleSetAddressList = Span<Address>;

/// Backend-neutral mirror of the real `RuleSet` enum.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuleSetData {
    None,
    ProgramAllowList(RuleSetAddressList),
    ProgramDenyList(RuleSetAddressList),
}

/// Backend-neutral `Royalties` payload — `fetch_asset_royalties`/
/// `fetch_collection_royalties` return this on both backends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoyaltiesData {
    pub basis_points: u16,
    pub creators: RoyaltyCreatorList,
    pub rule_set: RuleSetData,
}

/// Argument form of `RuleSet` for `attach_royalties_signed` — borrows
/// rather than owns, since it only needs to live for the CPI call.
pub enum RuleSetArg<'a> {
    None,
    ProgramAllowList(&'a [Address]),
    ProgramDenyList(&'a [Address]),
}

/// Maximum `creators` entries `attach_royalties_signed`/
/// `attach_collection_royalties_signed` accept on `pinocchio` — no real
/// protocol maximum exists, so this is a fixed cap sized for a stack buffer
/// rather than a heap `Vec`.
#[cfg(feature = "pinocchio")]
pub const MAX_ROYALTY_CREATORS: usize = 16;

/// Maximum `RuleSetArg::ProgramAllowList`/`ProgramDenyList` entries on
/// `pinocchio` — same reasoning as `MAX_ROYALTY_CREATORS`, and the same
/// shape (plain 32-byte addresses), so the same cap.
#[cfg(feature = "pinocchio")]
pub const MAX_RULE_SET_ADDRESSES: usize = 16;

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
        let mut data = crate::prelude::Vec::new();
        data.push(2u8); // AddPluginV1 discriminator
        data.push(plugin_type::ROYALTIES);
        encode_royalties_payload_owned(&mut data, basis_points, creators, &rule_set);
        data.push(0u8); // init_authority: None
        add_asset_plugin_signed(program, accounts, &data, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mut data = crate::fixed_buf::FixedBuf::<MAX_ROYALTIES_IX_LEN>::new();
        data.push(2u8); // AddPluginV1 discriminator
        data.push(plugin_type::ROYALTIES);
        encode_royalties_payload(&mut data, basis_points, creators, &rule_set)?;
        data.push(0u8); // init_authority: None
        add_asset_plugin_signed_pinocchio(program, accounts, data.as_slice(), signer_seeds)
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
        let mut data = crate::prelude::Vec::new();
        data.push(3u8); // AddCollectionPluginV1 discriminator
        data.push(plugin_type::ROYALTIES);
        encode_royalties_payload_owned(&mut data, basis_points, creators, &rule_set);
        data.push(0u8); // init_authority: None
        add_collection_plugin_signed(program, accounts, &data, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mut data = crate::fixed_buf::FixedBuf::<MAX_ROYALTIES_IX_LEN>::new();
        data.push(3u8); // AddCollectionPluginV1 discriminator
        data.push(plugin_type::ROYALTIES);
        encode_royalties_payload(&mut data, basis_points, creators, &rule_set)?;
        data.push(0u8); // init_authority: None
        add_collection_plugin_signed_pinocchio(program, accounts, data.as_slice(), signer_seeds)
    }
}

/// `solana`-only mirror of `encode_royalties_payload`: same wire layout,
/// written directly into a heap `Vec<u8>` instead of through the
/// `pinocchio`-only `ByteSink`/`FixedBuf` machinery (unavailable on this
/// backend), with no artificial cap on `creators`/rule-set address list
/// length (no real protocol maximum exists).
#[cfg(not(feature = "pinocchio"))]
fn encode_royalties_payload_owned(
    data: &mut crate::prelude::Vec<u8>,
    basis_points: u16,
    creators: &[RoyaltyCreator],
    rule_set: &RuleSetArg<'_>,
) {
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
}

/// Max total instruction data for `attach_royalties_signed`/
/// `attach_collection_royalties_signed`: discriminator(1) + `PluginType`
/// tag(1) + `basis_points`(2) + `creators` (4-byte count + up to
/// `MAX_ROYALTY_CREATORS` × 33 bytes each) + `rule_set` (1-byte tag + up to
/// a 4-byte count + `MAX_RULE_SET_ADDRESSES` × 32 bytes for the
/// allow/deny-list variants) + trailing `init_authority: None`(1).
#[cfg(feature = "pinocchio")]
const MAX_ROYALTIES_IX_LEN: usize = 1
    + 1
    + 2
    + (4 + MAX_ROYALTY_CREATORS * 33)
    + (1 + 4 + MAX_RULE_SET_ADDRESSES * 32)
    + 1;

/// Writes `Royalties`' Borsh-encoded payload (everything after the
/// `PluginType` tag) into `data`. Errors if `creators`/`rule_set`'s address
/// list exceeds this crate's fixed caps (`MAX_ROYALTY_CREATORS`/
/// `MAX_RULE_SET_ADDRESSES`) — no real protocol maximum exists.
#[cfg(feature = "pinocchio")]
fn encode_royalties_payload<S: crate::fixed_buf::ByteSink>(
    data: &mut S,
    basis_points: u16,
    creators: &[RoyaltyCreator],
    rule_set: &RuleSetArg<'_>,
) -> Result<()> {
    if creators.len() > MAX_ROYALTY_CREATORS {
        return Err(NaclacError::InvalidInstructionData.err(0));
    }
    data.extend_from_slice(&basis_points.to_le_bytes());
    data.extend_from_slice(&(creators.len() as u32).to_le_bytes());
    for creator in creators {
        data.extend_from_slice(creator.address.as_ref());
        data.push(creator.percentage);
    }
    match rule_set {
        RuleSetArg::None => data.push(0u8),
        RuleSetArg::ProgramAllowList(list) => {
            if list.len() > MAX_RULE_SET_ADDRESSES {
                return Err(NaclacError::InvalidInstructionData.err(0));
            }
            data.push(1u8);
            data.extend_from_slice(&(list.len() as u32).to_le_bytes());
            for address in *list {
                data.extend_from_slice(address.as_ref());
            }
        }
        RuleSetArg::ProgramDenyList(list) => {
            if list.len() > MAX_RULE_SET_ADDRESSES {
                return Err(NaclacError::InvalidInstructionData.err(0));
            }
            data.push(2u8);
            data.extend_from_slice(&(list.len() as u32).to_le_bytes());
            for address in *list {
                data.extend_from_slice(address.as_ref());
            }
        }
    }
    Ok(())
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
    let asset_view = crate::asset::AssetView::from_bytes(&data)?;
    let Some(plugin_header_offset) = asset_view.plugin_header_offset() else {
        return Ok(None);
    };
    read_royalties_owned(&data, plugin_header_offset)
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
    let collection_view = crate::collection::CollectionView::from_bytes(&data)?;
    let Some(plugin_header_offset) = collection_view.plugin_header_offset() else {
        return Ok(None);
    };
    read_royalties_owned(&data, plugin_header_offset)
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

/// `solana`-only: `RoyaltyCreator`/`Address` aren't `bytemuck::Pod` on this
/// backend (see `RoyaltyCreator`'s own doc comment), so `RoyaltyCreatorList`/
/// `RuleSetAddressList` resolve to `Vec`, not `Span`, here — this walks the
/// same layout `read_royalties` does, but collects owned `Vec`s instead of
/// constructing `Span`s.
#[cfg(not(feature = "pinocchio"))]
fn read_royalties_owned(data: &[u8], plugin_header_offset: usize) -> Result<Option<RoyaltiesData>> {
    let Some(offset) = find_plugin_offset(data, plugin_header_offset, plugin_type::ROYALTIES)?
    else {
        return Ok(None);
    };
    let offset = offset as usize;

    let creators_start = checked_end(offset, 6)?;
    if data.len() < creators_start {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let basis_points = u16::from_le_bytes(data[offset..offset + 2].try_into().unwrap());
    let creator_count = u32::from_le_bytes(data[offset + 2..creators_start].try_into().unwrap()) as usize;

    let rule_set_offset = creator_count
        .checked_mul(33)
        .and_then(|bytes| creators_start.checked_add(bytes))
        .ok_or_else(|| NaclacError::AccountDataTooSmall.err(0))?;
    if data.len() < rule_set_offset {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let mut creators = crate::prelude::Vec::with_capacity(creator_count);
    for i in 0..creator_count {
        let entry_start = creators_start + i * 33;
        let bytes: [u8; 32] = data[entry_start..entry_start + 32].try_into().unwrap();
        creators.push(RoyaltyCreator {
            address: Address::new_from_array(bytes),
            percentage: data[entry_start + 32],
        });
    }

    if data.len() <= rule_set_offset {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let rule_set = match data[rule_set_offset] {
        0 => RuleSetData::None,
        tag @ (1 | 2) => {
            let count_offset = checked_end(rule_set_offset, 1)?;
            let list_start = checked_end(count_offset, 4)?;
            if data.len() < list_start {
                return Err(NaclacError::AccountDataTooSmall.err(0));
            }
            let count =
                u32::from_le_bytes(data[count_offset..list_start].try_into().unwrap())
                    as usize;
            let list_end = count
                .checked_mul(32)
                .and_then(|bytes| list_start.checked_add(bytes))
                .ok_or_else(|| NaclacError::AccountDataTooSmall.err(0))?;
            if data.len() < list_end {
                return Err(NaclacError::AccountDataTooSmall.err(0));
            }
            let mut list = crate::prelude::Vec::with_capacity(count);
            for i in 0..count {
                let entry_start = list_start + i * 32;
                let bytes: [u8; 32] = data[entry_start..entry_start + 32].try_into().unwrap();
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

    let creators_start = checked_end(offset, 6)?;
    if data.len() < creators_start {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let basis_points = u16::from_le_bytes(data[offset..offset + 2].try_into().unwrap());
    let creator_count = u32::from_le_bytes(data[offset + 2..creators_start].try_into().unwrap()) as usize;

    let rule_set_offset = creator_count
        .checked_mul(33)
        .and_then(|bytes| creators_start.checked_add(bytes))
        .ok_or_else(|| NaclacError::AccountDataTooSmall.err(0))?;
    if data.len() < rule_set_offset {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let creators: Span<RoyaltyCreator> =
        Span::from_bytes(&data[creators_start..rule_set_offset])?;

    if data.len() <= rule_set_offset {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let rule_set = match data[rule_set_offset] {
        0 => RuleSetData::None,
        tag @ (1 | 2) => {
            let count_offset = checked_end(rule_set_offset, 1)?;
            let list_start = checked_end(count_offset, 4)?;
            if data.len() < list_start {
                return Err(NaclacError::AccountDataTooSmall.err(0));
            }
            let count =
                u32::from_le_bytes(data[count_offset..list_start].try_into().unwrap())
                    as usize;
            let list_end = count
                .checked_mul(32)
                .and_then(|bytes| list_start.checked_add(bytes))
                .ok_or_else(|| NaclacError::AccountDataTooSmall.err(0))?;
            if data.len() < list_end {
                return Err(NaclacError::AccountDataTooSmall.err(0));
            }
            let list: Span<Address> = Span::from_bytes(&data[list_start..list_end])?;
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

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Proves the non-pinocchio variant never panics end-to-end, including
    /// through `find_plugin_offset`, the creator-list count×stride
    /// arithmetic, and the nested rule-set list's own count×stride —
    /// deliberately small buffer/unwind bound given this function has two
    /// loops (creators + rule-set list), the same nested-loop shape that
    /// caused `external_plugin_registry`'s resource blowup earlier in this
    /// audit (see docs/plan/kani-audit.md).
    #[cfg(not(feature = "pinocchio"))]
    #[kani::proof]
    #[kani::unwind(12)]
    fn prove_read_royalties_owned_never_panics() {
        let data: [u8; 32] = kani::any();
        let plugin_header_offset: usize = kani::any();
        let _ = read_royalties_owned(&data, plugin_header_offset);
    }

    #[cfg(feature = "pinocchio")]
    #[kani::proof]
    #[kani::unwind(12)]
    fn prove_read_royalties_never_panics() {
        let data: [u8; 32] = kani::any();
        let plugin_header_offset: usize = kani::any();
        let _ = read_royalties(&data, plugin_header_offset);
    }
}
