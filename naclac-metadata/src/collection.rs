// ===========================================================================
// collection.rs — Metaplex Core `Collection` (`BaseCollectionV1`)
// ===========================================================================

//! Read access to a Metaplex Core `Collection` account's base fields, and
//! the `create_collection_v2` CPI that creates one. Mirrors `asset.rs`
//! exactly — see that file's header for the full solana/pinocchio design
//! rationale (real `mpl-core` crate vs. hand-rolled `no_std` walk, and the
//! verified `mpl-core`-`Pubkey`-equals-naclac-`Address` type identity).
//! `PluginHeaderV1` placement (immediately after the base struct's own
//! encoding) is the same mechanism for `Collection` as for `Asset` —
//! verified against the real crate's `hooked/collection.rs`, which uses the
//! identical `PluginHeaderV1::from_bytes(&data[base_data.len()..])` pattern.

use crate::prelude::*;

#[cfg(feature = "pinocchio")]
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

// ===========================================================================
// solana backend — wraps the real `mpl-core` crate directly
// ===========================================================================

#[cfg(not(feature = "pinocchio"))]
impl CollectionLike for ::mpl_core::accounts::BaseCollectionV1 {
    fn update_authority(&self) -> Address {
        self.update_authority
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn uri(&self) -> &str {
        &self.uri
    }
    fn num_minted(&self) -> u32 {
        self.num_minted
    }
    fn current_size(&self) -> u32 {
        self.current_size
    }
}

/// Reads and fully deserializes a Metaplex Core `Collection` account. Fails
/// if the account isn't owned by the Core program, or isn't a valid
/// `CollectionV1`-keyed account.
#[cfg(not(feature = "pinocchio"))]
pub fn fetch_collection(info: &AccountInfo) -> Result<::mpl_core::accounts::BaseCollectionV1> {
    if Owner::program_owner(info) != crate::ID {
        return Err(NaclacError::ConstraintOwner.into());
    }
    let solana_info = unsafe { info.to_lifetime() };
    ::mpl_core::accounts::BaseCollectionV1::try_from(&solana_info)
        .map_err(|_| NaclacError::DeserializationFailed.into())
}

// ===========================================================================
// pinocchio backend — hand-rolled, no_std, no `mpl-core` dependency
// ===========================================================================

/// Zero-copy, sequential-offset view into a Metaplex Core `Collection`
/// account's raw bytes. See this file's header, and `asset.rs`'s header,
/// for why this is hand-rolled rather than backed by the real `mpl-core`
/// crate.
#[cfg(feature = "pinocchio")]
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

#[cfg(feature = "pinocchio")]
impl<'a> CollectionView<'a> {
    pub fn try_from(info: &'a AccountInfo) -> Result<Self> {
        if Owner::program_owner(info) != crate::ID {
            return Err(NaclacError::ConstraintOwner.into());
        }
        Self::from_bytes(info.data())
    }

    fn from_bytes(data: &'a [u8]) -> Result<Self> {
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
        if data.len() < num_minted_offset + 8 {
            return Err(NaclacError::AccountDataTooSmall.err(0));
        }
        let base_encoding_end = num_minted_offset + 8;

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

#[cfg(feature = "pinocchio")]
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

#[cfg(feature = "pinocchio")]
fn read_borsh_string_len(data: &[u8], offset: usize) -> Result<usize> {
    if data.len() < offset + 4 {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let len = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
    if data.len() < offset + 4 + len {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    Ok(len)
}

#[cfg(feature = "pinocchio")]
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
        let ix = ::mpl_core::instructions::CreateCollectionV2 {
            collection: accounts.collection.address(),
            update_authority: accounts.update_authority.as_ref().map(|u| u.address()),
            payer: accounts.payer.address(),
            system_program: accounts.system_program.address(),
        }
        .instruction(::mpl_core::instructions::CreateCollectionV2InstructionArgs {
            name: name.to_string(),
            uri: uri.to_string(),
            plugins: None,
            external_plugin_adapters: None,
        });

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
        let mut data =
            crate::prelude::Vec::with_capacity(1 + 4 + name_bytes.len() + 4 + uri_bytes.len() + 2);
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
            data: &data,
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
