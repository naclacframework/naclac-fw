// ===========================================================================
// asset.rs — Metaplex Core `Asset` (`BaseAssetV1`)
// ===========================================================================

//! Read access to a Metaplex Core `Asset` account's base fields, and the
//! `create_v2` CPI that mints one — Tier 1 of `naclac-metadata`. Plugin data
//! (attributes, royalties, delegates, editions, ...) is Tier 2, read through
//! a separate plugin-registry walk starting at `AssetView::plugin_header_offset`,
//! not through this file.
//!
//! Neither backend depends on the real `mpl-core` crate: its `Cargo.toml`
//! declares `crate-type = ["cdylib", "lib"]` (both), which Solana's own docs
//! (`solana.com/docs/programs/limitations`) document as precluding dead-code
//! elimination for consumers — real, verified `cargo build-sbf` stack-frame
//! overflow errors resulted (`docs/07-solana-backend-stack-overflow-fix.md`),
//! not a hypothetical. `AssetView` hand-walks the real Borsh layout directly
//! off the account's raw bytes on **both** backends instead — necessarily a
//! sequential offset walk, not the fixed-stride `Span<T>`/`bytemuck::Pod`
//! shape naclac-token's TLV extensions use, since `name`/`uri` are
//! variable-length; per-field offsets are computed once in
//! `AssetView::try_from`/`from_bytes` and cached, so later accessor calls
//! are plain O(1) slot reads with no re-validation. CPI instruction bytes
//! (`CreateV2`'s discriminator + Borsh-encoded args) are likewise hand-built
//! on both backends, verified against the real `mpl-core` crate's own
//! generated `instructions/create_v2.rs` (cloned locally for reference at
//! `naclac-metadata/mpl-core/`, not a runtime dependency).

use crate::prelude::*;

const KEY_ASSET_V1: u8 = 1;

/// Backend-uniform view of an `Asset`'s `update_authority` field. The real
/// `mpl-core` enum carries an `Address`/`Collection` payload, but the
/// `pinocchio` backend can't depend on that type (see this file's header),
/// so both backends read into this shared shape instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateAuthorityKind {
    None,
    Address(Address),
    Collection(Address),
}

/// Backend-uniform read access to a Metaplex Core `Asset` account's base
/// (non-plugin) fields.
pub trait AssetLike {
    fn owner(&self) -> Address;
    fn update_authority(&self) -> UpdateAuthorityKind;
    fn name(&self) -> &str;
    fn uri(&self) -> &str;
    fn seq(&self) -> Option<u64>;
}

/// Owned, backend-uniform snapshot of an `Asset` account's base (non-plugin)
/// fields — what `fetch_asset` returns on both backends, built from
/// `AssetView` either way (`solana`'s real `AccountInfo` only hands out
/// `Ref`-guarded borrows via `try_borrow_data()`, so returning a borrowed
/// `AssetView<'_>` isn't possible there without extending its lifetime past
/// the guard; an owned snapshot sidesteps that entirely).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetData {
    pub owner: Address,
    pub update_authority: UpdateAuthorityKind,
    pub name: crate::prelude::String,
    pub uri: crate::prelude::String,
    pub seq: Option<u64>,
}

/// Reads and fully deserializes a Metaplex Core `Asset` account's base
/// fields. Fails if the account isn't owned by the Core program, or isn't a
/// valid `AssetV1`-keyed account.
#[cfg(not(feature = "pinocchio"))]
pub fn fetch_asset(info: &AccountInfo) -> Result<AssetData> {
    if Owner::program_owner(info) != crate::ID {
        return Err(NaclacError::ConstraintOwner.into());
    }
    let solana_info = unsafe { info.to_lifetime() };
    let data = solana_info
        .try_borrow_data()
        .map_err(|_| NaclacError::AccountBorrowFailed.err(0))?;
    let view = AssetView::from_bytes(&data)?;
    Ok(AssetData {
        owner: view.owner(),
        update_authority: view.update_authority(),
        name: view.name().into(),
        uri: view.uri().into(),
        seq: view.seq(),
    })
}

/// Reads and fully deserializes a Metaplex Core `Asset` account's base
/// fields. Fails if the account isn't owned by the Core program, or isn't a
/// valid `AssetV1`-keyed account.
#[cfg(feature = "pinocchio")]
pub fn fetch_asset(info: &AccountInfo) -> Result<AssetData> {
    let view = AssetView::try_from(info)?;
    Ok(AssetData {
        owner: view.owner(),
        update_authority: view.update_authority(),
        name: view.name().into(),
        uri: view.uri().into(),
        seq: view.seq(),
    })
}

// ===========================================================================
// AssetView — hand-rolled byte walk, shared by both backends
// ===========================================================================

/// Zero-copy, sequential-offset view into a Metaplex Core `Asset` account's
/// raw bytes. See this file's header for why this is hand-rolled rather
/// than backed by the real `mpl-core` crate.
#[derive(Clone, Copy)]
pub struct AssetView<'a> {
    data: &'a [u8],
    update_authority_tag: u8,
    /// Byte offset of `update_authority`'s 32-byte pubkey payload.
    /// Meaningless when `update_authority_tag == 0`.
    update_authority_pubkey_offset: usize,
    name_offset: usize,
    name_len: usize,
    uri_offset: usize,
    uri_len: usize,
    seq_tag_offset: usize,
    /// Byte offset immediately after this `Asset`'s own Borsh encoding —
    /// where `PluginHeaderV1` begins if this account has any plugins at
    /// all. Verified against the real crate's own `Asset::deserialize`
    /// (`hooked/asset.rs`): `PluginHeaderV1::from_bytes(&data[base_data.len()..])`,
    /// where `base_data` is the base struct re-serialized to measure its
    /// own encoded length — i.e. the plugin header, if present, starts
    /// exactly where the base struct's encoding ends, with no gap.
    base_encoding_end: usize,
}

impl<'a> AssetView<'a> {
    /// `pinocchio`-only: `AccountInfo::data()` (a plain, un-guarded `&[u8]`)
    /// only exists on that backend. On `solana`, callers must go through
    /// `try_borrow_data()` themselves (a `Ref`-guarded borrow) and construct
    /// via `from_bytes` directly instead — see e.g. `plugins/attributes.rs`'s
    /// `fetch_asset_attributes`.
    #[cfg(feature = "pinocchio")]
    pub fn try_from(info: &'a AccountInfo) -> Result<Self> {
        if Owner::program_owner(info) != crate::ID {
            return Err(NaclacError::ConstraintOwner.into());
        }
        Self::from_bytes(info.data())
    }

    pub(crate) fn from_bytes(data: &'a [u8]) -> Result<Self> {
        // key(1) + owner(32) + update_authority tag(1)
        if data.len() < 34 {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        if data[0] != KEY_ASSET_V1 {
            return Err(NaclacError::InvalidAccountDiscriminator.err(0));
        }

        let update_authority_tag = data[33];
        let (update_authority_pubkey_offset, mut offset) = match update_authority_tag {
            0 => (0usize, 34usize),
            1 | 2 => {
                if data.len() < 66 {
                    return Err(NaclacError::AccountDataTooSmall.err(0));
                }
                (34usize, 66usize)
            }
            _ => return Err(NaclacError::InvalidInstructionData.err(0)),
        };

        let name_offset = offset;
        let name_len = read_borsh_string_len(data, name_offset)?;
        offset = name_offset + 4 + name_len;

        let uri_offset = offset;
        let uri_len = read_borsh_string_len(data, uri_offset)?;
        offset = uri_offset + 4 + uri_len;

        let seq_tag_offset = offset;
        if data.len() <= seq_tag_offset {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        let base_encoding_end = match data[seq_tag_offset] {
            0 => seq_tag_offset + 1,
            1 => {
                let end = seq_tag_offset
                    .checked_add(9)
                    .ok_or_else(|| NaclacError::AccountDataTooSmall.err(0))?;
                if data.len() < end {
                    return Err(NaclacError::AccountDataTooSmall.err(0));
                }
                end
            }
            _ => return Err(NaclacError::InvalidInstructionData.err(0)),
        };

        Ok(Self {
            data,
            update_authority_tag,
            update_authority_pubkey_offset,
            name_offset,
            name_len,
            uri_offset,
            uri_len,
            seq_tag_offset,
            base_encoding_end,
        })
    }

    pub fn key(&self) -> u8 {
        self.data[0]
    }

    /// `None` if this account has no plugins at all (its data is exactly
    /// the size of the base `Asset` encoding); otherwise the byte offset
    /// where `PluginHeaderV1` begins.
    pub fn plugin_header_offset(&self) -> Option<usize> {
        if self.data.len() > self.base_encoding_end {
            Some(self.base_encoding_end)
        } else {
            None
        }
    }
}

impl<'a> AssetLike for AssetView<'a> {
    fn owner(&self) -> Address {
        read_pubkey(self.data, 1)
    }

    fn update_authority(&self) -> UpdateAuthorityKind {
        match self.update_authority_tag {
            0 => UpdateAuthorityKind::None,
            1 => {
                UpdateAuthorityKind::Address(read_pubkey(self.data, self.update_authority_pubkey_offset))
            }
            _ => UpdateAuthorityKind::Collection(read_pubkey(
                self.data,
                self.update_authority_pubkey_offset,
            )),
        }
    }

    fn name(&self) -> &str {
        // SAFETY: bounds were validated in `from_bytes`; Core's own write
        // path always encodes `name` as a valid Borsh (UTF-8) `String`.
        unsafe {
            core::str::from_utf8_unchecked(
                &self.data[self.name_offset + 4..self.name_offset + 4 + self.name_len],
            )
        }
    }

    fn uri(&self) -> &str {
        // SAFETY: same as `name`.
        unsafe {
            core::str::from_utf8_unchecked(
                &self.data[self.uri_offset + 4..self.uri_offset + 4 + self.uri_len],
            )
        }
    }

    fn seq(&self) -> Option<u64> {
        if self.data[self.seq_tag_offset] == 0 {
            return None;
        }
        let start = self.seq_tag_offset + 1;
        Some(u64::from_le_bytes(
            self.data[start..start + 8].try_into().unwrap(),
        ))
    }
}

/// Reads a Borsh `String`'s 4-byte little-endian length prefix at `offset`
/// and validates the full string (prefix + payload) fits within `data`.
/// Shared by every variable-length-string field this crate walks by hand
/// (`Asset`/`Collection` `name`/`uri`, and later plugin payloads).
fn read_borsh_string_len(data: &[u8], offset: usize) -> Result<usize> {
    let prefix_end = offset
        .checked_add(4)
        .ok_or_else(|| NaclacError::AccountDataTooSmall.err(0))?;
    if data.len() < prefix_end {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let len = u32::from_le_bytes(data[offset..prefix_end].try_into().unwrap()) as usize;
    let payload_end = prefix_end
        .checked_add(len)
        .ok_or_else(|| NaclacError::AccountDataTooSmall.err(0))?;
    if data.len() < payload_end {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    Ok(len)
}

fn read_pubkey(data: &[u8], offset: usize) -> Address {
    let bytes: [u8; 32] = data[offset..offset + 32].try_into().unwrap();
    Address::new_from_array(bytes)
}

// ===========================================================================
// create_v2 CPI — shared accounts struct, per-backend body
// ===========================================================================

/// Accounts consumed by `create_asset_signed` — mirrors the real `CreateV2`
/// instruction exactly. Every optional field left `None` is filled with the
/// Core program's own address (matching what `mpl-core`'s real instruction
/// builder does internally for the solana backend), so both backends
/// substitute `program`'s own handle into that slot rather than omitting it.
/// Maximum `name`/`uri` byte length `create_asset_signed` accepts on
/// `pinocchio` — no real protocol maximum exists (verified: `mpl-core`'s
/// processor enforces none), so this is a fixed cap sized for a stack
/// buffer rather than a heap `Vec`.
#[cfg(feature = "pinocchio")]
pub const MAX_ASSET_NAME_LEN: usize = 32;
#[cfg(feature = "pinocchio")]
pub const MAX_ASSET_URI_LEN: usize = 200;

pub struct CreateAssetAccounts<'a> {
    pub asset: CpiHandleMut<'a>,
    pub collection: Option<CpiHandleMut<'a>>,
    pub authority: Option<CpiHandle<'a>>,
    pub payer: CpiHandleMut<'a>,
    pub owner: Option<CpiHandle<'a>>,
    pub update_authority: Option<CpiHandle<'a>>,
    pub system_program: CpiHandle<'a>,
    pub log_wrapper: Option<CpiHandle<'a>>,
}

/// Mints a new Metaplex Core `Asset` via a real `CreateV2` CPI, in
/// `DataState::AccountState` with no initial plugins — attach plugins
/// afterward (Tier 2).
pub fn create_asset_signed(
    program: CpiHandle<'_>,
    accounts: CreateAssetAccounts<'_>,
    name: &str,
    uri: &str,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        // CreateV2 wire format verified against the real `mpl-core` crate's
        // own generated `instructions/create_v2.rs`: discriminator(1) +
        // `CreateV2InstructionArgs { data_state: DataState(1), name: String,
        // uri: String, plugins: Option<Vec<..>>(1), external_plugin_adapters:
        // Option<Vec<..>>(1) }`. Always `DataState::AccountState`(0) with no
        // initial plugins/adapters here — same as the `pinocchio` branch
        // below, which encodes this identical layout by hand already.
        let mut data =
            crate::prelude::Vec::with_capacity(2 + 4 + name.len() + 4 + uri.len() + 2);
        data.push(20u8); // CreateV2 discriminator
        data.push(0u8); // DataState::AccountState
        data.extend_from_slice(&(name.len() as u32).to_le_bytes());
        data.extend_from_slice(name.as_bytes());
        data.extend_from_slice(&(uri.len() as u32).to_le_bytes());
        data.extend_from_slice(uri.as_bytes());
        data.push(0u8); // plugins: None
        data.push(0u8); // external_plugin_adapters: None

        let accounts_meta = vec![
            solana_program::instruction::AccountMeta::new(accounts.asset.address(), true),
            match &accounts.collection {
                Some(c) => solana_program::instruction::AccountMeta::new(c.address(), false),
                None => solana_program::instruction::AccountMeta::new_readonly(crate::ID, false),
            },
            match &accounts.authority {
                Some(a) => {
                    solana_program::instruction::AccountMeta::new_readonly(a.address(), true)
                }
                None => solana_program::instruction::AccountMeta::new_readonly(crate::ID, false),
            },
            solana_program::instruction::AccountMeta::new(accounts.payer.address(), true),
            match &accounts.owner {
                Some(o) => {
                    solana_program::instruction::AccountMeta::new_readonly(o.address(), false)
                }
                None => solana_program::instruction::AccountMeta::new_readonly(crate::ID, false),
            },
            match &accounts.update_authority {
                Some(u) => {
                    solana_program::instruction::AccountMeta::new_readonly(u.address(), false)
                }
                None => solana_program::instruction::AccountMeta::new_readonly(crate::ID, false),
            },
            solana_program::instruction::AccountMeta::new_readonly(
                accounts.system_program.address(),
                false,
            ),
            match &accounts.log_wrapper {
                Some(l) => {
                    solana_program::instruction::AccountMeta::new_readonly(l.address(), false)
                }
                None => solana_program::instruction::AccountMeta::new_readonly(crate::ID, false),
            },
        ];

        let ix = solana_program::instruction::Instruction {
            program_id: crate::ID,
            accounts: accounts_meta,
            data,
        };

        let cpi_accounts = [
            CpiHandle::from(accounts.asset),
            accounts
                .collection
                .map(CpiHandle::from)
                .unwrap_or_else(|| program.clone()),
            accounts.authority.unwrap_or_else(|| program.clone()),
            CpiHandle::from(accounts.payer),
            accounts.owner.unwrap_or_else(|| program.clone()),
            accounts
                .update_authority
                .unwrap_or_else(|| program.clone()),
            accounts.system_program,
            accounts.log_wrapper.unwrap_or_else(|| program.clone()),
            program,
        ];
        crate::cpi::invoke_signed(&ix, &cpi_accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let name_bytes = name.as_bytes();
        let uri_bytes = uri.as_bytes();
        if name_bytes.len() > MAX_ASSET_NAME_LEN || uri_bytes.len() > MAX_ASSET_URI_LEN {
            return Err(NaclacError::InvalidInstructionData.err(0));
        }
        let mut data = crate::fixed_buf::FixedBuf::<
            { 2 + 4 + MAX_ASSET_NAME_LEN + 4 + MAX_ASSET_URI_LEN + 2 },
        >::new();
        data.push(20u8); // CreateV2 discriminator
        data.push(0u8); // DataState::AccountState
        data.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
        data.extend_from_slice(name_bytes);
        data.extend_from_slice(&(uri_bytes.len() as u32).to_le_bytes());
        data.extend_from_slice(uri_bytes);
        data.push(0u8); // plugins: None
        data.push(0u8); // external_plugin_adapters: None

        let collection_is_some = accounts.collection.is_some();
        let authority_is_some = accounts.authority.is_some();

        let asset_handle: CpiHandle<'_> = CpiHandle::from(accounts.asset);
        let payer_handle: CpiHandle<'_> = CpiHandle::from(accounts.payer);
        let collection_handle = accounts.collection.map(CpiHandle::from).unwrap_or(program);
        let authority_handle = accounts.authority.unwrap_or(program);
        let owner_handle = accounts.owner.unwrap_or(program);
        let update_authority_handle = accounts.update_authority.unwrap_or(program);
        let log_wrapper_handle = accounts.log_wrapper.unwrap_or(program);

        let ix_accounts = [
            ::pinocchio::instruction::InstructionAccount::writable_signer(
                asset_handle.info.view.address(),
            ),
            if collection_is_some {
                ::pinocchio::instruction::InstructionAccount::writable(
                    collection_handle.info.view.address(),
                )
            } else {
                ::pinocchio::instruction::InstructionAccount::readonly(
                    collection_handle.info.view.address(),
                )
            },
            if authority_is_some {
                ::pinocchio::instruction::InstructionAccount::readonly_signer(
                    authority_handle.info.view.address(),
                )
            } else {
                ::pinocchio::instruction::InstructionAccount::readonly(
                    authority_handle.info.view.address(),
                )
            },
            ::pinocchio::instruction::InstructionAccount::writable_signer(
                payer_handle.info.view.address(),
            ),
            ::pinocchio::instruction::InstructionAccount::readonly(owner_handle.info.view.address()),
            ::pinocchio::instruction::InstructionAccount::readonly(
                update_authority_handle.info.view.address(),
            ),
            ::pinocchio::instruction::InstructionAccount::readonly(
                accounts.system_program.info.view.address(),
            ),
            ::pinocchio::instruction::InstructionAccount::readonly(
                log_wrapper_handle.info.view.address(),
            ),
        ];
        let instruction = ::pinocchio::instruction::InstructionView {
            program_id: program.info.view.address(),
            accounts: &ix_accounts,
            data: data.as_slice(),
        };
        let handles = [
            asset_handle,
            collection_handle,
            authority_handle,
            payer_handle,
            owner_handle,
            update_authority_handle,
            accounts.system_program,
            log_wrapper_handle,
        ];
        crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles, signer_seeds)
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Proves `AssetView::from_bytes` never panics for any account bytes —
    /// including through `read_borsh_string_len`'s (now-`checked_end`-
    /// guarded) name/uri length prefixes and the `seq_tag_offset + 9`
    /// arithmetic, both part of this audit's crate-wide overflow-panic fix
    /// (see docs/plan/kani-audit.md).
    #[kani::proof]
    #[kani::unwind(6)]
    fn prove_asset_view_from_bytes_never_panics() {
        let data: [u8; 48] = kani::any();
        let _ = AssetView::from_bytes(&data);
    }
}
