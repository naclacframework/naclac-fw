// ===========================================================================
// group.rs — the real `GroupV1` account (taxonomy groups)
// ===========================================================================

//! Read access to a real `GroupV1` account (a taxonomy node that can
//! reference collections, other groups, and assets — separate from the
//! `Groups` *plugin*, `plugins/groups.rs`, which just lists an asset/
//! collection's own group memberships), plus `CreateGroupV1`/
//! `CloseGroupV1`/`UpdateGroupV1`. Real layout verified against `mpl-core`'s
//! `state::GroupV1` and `generated::accounts::GroupV1` (identical shape):
//! `key(1) + update_authority: Pubkey(32) + name: String(4+len) +
//! uri: String(4+len) + collections: Vec<Pubkey>(4+32N) +
//! groups: Vec<Pubkey>(4+32N) + parent_groups: Vec<Pubkey>(4+32N) +
//! assets: Vec<Pubkey>(4+32N)` — every `Vec` here holds fixed-width
//! 32-byte pubkeys, so (unlike `Asset`/`Collection`'s plugin data) no
//! variable-per-element walk is needed, just four sequential length-prefixed
//! lists. `Key::GroupV1 = 6`.
//!
//! `CreateGroupV1`'s `relationships` arg (seeding the group with initial
//! members at creation) is **not supported** here — `create_group_signed`
//! always creates an empty group. Verified this is a real, lossless subset:
//! seeding relationships at creation is equivalent to creating empty then
//! calling the dedicated `Add*ToGroupV1` instructions (Phase 4's remaining
//! work) afterward, just as one extra transaction instead of zero. Revisit
//! if a concrete need for single-transaction seeding shows up.

use crate::prelude::*;

#[cfg(feature = "pinocchio")]
const KEY_GROUP_V1: u8 = 6;

// ===========================================================================
// solana backend
// ===========================================================================

/// Reads and fully deserializes a `GroupV1` account.
#[cfg(not(feature = "pinocchio"))]
pub fn fetch_group(info: &AccountInfo) -> Result<::mpl_core::accounts::GroupV1> {
    if Owner::program_owner(info) != crate::ID {
        return Err(NaclacError::ConstraintOwner.into());
    }
    let solana_info = unsafe { info.to_lifetime() };
    ::mpl_core::accounts::GroupV1::try_from(&solana_info)
        .map_err(|_| NaclacError::DeserializationFailed.into())
}

// ===========================================================================
// pinocchio backend — hand-rolled, no_std, no `mpl-core` dependency
// ===========================================================================

/// Zero-copy, sequential-offset view into a `GroupV1` account's raw bytes.
/// See `asset.rs`'s header for why this is hand-rolled rather than backed
/// by the real `mpl-core` crate.
#[cfg(feature = "pinocchio")]
#[derive(Clone, Copy)]
pub struct GroupView<'a> {
    data: &'a [u8],
    name_offset: usize,
    name_len: usize,
    uri_offset: usize,
    uri_len: usize,
    collections_offset: usize,
    groups_offset: usize,
    parent_groups_offset: usize,
    assets_offset: usize,
}

#[cfg(feature = "pinocchio")]
impl<'a> GroupView<'a> {
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
        if data[0] != KEY_GROUP_V1 {
            return Err(NaclacError::InvalidAccountDiscriminator.err(0));
        }

        let name_offset = 33;
        let name_len = read_borsh_string_len(data, name_offset)?;
        let uri_offset = name_offset + 4 + name_len;
        let uri_len = read_borsh_string_len(data, uri_offset)?;
        let collections_offset = uri_offset + 4 + uri_len;
        let groups_offset = skip_pubkey_vec(data, collections_offset)?;
        let parent_groups_offset = skip_pubkey_vec(data, groups_offset)?;
        let assets_offset = skip_pubkey_vec(data, parent_groups_offset)?;
        skip_pubkey_vec(data, assets_offset)?; // validates the last list fits

        Ok(Self {
            data,
            name_offset,
            name_len,
            uri_offset,
            uri_len,
            collections_offset,
            groups_offset,
            parent_groups_offset,
            assets_offset,
        })
    }

    pub fn key(&self) -> u8 {
        self.data[0]
    }

    pub fn update_authority(&self) -> Address {
        let bytes: [u8; 32] = self.data[1..33].try_into().unwrap();
        Address::new_from_array(bytes)
    }

    pub fn name(&self) -> &'a str {
        // SAFETY: bounds validated in `from_bytes`; Core's own write path
        // always encodes `name` as a valid Borsh (UTF-8) `String`.
        unsafe {
            core::str::from_utf8_unchecked(
                &self.data[self.name_offset + 4..self.name_offset + 4 + self.name_len],
            )
        }
    }

    pub fn uri(&self) -> &'a str {
        // SAFETY: same as `name`.
        unsafe {
            core::str::from_utf8_unchecked(
                &self.data[self.uri_offset + 4..self.uri_offset + 4 + self.uri_len],
            )
        }
    }

    pub fn collections(&self) -> crate::prelude::Vec<Address> {
        read_pubkey_vec(self.data, self.collections_offset)
    }

    pub fn groups(&self) -> crate::prelude::Vec<Address> {
        read_pubkey_vec(self.data, self.groups_offset)
    }

    pub fn parent_groups(&self) -> crate::prelude::Vec<Address> {
        read_pubkey_vec(self.data, self.parent_groups_offset)
    }

    pub fn assets(&self) -> crate::prelude::Vec<Address> {
        read_pubkey_vec(self.data, self.assets_offset)
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

/// Validates a `Vec<Pubkey>` at `offset` fits within `data` and returns the
/// offset immediately after it (where the next field starts).
#[cfg(feature = "pinocchio")]
fn skip_pubkey_vec(data: &[u8], offset: usize) -> Result<usize> {
    if data.len() < offset + 4 {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let count = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
    let end = offset + 4 + count * 32;
    if data.len() < end {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    Ok(end)
}

#[cfg(feature = "pinocchio")]
fn read_pubkey_vec(data: &[u8], offset: usize) -> crate::prelude::Vec<Address> {
    let count = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
    let mut list = crate::prelude::Vec::with_capacity(count);
    for i in 0..count {
        let start = offset + 4 + i * 32;
        let bytes: [u8; 32] = data[start..start + 32].try_into().unwrap();
        list.push(Address::new_from_array(bytes));
    }
    list
}

// ===========================================================================
// create_group / close_group / update_group
// ===========================================================================

/// Accounts consumed by `create_group_signed` — mirrors the real
/// `CreateGroupV1` instruction exactly. Note `update_authority` must be a
/// **signer** when `Some` here (verified: `AccountMeta::new_readonly(update_authority,
/// true)`) — unlike every other optional-authority-reference field
/// elsewhere in this crate, which are never required to co-sign.
pub struct CreateGroupAccounts<'a> {
    pub group: CpiHandleMut<'a>,
    pub update_authority: Option<CpiHandle<'a>>,
    pub payer: CpiHandleMut<'a>,
    pub system_program: CpiHandle<'a>,
}

/// Accounts consumed by `close_group_signed` — mirrors the real
/// `CloseGroupV1` instruction exactly. No `system_program` account here
/// (unlike almost everything else in this crate) — verified, not assumed.
pub struct CloseGroupAccounts<'a> {
    pub group: CpiHandleMut<'a>,
    pub payer: CpiHandleMut<'a>,
    pub authority: Option<CpiHandle<'a>>,
}

/// Accounts consumed by `update_group_signed` — mirrors the real
/// `UpdateGroupV1` instruction exactly. `new_update_authority` is an
/// *account* (like `UpdateCollectionAccounts`'s), not an arg.
pub struct UpdateGroupAccounts<'a> {
    pub group: CpiHandleMut<'a>,
    pub payer: CpiHandleMut<'a>,
    pub authority: Option<CpiHandle<'a>>,
    pub new_update_authority: Option<CpiHandle<'a>>,
    pub system_program: CpiHandle<'a>,
}

/// Creates a new, empty `GroupV1` via a real `CreateGroupV1` CPI. See this
/// file's header for why "empty" (no initial `relationships`).
pub fn create_group_signed(
    program: CpiHandle<'_>,
    accounts: CreateGroupAccounts<'_>,
    name: &str,
    uri: &str,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = ::mpl_core::instructions::CreateGroupV1 {
            group: accounts.group.address(),
            update_authority: accounts.update_authority.as_ref().map(|a| a.address()),
            payer: accounts.payer.address(),
            system_program: accounts.system_program.address(),
        }
        .instruction(::mpl_core::instructions::CreateGroupV1InstructionArgs {
            name: name.to_string(),
            uri: uri.to_string(),
            relationships: crate::prelude::Vec::new(),
        });

        let cpi_accounts = [
            CpiHandle::from(accounts.group),
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
        let mut data = crate::prelude::Vec::with_capacity(9 + name.len() + uri.len());
        data.push(39u8); // CreateGroupV1 discriminator
        data.extend_from_slice(&(name.len() as u32).to_le_bytes());
        data.extend_from_slice(name.as_bytes());
        data.extend_from_slice(&(uri.len() as u32).to_le_bytes());
        data.extend_from_slice(uri.as_bytes());
        data.extend_from_slice(&0u32.to_le_bytes()); // relationships: empty Vec

        let group_handle: CpiHandle<'_> = CpiHandle::from(accounts.group);
        let payer_handle: CpiHandle<'_> = CpiHandle::from(accounts.payer);
        let update_authority_is_some = accounts.update_authority.is_some();
        let update_authority_handle = accounts.update_authority.unwrap_or(program);

        let ix_accounts = [
            ::pinocchio::instruction::InstructionAccount::writable_signer(
                group_handle.info.view.address(),
            ),
            if update_authority_is_some {
                ::pinocchio::instruction::InstructionAccount::readonly_signer(
                    update_authority_handle.info.view.address(),
                )
            } else {
                ::pinocchio::instruction::InstructionAccount::readonly(
                    update_authority_handle.info.view.address(),
                )
            },
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
            group_handle,
            update_authority_handle,
            payer_handle,
            accounts.system_program,
        ];
        crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles, signer_seeds)
    }
}

/// Closes a `GroupV1` via a real `CloseGroupV1` CPI, returning its lamports
/// to `payer`.
pub fn close_group_signed(
    program: CpiHandle<'_>,
    accounts: CloseGroupAccounts<'_>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = ::mpl_core::instructions::CloseGroupV1 {
            group: accounts.group.address(),
            payer: accounts.payer.address(),
            authority: accounts.authority.as_ref().map(|a| a.address()),
        }
        .instruction();

        let cpi_accounts = [
            CpiHandle::from(accounts.group),
            CpiHandle::from(accounts.payer),
            accounts.authority.unwrap_or_else(|| program.clone()),
            program,
        ];
        crate::cpi::invoke_signed(&ix, &cpi_accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let data = [40u8]; // CloseGroupV1 discriminator, no args

        let group_handle: CpiHandle<'_> = CpiHandle::from(accounts.group);
        let payer_handle: CpiHandle<'_> = CpiHandle::from(accounts.payer);
        let authority_is_some = accounts.authority.is_some();
        let authority_handle = accounts.authority.unwrap_or(program);

        let ix_accounts = [
            ::pinocchio::instruction::InstructionAccount::writable(
                group_handle.info.view.address(),
            ),
            ::pinocchio::instruction::InstructionAccount::writable_signer(
                payer_handle.info.view.address(),
            ),
            if authority_is_some {
                ::pinocchio::instruction::InstructionAccount::readonly_signer(
                    authority_handle.info.view.address(),
                )
            } else {
                ::pinocchio::instruction::InstructionAccount::readonly(
                    authority_handle.info.view.address(),
                )
            },
        ];
        let instruction = ::pinocchio::instruction::InstructionView {
            program_id: program.info.view.address(),
            accounts: &ix_accounts,
            data: &data,
        };
        let handles = [group_handle, payer_handle, authority_handle];
        crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles, signer_seeds)
    }
}

/// Updates a `GroupV1`'s `name`/`uri`/`update_authority` via a real
/// `UpdateGroupV1` CPI. Any argument left `None` is left unchanged.
pub fn update_group_signed(
    program: CpiHandle<'_>,
    accounts: UpdateGroupAccounts<'_>,
    new_name: Option<&str>,
    new_uri: Option<&str>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = ::mpl_core::instructions::UpdateGroupV1 {
            group: accounts.group.address(),
            payer: accounts.payer.address(),
            authority: accounts.authority.as_ref().map(|a| a.address()),
            new_update_authority: accounts.new_update_authority.as_ref().map(|a| a.address()),
            system_program: accounts.system_program.address(),
        }
        .instruction(::mpl_core::instructions::UpdateGroupV1InstructionArgs {
            new_name: new_name.map(|s| s.to_string()),
            new_uri: new_uri.map(|s| s.to_string()),
        });

        let cpi_accounts = [
            CpiHandle::from(accounts.group),
            CpiHandle::from(accounts.payer),
            accounts.authority.unwrap_or_else(|| program.clone()),
            accounts
                .new_update_authority
                .unwrap_or_else(|| program.clone()),
            accounts.system_program,
            program,
        ];
        crate::cpi::invoke_signed(&ix, &cpi_accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let mut data = crate::prelude::Vec::new();
        data.push(41u8); // UpdateGroupV1 discriminator
        match new_name {
            Some(s) => {
                data.push(1u8);
                data.extend_from_slice(&(s.len() as u32).to_le_bytes());
                data.extend_from_slice(s.as_bytes());
            }
            None => data.push(0u8),
        }
        match new_uri {
            Some(s) => {
                data.push(1u8);
                data.extend_from_slice(&(s.len() as u32).to_le_bytes());
                data.extend_from_slice(s.as_bytes());
            }
            None => data.push(0u8),
        }

        let group_handle: CpiHandle<'_> = CpiHandle::from(accounts.group);
        let payer_handle: CpiHandle<'_> = CpiHandle::from(accounts.payer);
        let authority_is_some = accounts.authority.is_some();
        let authority_handle = accounts.authority.unwrap_or(program);
        let new_update_authority_handle = accounts.new_update_authority.unwrap_or(program);

        let ix_accounts = [
            ::pinocchio::instruction::InstructionAccount::writable(
                group_handle.info.view.address(),
            ),
            ::pinocchio::instruction::InstructionAccount::writable_signer(
                payer_handle.info.view.address(),
            ),
            if authority_is_some {
                ::pinocchio::instruction::InstructionAccount::readonly_signer(
                    authority_handle.info.view.address(),
                )
            } else {
                ::pinocchio::instruction::InstructionAccount::readonly(
                    authority_handle.info.view.address(),
                )
            },
            ::pinocchio::instruction::InstructionAccount::readonly(
                new_update_authority_handle.info.view.address(),
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
            group_handle,
            payer_handle,
            authority_handle,
            new_update_authority_handle,
            accounts.system_program,
        ];
        crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles, signer_seeds)
    }
}
