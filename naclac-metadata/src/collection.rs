// ===========================================================================
// collection.rs — Metaplex Core `Collection` (`BaseCollectionV1`)
// ===========================================================================

//! Read access to a Metaplex Core `Collection` account's base fields, and
//! the `create_collection_v2` CPI that creates one. Mirrors `asset.rs`
//! exactly — see that file's header for why neither backend depends on the
//! real `mpl-core` crate. `PluginHeaderV1` placement (immediately after the
//! base struct's own encoding) is the same mechanism for `Collection` as
//! for `Asset` — verified against the real crate's `hooked/collection.rs`,
//! which uses the identical `PluginHeaderV1::from_bytes(&data[base_data.len()..])`
//! pattern.

use crate::prelude::*;

const KEY_COLLECTION_V1: u8 = 5;

/// Backend-uniform read access to a Metaplex Core `Collection` account's
/// base (non-plugin) fields.
pub trait CollectionLike {
    fn update_authority(&self) -> Address;
    fn name(&self) -> &str;
    fn uri(&self) -> &str;
    fn num_minted(&self) -> u32;
    fn current_size(&self) -> u32;
}

/// Owned, backend-uniform snapshot of a `Collection` account's base
/// (non-plugin) fields — what `fetch_collection` returns on both backends.
/// See `asset.rs`'s `AssetData` doc comment for why this is owned rather
/// than a borrowed `CollectionView<'_>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CollectionData {
    pub update_authority: Address,
    pub name: crate::prelude::String,
    pub uri: crate::prelude::String,
    pub num_minted: u32,
    pub current_size: u32,
}

/// Reads and fully deserializes a Metaplex Core `Collection` account's base
/// fields. Fails if the account isn't owned by the Core program, or isn't a
/// valid `CollectionV1`-keyed account.
#[cfg(not(feature = "pinocchio"))]
pub fn fetch_collection(info: &AccountInfo) -> Result<CollectionData> {
    if Owner::program_owner(info) != crate::ID {
        return Err(NaclacError::ConstraintOwner.into());
    }
    let solana_info = unsafe { info.to_lifetime() };
    let data = solana_info
        .try_borrow_data()
        .map_err(|_| NaclacError::AccountBorrowFailed.err(0))?;
    let view = CollectionView::from_bytes(&data)?;
    Ok(CollectionData {
        update_authority: view.update_authority(),
        name: view.name().into(),
        uri: view.uri().into(),
        num_minted: view.num_minted(),
        current_size: view.current_size(),
    })
}

/// Reads and fully deserializes a Metaplex Core `Collection` account's base
/// fields. Fails if the account isn't owned by the Core program, or isn't a
/// valid `CollectionV1`-keyed account.
#[cfg(feature = "pinocchio")]
pub fn fetch_collection(info: &AccountInfo) -> Result<CollectionData> {
    let view = CollectionView::try_from(info)?;
    Ok(CollectionData {
        update_authority: view.update_authority(),
        name: view.name().into(),
        uri: view.uri().into(),
        num_minted: view.num_minted(),
        current_size: view.current_size(),
    })
}

// ===========================================================================
// CollectionView — hand-rolled byte walk, shared by both backends
// ===========================================================================

/// Zero-copy, sequential-offset view into a Metaplex Core `Collection`
/// account's raw bytes. See this file's header, and `asset.rs`'s header,
/// for why this is hand-rolled rather than backed by the real `mpl-core`
/// crate.
#[derive(Clone, Copy)]
pub struct CollectionView<'a> {
    data: &'a [u8],
    name_offset: usize,
    name_len: usize,
    uri_offset: usize,
    uri_len: usize,
    num_minted_offset: usize,
    /// Byte offset immediately after this `Collection`'s own Borsh encoding
    /// — where `PluginHeaderV1` begins if this account has any plugins at
    /// all. See `asset.rs`'s `AssetView::base_encoding_end` for the
    /// verification this mirrors.
    base_encoding_end: usize,
}

impl<'a> CollectionView<'a> {
    /// `pinocchio`-only: see `AssetView::try_from`'s doc comment (`asset.rs`)
    /// for why the `solana` backend must go through `from_bytes` directly.
    #[cfg(feature = "pinocchio")]
    pub fn try_from(info: &'a AccountInfo) -> Result<Self> {
        if Owner::program_owner(info) != crate::ID {
            return Err(NaclacError::ConstraintOwner.into());
        }
        Self::from_bytes(info.data())
    }

    pub(crate) fn from_bytes(data: &'a [u8]) -> Result<Self> {
        // key(1) + update_authority(32)
        if data.len() < 33 {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        if data[0] != KEY_COLLECTION_V1 {
            return Err(NaclacError::InvalidAccountDiscriminator.err(0));
        }

        let name_offset = 33;
        let name_len = read_borsh_string_len(data, name_offset)?;
        let uri_offset = name_offset + 4 + name_len;
        let uri_len = read_borsh_string_len(data, uri_offset)?;
        let num_minted_offset = uri_offset + 4 + uri_len;

        // num_minted(4) + current_size(4)
        let base_encoding_end = num_minted_offset
            .checked_add(8)
            .ok_or_else(|| NaclacError::AccountDataTooSmall.err(0))?;
        if data.len() < base_encoding_end {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }

        Ok(Self {
            data,
            name_offset,
            name_len,
            uri_offset,
            uri_len,
            num_minted_offset,
            base_encoding_end,
        })
    }

    pub fn key(&self) -> u8 {
        self.data[0]
    }

    /// `None` if this account has no plugins at all (its data is exactly
    /// the size of the base `Collection` encoding); otherwise the byte
    /// offset where `PluginHeaderV1` begins.
    pub fn plugin_header_offset(&self) -> Option<usize> {
        if self.data.len() > self.base_encoding_end {
            Some(self.base_encoding_end)
        } else {
            None
        }
    }
}

impl<'a> CollectionLike for CollectionView<'a> {
    fn update_authority(&self) -> Address {
        read_pubkey(self.data, 1)
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

    fn num_minted(&self) -> u32 {
        u32::from_le_bytes(
            self.data[self.num_minted_offset..self.num_minted_offset + 4]
                .try_into()
                .unwrap(),
        )
    }

    fn current_size(&self) -> u32 {
        u32::from_le_bytes(
            self.data[self.num_minted_offset + 4..self.num_minted_offset + 8]
                .try_into()
                .unwrap(),
        )
    }
}

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
// create_collection_v2 CPI
// ===========================================================================

/// Accounts consumed by `create_collection_signed` — mirrors the real
/// `CreateCollectionV2` instruction exactly. `update_authority` left `None`
/// is filled with the Core program's own address, matching what
/// `mpl-core`'s real instruction builder does internally for the solana
/// backend.
/// Maximum `name`/`uri` byte length `create_collection_signed` accepts on
/// `pinocchio` — same reasoning and cap as `asset::MAX_ASSET_NAME_LEN`/
/// `asset::MAX_ASSET_URI_LEN`.
#[cfg(feature = "pinocchio")]
pub const MAX_COLLECTION_NAME_LEN: usize = 32;
#[cfg(feature = "pinocchio")]
pub const MAX_COLLECTION_URI_LEN: usize = 200;

pub struct CreateCollectionAccounts<'a> {
    pub collection: CpiHandleMut<'a>,
    pub update_authority: Option<CpiHandle<'a>>,
    pub payer: CpiHandleMut<'a>,
    pub system_program: CpiHandle<'a>,
}

/// Creates a new Metaplex Core `Collection` via a real `CreateCollectionV2`
/// CPI, with no initial plugins — attach plugins afterward (Tier 2).
pub fn create_collection_signed(
    program: CpiHandle<'_>,
    accounts: CreateCollectionAccounts<'_>,
    name: &str,
    uri: &str,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        // CreateCollectionV2 wire format verified against the real
        // `mpl-core` crate's own generated `instructions/create_collection_v2.rs`:
        // discriminator(1) + `CreateCollectionV2InstructionArgs { name:
        // String, uri: String, plugins: Option<Vec<..>>(1),
        // external_plugin_adapters: Option<Vec<..>>(1) }` — same shape as
        // `CreateV2` minus `data_state`. Always no initial plugins/adapters
        // here — same as the `pinocchio` branch below, which encodes this
        // identical layout by hand already.
        let mut data = crate::prelude::Vec::with_capacity(1 + 4 + name.len() + 4 + uri.len() + 2);
        data.push(21u8); // CreateCollectionV2 discriminator
        data.extend_from_slice(&(name.len() as u32).to_le_bytes());
        data.extend_from_slice(name.as_bytes());
        data.extend_from_slice(&(uri.len() as u32).to_le_bytes());
        data.extend_from_slice(uri.as_bytes());
        data.push(0u8); // plugins: None
        data.push(0u8); // external_plugin_adapters: None

        let accounts_meta = vec![
            solana_program::instruction::AccountMeta::new(accounts.collection.address(), true),
            match &accounts.update_authority {
                Some(u) => {
                    solana_program::instruction::AccountMeta::new_readonly(u.address(), false)
                }
                None => solana_program::instruction::AccountMeta::new_readonly(crate::ID, false),
            },
            solana_program::instruction::AccountMeta::new(accounts.payer.address(), true),
            solana_program::instruction::AccountMeta::new_readonly(
                accounts.system_program.address(),
                false,
            ),
        ];

        let ix = solana_program::instruction::Instruction {
            program_id: crate::ID,
            accounts: accounts_meta,
            data,
        };

        let cpi_accounts = [
            CpiHandle::from(accounts.collection),
            accounts
                .update_authority
                .unwrap_or_else(|| program.clone()),
            CpiHandle::from(accounts.payer),
            accounts.system_program,
            program,
        ];
        crate::cpi::invoke_signed(&ix, &cpi_accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let name_bytes = name.as_bytes();
        let uri_bytes = uri.as_bytes();
        if name_bytes.len() > MAX_COLLECTION_NAME_LEN || uri_bytes.len() > MAX_COLLECTION_URI_LEN {
            return Err(NaclacError::InvalidInstructionData.err(0));
        }
        let mut data = crate::fixed_buf::FixedBuf::<
            { 1 + 4 + MAX_COLLECTION_NAME_LEN + 4 + MAX_COLLECTION_URI_LEN + 2 },
        >::new();
        data.push(21u8); // CreateCollectionV2 discriminator
        data.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
        data.extend_from_slice(name_bytes);
        data.extend_from_slice(&(uri_bytes.len() as u32).to_le_bytes());
        data.extend_from_slice(uri_bytes);
        data.push(0u8); // plugins: None
        data.push(0u8); // external_plugin_adapters: None

        let collection_handle: CpiHandle<'_> = CpiHandle::from(accounts.collection);
        let payer_handle: CpiHandle<'_> = CpiHandle::from(accounts.payer);
        let update_authority_handle = accounts.update_authority.unwrap_or(program);

        let ix_accounts = [
            ::pinocchio::instruction::InstructionAccount::writable_signer(
                collection_handle.info.view.address(),
            ),
            ::pinocchio::instruction::InstructionAccount::readonly(
                update_authority_handle.info.view.address(),
            ),
            ::pinocchio::instruction::InstructionAccount::writable_signer(
                payer_handle.info.view.address(),
            ),
            ::pinocchio::instruction::InstructionAccount::readonly(
                accounts.system_program.info.view.address(),
            ),
        ];
        let instruction = ::pinocchio::instruction::InstructionView {
            program_id: program.info.view.address(),
            accounts: &ix_accounts,
            data: data.as_slice(),
        };
        let handles = [
            collection_handle,
            update_authority_handle,
            payer_handle,
            accounts.system_program,
        ];
        crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles, signer_seeds)
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Proves `CollectionView::from_bytes` never panics for any account
    /// bytes — including through `read_borsh_string_len`'s (now-
    /// `checked_end`-guarded) name/uri length prefixes and the
    /// `num_minted_offset + 8` arithmetic, both part of this audit's
    /// crate-wide overflow-panic fix (see docs/plan/kani-audit.md).
    #[kani::proof]
    #[kani::unwind(6)]
    fn prove_collection_view_from_bytes_never_panics() {
        let data: [u8; 48] = kani::any();
        let _ = CollectionView::from_bytes(&data);
    }
}
