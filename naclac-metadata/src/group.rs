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

const KEY_GROUP_V1: u8 = 6;

/// Owned, backend-uniform snapshot of a `GroupV1` account's fields — what
/// `fetch_group` returns on both backends. See `asset.rs`'s `AssetData` doc
/// comment for why this is owned rather than a borrowed `GroupView<'_>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupData {
    pub update_authority: Address,
    pub name: crate::prelude::String,
    pub uri: crate::prelude::String,
    pub collections: crate::prelude::Vec<Address>,
    pub groups: crate::prelude::Vec<Address>,
    pub parent_groups: crate::prelude::Vec<Address>,
    pub assets: crate::prelude::Vec<Address>,
}

/// Reads and fully deserializes a `GroupV1` account.
#[cfg(not(feature = "pinocchio"))]
pub fn fetch_group(info: &AccountInfo) -> Result<GroupData> {
    if Owner::program_owner(info) != crate::ID {
        return Err(NaclacError::ConstraintOwner.into());
    }
    let solana_info = unsafe { info.to_lifetime() };
    let data = solana_info
        .try_borrow_data()
        .map_err(|_| NaclacError::AccountBorrowFailed.err(0))?;
    let view = GroupView::from_bytes(&data)?;
    Ok(GroupData {
        update_authority: view.update_authority(),
        name: view.name().into(),
        uri: view.uri().into(),
        collections: view.collections().iter().collect(),
        groups: view.groups().iter().collect(),
        parent_groups: view.parent_groups().iter().collect(),
        assets: view.assets().iter().collect(),
    })
}

/// Reads and fully deserializes a `GroupV1` account.
#[cfg(feature = "pinocchio")]
pub fn fetch_group(info: &AccountInfo) -> Result<GroupData> {
    let view = GroupView::try_from(info)?;
    Ok(GroupData {
        update_authority: view.update_authority(),
        name: view.name().into(),
        uri: view.uri().into(),
        collections: view.collections().iter().collect(),
        groups: view.groups().iter().collect(),
        parent_groups: view.parent_groups().iter().collect(),
        assets: view.assets().iter().collect(),
    })
}

/// Zero-copy, sequential-offset view into a `GroupV1` account's raw bytes,
/// shared by both backends. See `asset.rs`'s header for why neither backend
/// depends on the real `mpl-core` crate.
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

impl<'a> GroupView<'a> {
    /// `pinocchio`-only: see `AssetView::try_from`'s doc comment
    /// (`asset.rs`) for why the `solana` backend must go through
    /// `from_bytes` directly.
    #[cfg(feature = "pinocchio")]
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

    pub fn collections(&self) -> Span<Address> {
        read_pubkey_span(self.data, self.collections_offset)
    }

    pub fn groups(&self) -> Span<Address> {
        read_pubkey_span(self.data, self.groups_offset)
    }

    pub fn parent_groups(&self) -> Span<Address> {
        read_pubkey_span(self.data, self.parent_groups_offset)
    }

    pub fn assets(&self) -> Span<Address> {
        read_pubkey_span(self.data, self.assets_offset)
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

/// Validates a `Vec<Pubkey>` at `offset` fits within `data` and returns the
/// offset immediately after it (where the next field starts).
fn skip_pubkey_vec(data: &[u8], offset: usize) -> Result<usize> {
    let prefix_end = offset
        .checked_add(4)
        .ok_or_else(|| NaclacError::AccountDataTooSmall.err(0))?;
    if data.len() < prefix_end {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    let count = u32::from_le_bytes(data[offset..prefix_end].try_into().unwrap()) as usize;
    let end = count
        .checked_mul(32)
        .and_then(|bytes| prefix_end.checked_add(bytes))
        .ok_or_else(|| NaclacError::AccountDataTooSmall.err(0))?;
    if data.len() < end {
        return Err(NaclacError::AccountDataTooSmall.err(0));
    }
    Ok(end)
}

/// Zero-copy `Span<Address>` view of a `Vec<Pubkey>` field at `offset` —
/// `Address` is `bytemuck::Pod` on both backends (naclac-core's own newtype
/// on `pinocchio`; the real `solana_address::Address`'s own impl on
/// `solana`), so no heap collection is needed here (bounds already
/// validated by `skip_pubkey_vec` during `GroupView::from_bytes`).
fn read_pubkey_span(data: &[u8], offset: usize) -> Span<Address> {
    let count = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
    // `skip_pubkey_vec` already validated this exact byte range during
    // `GroupView::from_bytes` — its length is guaranteed a multiple of 32,
    // so `Span::from_bytes` cannot fail here.
    Span::from_bytes(&data[offset + 4..offset + 4 + count * 32])
        .expect("range pre-validated by skip_pubkey_vec")
}

// ===========================================================================
// create_group / close_group / update_group
// ===========================================================================

/// Accounts consumed by `create_group_signed` — mirrors the real
/// `CreateGroupV1` instruction exactly. Note `update_authority` must be a
/// **signer** when `Some` here (verified: `AccountMeta::new_readonly(update_authority,
/// true)`) — unlike every other optional-authority-reference field
/// elsewhere in this crate, which are never required to co-sign.
/// Maximum `name`/`uri` byte length `create_group_signed` accepts on
/// `pinocchio` — same reasoning and cap as `asset::MAX_ASSET_NAME_LEN`/
/// `asset::MAX_ASSET_URI_LEN`.
#[cfg(feature = "pinocchio")]
pub const MAX_GROUP_NAME_LEN: usize = 32;
#[cfg(feature = "pinocchio")]
pub const MAX_GROUP_URI_LEN: usize = 200;

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
        let mut data = crate::prelude::Vec::new();
        data.push(39u8); // CreateGroupV1 discriminator
        data.extend_from_slice(&(name.len() as u32).to_le_bytes());
        data.extend_from_slice(name.as_bytes());
        data.extend_from_slice(&(uri.len() as u32).to_le_bytes());
        data.extend_from_slice(uri.as_bytes());
        data.extend_from_slice(&0u32.to_le_bytes()); // relationships: empty Vec

        let accounts_meta = vec![
            solana_program::instruction::AccountMeta::new(accounts.group.address(), true),
            match &accounts.update_authority {
                Some(a) => {
                    solana_program::instruction::AccountMeta::new_readonly(a.address(), true)
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
        if name.len() > MAX_GROUP_NAME_LEN || uri.len() > MAX_GROUP_URI_LEN {
            return Err(NaclacError::InvalidInstructionData.err(0));
        }
        let mut data = crate::fixed_buf::FixedBuf::<
            { 1 + 4 + MAX_GROUP_NAME_LEN + 4 + MAX_GROUP_URI_LEN + 4 },
        >::new();
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
            data: data.as_slice(),
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
        let data = [40u8]; // CloseGroupV1 discriminator, no args
        let accounts_meta = vec![
            solana_program::instruction::AccountMeta::new(accounts.group.address(), false),
            solana_program::instruction::AccountMeta::new(accounts.payer.address(), true),
            match &accounts.authority {
                Some(a) => {
                    solana_program::instruction::AccountMeta::new_readonly(a.address(), true)
                }
                None => solana_program::instruction::AccountMeta::new_readonly(crate::ID, false),
            },
        ];
        let ix = solana_program::instruction::Instruction {
            program_id: crate::ID,
            accounts: accounts_meta,
            data: data.to_vec(),
        };

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

        let accounts_meta = vec![
            solana_program::instruction::AccountMeta::new(accounts.group.address(), false),
            solana_program::instruction::AccountMeta::new(accounts.payer.address(), true),
            match &accounts.authority {
                Some(a) => {
                    solana_program::instruction::AccountMeta::new_readonly(a.address(), true)
                }
                None => solana_program::instruction::AccountMeta::new_readonly(crate::ID, false),
            },
            match &accounts.new_update_authority {
                Some(a) => {
                    solana_program::instruction::AccountMeta::new_readonly(a.address(), false)
                }
                None => solana_program::instruction::AccountMeta::new_readonly(crate::ID, false),
            },
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
        if new_name.is_some_and(|s| s.len() > MAX_GROUP_NAME_LEN)
            || new_uri.is_some_and(|s| s.len() > MAX_GROUP_URI_LEN)
        {
            return Err(NaclacError::InvalidInstructionData.err(0));
        }
        let mut data = crate::fixed_buf::FixedBuf::<
            { 1 + 5 + MAX_GROUP_NAME_LEN + 5 + MAX_GROUP_URI_LEN },
        >::new();
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
            data: data.as_slice(),
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

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Proves `skip_pubkey_vec` never panics for any account bytes and any
    /// `offset` — `offset` is a bare `usize` parameter with no bound in the
    /// function's own signature, and `end = offset + 4 + count * 32` is
    /// exactly the same "unguarded addition on an account-byte-derived
    /// value" shape that `plugin_registry::find_plugin_offset` was
    /// confirmed to panic on before its fix (see docs/plan/kani-audit.md).
    #[kani::proof]
    fn prove_skip_pubkey_vec_never_panics() {
        let data: [u8; 40] = kani::any();
        let offset: usize = kani::any();
        let _ = skip_pubkey_vec(&data, offset);
    }

    /// Proves `GroupView::from_bytes` never panics for any account bytes —
    /// including through `read_borsh_string_len`'s and `skip_pubkey_vec`'s
    /// now-`checked_end`-guarded offset arithmetic.
    #[kani::proof]
    #[kani::unwind(8)]
    fn prove_group_view_from_bytes_never_panics() {
        let data: [u8; 48] = kani::any();
        let _ = GroupView::from_bytes(&data);
    }

    /// Proves `read_pubkey_span`'s own doc comment ("cannot fail" once
    /// `skip_pubkey_vec` has validated `offset`) formally, rather than
    /// leaving it an unverified claim: for any `data`/`offset` pair that
    /// `skip_pubkey_vec` actually accepts (`Ok(_)`), `read_pubkey_span`
    /// neither panics nor overflows its own re-derived offsets, and reads
    /// back exactly the byte count `skip_pubkey_vec` validated.
    #[kani::proof]
    fn prove_read_pubkey_span_is_safe_when_precondition_holds() {
        let data: [u8; 40] = kani::any();
        let offset: usize = kani::any();

        if let Ok(end) = skip_pubkey_vec(&data, offset) {
            let span = read_pubkey_span(&data, offset);
            let count = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
            assert_eq!(offset + 4 + count * 32, end);
            assert_eq!(span.len(), count);
        }
    }
}
