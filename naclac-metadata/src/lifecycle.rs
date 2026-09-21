// ===========================================================================
// lifecycle.rs — transfer/burn/update for both `Asset` and `Collection`
// ===========================================================================

//! The top-level lifecycle instructions — move ownership, destroy, and
//! change name/uri/update-authority — as distinct from plugin management
//! (`plugin.rs`). The `Asset` trio (`TransferV1`/`BurnV1`/`UpdateV2`) and
//! `Collection`'s own `UpdateCollectionV1`/`BurnCollectionV1` (there is no
//! `TransferCollectionV1` — collections aren't owned/transferred the way
//! assets are). All burn/transfer instructions take a `compression_proof`
//! field this crate always leaves at its default (`None`/no move) since
//! state-compressed assets are out of scope (see `plugin_registry.rs`'s
//! header for the same "Tier 4, out of scope for now" reasoning applied to
//! compression elsewhere in this crate).
//!
//! Real shapes verified against `mpl-core`: `TransferV1` (discriminator
//! `14`), `BurnV1` (discriminator `12`), `UpdateV2` (discriminator `30`,
//! not `UpdateV1` — matches this crate's `create_asset_signed` already
//! using `CreateV2` over `CreateV1`), `UpdateCollectionV1` (discriminator
//! `16` — there is no V2 for this one), `BurnCollectionV1` (discriminator
//! `13`).

use crate::prelude::*;

/// Accounts consumed by `transfer_asset_signed` — mirrors `TransferV1`
/// exactly. Note `collection` is **readonly** here (unlike `plugin.rs`'s
/// `AddAssetPluginAccounts`, where it's writable) — verified separately
/// against real source, not assumed from that similarity.
pub struct TransferAssetAccounts<'a> {
    pub asset: CpiHandleMut<'a>,
    pub collection: Option<CpiHandle<'a>>,
    pub payer: CpiHandleMut<'a>,
    pub authority: Option<CpiHandle<'a>>,
    pub new_owner: CpiHandle<'a>,
    pub system_program: Option<CpiHandle<'a>>,
    pub log_wrapper: Option<CpiHandle<'a>>,
}

/// Accounts consumed by `burn_asset_signed` — mirrors `BurnV1` exactly.
/// `collection` here is writable (burning updates the collection's
/// `current_size`), unlike `TransferAssetAccounts`'s readonly one.
pub struct BurnAssetAccounts<'a> {
    pub asset: CpiHandleMut<'a>,
    pub collection: Option<CpiHandleMut<'a>>,
    pub payer: CpiHandleMut<'a>,
    pub authority: Option<CpiHandle<'a>>,
    pub system_program: Option<CpiHandle<'a>>,
    pub log_wrapper: Option<CpiHandle<'a>>,
}

/// Accounts consumed by `update_asset_signed` — mirrors `UpdateV2` exactly.
/// `new_collection` is a separate, optional slot for *moving* the asset to
/// a different collection as part of the same update — leave `None` to
/// leave the asset's collection unchanged.
pub struct UpdateAssetAccounts<'a> {
    pub asset: CpiHandleMut<'a>,
    pub collection: Option<CpiHandleMut<'a>>,
    pub payer: CpiHandleMut<'a>,
    pub authority: Option<CpiHandle<'a>>,
    pub new_collection: Option<CpiHandleMut<'a>>,
    pub system_program: CpiHandle<'a>,
    pub log_wrapper: Option<CpiHandle<'a>>,
}

/// Accounts consumed by `update_collection_signed` — mirrors the real
/// `UpdateCollectionV1` instruction exactly. `new_update_authority` is an
/// *account* here (its pubkey becomes the collection's new plain
/// `Pubkey` `update_authority`), unlike `UpdateAssetAccounts`'s
/// `new_update_authority` *argument* (`UpdateAuthorityKind`) — verified
/// directly from the real processor: `collection.update_authority =
/// *new_update_authority.key`.
pub struct UpdateCollectionAccounts<'a> {
    pub collection: CpiHandleMut<'a>,
    pub payer: CpiHandleMut<'a>,
    pub authority: Option<CpiHandle<'a>>,
    pub new_update_authority: Option<CpiHandle<'a>>,
    pub system_program: CpiHandle<'a>,
    pub log_wrapper: Option<CpiHandle<'a>>,
}

/// Accounts consumed by `burn_collection_signed` — mirrors the real
/// `BurnCollectionV1` instruction exactly. Two details that differ from
/// every other lifecycle instruction in this file, verified directly
/// against real source rather than assumed from the pattern: `authority`
/// is **writable** here (`AccountMeta::new(authority, true)`, not
/// `new_readonly`), and there is **no `system_program` account at all**.
pub struct BurnCollectionAccounts<'a> {
    pub collection: CpiHandleMut<'a>,
    pub payer: CpiHandleMut<'a>,
    pub authority: Option<CpiHandleMut<'a>>,
    pub log_wrapper: Option<CpiHandle<'a>>,
}

/// Moves an `Asset` to a new owner via a real `TransferV1` CPI.
pub fn transfer_asset_signed(
    program: CpiHandle<'_>,
    accounts: TransferAssetAccounts<'_>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let data = [14u8, 0u8]; // TransferV1 discriminator + compression_proof: None
        let accounts_meta = vec![
            solana_program::instruction::AccountMeta::new(accounts.asset.address(), false),
            match &accounts.collection {
                Some(c) => {
                    solana_program::instruction::AccountMeta::new_readonly(c.address(), false)
                }
                None => solana_program::instruction::AccountMeta::new_readonly(crate::ID, false),
            },
            solana_program::instruction::AccountMeta::new(accounts.payer.address(), true),
            match &accounts.authority {
                Some(a) => {
                    solana_program::instruction::AccountMeta::new_readonly(a.address(), true)
                }
                None => solana_program::instruction::AccountMeta::new_readonly(crate::ID, false),
            },
            solana_program::instruction::AccountMeta::new_readonly(
                accounts.new_owner.address(),
                false,
            ),
            match &accounts.system_program {
                Some(s) => {
                    solana_program::instruction::AccountMeta::new_readonly(s.address(), false)
                }
                None => solana_program::instruction::AccountMeta::new_readonly(crate::ID, false),
            },
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
            data: data.to_vec(),
        };

        let cpi_accounts = [
            CpiHandle::from(accounts.asset),
            accounts.collection.unwrap_or_else(|| program.clone()),
            CpiHandle::from(accounts.payer),
            accounts.authority.unwrap_or_else(|| program.clone()),
            accounts.new_owner,
            accounts.system_program.unwrap_or_else(|| program.clone()),
            accounts.log_wrapper.unwrap_or_else(|| program.clone()),
            program,
        ];
        crate::cpi::invoke_signed(&ix, &cpi_accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let data = [14u8, 0u8]; // TransferV1 discriminator + compression_proof: None

        let asset_handle: CpiHandle<'_> = CpiHandle::from(accounts.asset);
        let payer_handle: CpiHandle<'_> = CpiHandle::from(accounts.payer);
        let collection_handle = accounts.collection.unwrap_or(program);
        let authority_is_some = accounts.authority.is_some();
        let authority_handle = accounts.authority.unwrap_or(program);
        let system_program_handle = accounts.system_program.unwrap_or(program);
        let log_wrapper_handle = accounts.log_wrapper.unwrap_or(program);

        let ix_accounts = [
            ::pinocchio::instruction::InstructionAccount::writable(
                asset_handle.info.view.address(),
            ),
            ::pinocchio::instruction::InstructionAccount::readonly(
                collection_handle.info.view.address(),
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
                accounts.new_owner.info.view.address(),
            ),
            ::pinocchio::instruction::InstructionAccount::readonly(
                system_program_handle.info.view.address(),
            ),
            ::pinocchio::instruction::InstructionAccount::readonly(
                log_wrapper_handle.info.view.address(),
            ),
        ];
        let instruction = ::pinocchio::instruction::InstructionView {
            program_id: program.info.view.address(),
            accounts: &ix_accounts,
            data: &data,
        };
        let handles = [
            asset_handle,
            collection_handle,
            payer_handle,
            authority_handle,
            accounts.new_owner,
            system_program_handle,
            log_wrapper_handle,
        ];
        crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles, signer_seeds)
    }
}

/// Destroys an `Asset` via a real `BurnV1` CPI.
pub fn burn_asset_signed(
    program: CpiHandle<'_>,
    accounts: BurnAssetAccounts<'_>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let data = [12u8, 0u8]; // BurnV1 discriminator + compression_proof: None
        let accounts_meta = vec![
            solana_program::instruction::AccountMeta::new(accounts.asset.address(), false),
            match &accounts.collection {
                Some(c) => solana_program::instruction::AccountMeta::new(c.address(), false),
                None => solana_program::instruction::AccountMeta::new_readonly(crate::ID, false),
            },
            solana_program::instruction::AccountMeta::new(accounts.payer.address(), true),
            match &accounts.authority {
                Some(a) => {
                    solana_program::instruction::AccountMeta::new_readonly(a.address(), true)
                }
                None => solana_program::instruction::AccountMeta::new_readonly(crate::ID, false),
            },
            match &accounts.system_program {
                Some(s) => {
                    solana_program::instruction::AccountMeta::new_readonly(s.address(), false)
                }
                None => solana_program::instruction::AccountMeta::new_readonly(crate::ID, false),
            },
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
            data: data.to_vec(),
        };

        let cpi_accounts = [
            CpiHandle::from(accounts.asset),
            accounts
                .collection
                .map(CpiHandle::from)
                .unwrap_or_else(|| program.clone()),
            CpiHandle::from(accounts.payer),
            accounts.authority.unwrap_or_else(|| program.clone()),
            accounts.system_program.unwrap_or_else(|| program.clone()),
            accounts.log_wrapper.unwrap_or_else(|| program.clone()),
            program,
        ];
        crate::cpi::invoke_signed(&ix, &cpi_accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let data = [12u8, 0u8]; // BurnV1 discriminator + compression_proof: None

        let asset_handle: CpiHandle<'_> = CpiHandle::from(accounts.asset);
        let payer_handle: CpiHandle<'_> = CpiHandle::from(accounts.payer);
        let collection_is_some = accounts.collection.is_some();
        let authority_is_some = accounts.authority.is_some();
        let collection_handle = accounts.collection.map(CpiHandle::from).unwrap_or(program);
        let authority_handle = accounts.authority.unwrap_or(program);
        let system_program_handle = accounts.system_program.unwrap_or(program);
        let log_wrapper_handle = accounts.log_wrapper.unwrap_or(program);

        let ix_accounts = [
            ::pinocchio::instruction::InstructionAccount::writable(
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
                system_program_handle.info.view.address(),
            ),
            ::pinocchio::instruction::InstructionAccount::readonly(
                log_wrapper_handle.info.view.address(),
            ),
        ];
        let instruction = ::pinocchio::instruction::InstructionView {
            program_id: program.info.view.address(),
            accounts: &ix_accounts,
            data: &data,
        };
        let handles = [
            asset_handle,
            collection_handle,
            payer_handle,
            authority_handle,
            system_program_handle,
            log_wrapper_handle,
        ];
        crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles, signer_seeds)
    }
}

/// Updates an `Asset`'s `name`/`uri`/update-authority (and optionally moves
/// it to a new collection) via a real `UpdateV2` CPI. Any argument left
/// `None` is left unchanged.
pub fn update_asset_signed(
    program: CpiHandle<'_>,
    accounts: UpdateAssetAccounts<'_>,
    new_name: Option<&str>,
    new_uri: Option<&str>,
    new_update_authority: Option<UpdateAuthorityKind>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = crate::prelude::Vec::new();
        data.push(30u8); // UpdateV2 discriminator
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
        match new_update_authority {
            Some(a) => {
                data.push(1u8);
                match a {
                    UpdateAuthorityKind::None => data.push(0u8),
                    UpdateAuthorityKind::Address(addr) => {
                        data.push(1u8);
                        data.extend_from_slice(addr.as_ref());
                    }
                    UpdateAuthorityKind::Collection(addr) => {
                        data.push(2u8);
                        data.extend_from_slice(addr.as_ref());
                    }
                }
            }
            None => data.push(0u8),
        }

        let accounts_meta = vec![
            solana_program::instruction::AccountMeta::new(accounts.asset.address(), false),
            match &accounts.collection {
                Some(c) => solana_program::instruction::AccountMeta::new(c.address(), false),
                None => solana_program::instruction::AccountMeta::new_readonly(crate::ID, false),
            },
            solana_program::instruction::AccountMeta::new(accounts.payer.address(), true),
            match &accounts.authority {
                Some(a) => {
                    solana_program::instruction::AccountMeta::new_readonly(a.address(), true)
                }
                None => solana_program::instruction::AccountMeta::new_readonly(crate::ID, false),
            },
            match &accounts.new_collection {
                Some(c) => solana_program::instruction::AccountMeta::new(c.address(), false),
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
            CpiHandle::from(accounts.payer),
            accounts.authority.unwrap_or_else(|| program.clone()),
            accounts
                .new_collection
                .map(CpiHandle::from)
                .unwrap_or_else(|| program.clone()),
            accounts.system_program,
            accounts.log_wrapper.unwrap_or_else(|| program.clone()),
            program,
        ];
        crate::cpi::invoke_signed(&ix, &cpi_accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        if new_name.is_some_and(|s| s.len() > crate::asset::MAX_ASSET_NAME_LEN)
            || new_uri.is_some_and(|s| s.len() > crate::asset::MAX_ASSET_URI_LEN)
        {
            return Err(NaclacError::InvalidInstructionData.err(0));
        }
        let mut data = crate::fixed_buf::FixedBuf::<
            {
                1 + 5
                    + crate::asset::MAX_ASSET_NAME_LEN
                    + 5
                    + crate::asset::MAX_ASSET_URI_LEN
                    + 1
                    + 33
            },
        >::new();
        data.push(30u8); // UpdateV2 discriminator
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
        match new_update_authority {
            Some(a) => {
                data.push(1u8);
                match a {
                    UpdateAuthorityKind::None => data.push(0u8),
                    UpdateAuthorityKind::Address(addr) => {
                        data.push(1u8);
                        data.extend_from_slice(addr.as_ref());
                    }
                    UpdateAuthorityKind::Collection(addr) => {
                        data.push(2u8);
                        data.extend_from_slice(addr.as_ref());
                    }
                }
            }
            None => data.push(0u8),
        }

        let asset_handle: CpiHandle<'_> = CpiHandle::from(accounts.asset);
        let payer_handle: CpiHandle<'_> = CpiHandle::from(accounts.payer);
        let collection_is_some = accounts.collection.is_some();
        let authority_is_some = accounts.authority.is_some();
        let new_collection_is_some = accounts.new_collection.is_some();
        let collection_handle = accounts.collection.map(CpiHandle::from).unwrap_or(program);
        let authority_handle = accounts.authority.unwrap_or(program);
        let new_collection_handle = accounts
            .new_collection
            .map(CpiHandle::from)
            .unwrap_or(program);
        let log_wrapper_handle = accounts.log_wrapper.unwrap_or(program);

        let ix_accounts = [
            ::pinocchio::instruction::InstructionAccount::writable(
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
            if new_collection_is_some {
                ::pinocchio::instruction::InstructionAccount::writable(
                    new_collection_handle.info.view.address(),
                )
            } else {
                ::pinocchio::instruction::InstructionAccount::readonly(
                    new_collection_handle.info.view.address(),
                )
            },
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
            payer_handle,
            authority_handle,
            new_collection_handle,
            accounts.system_program,
            log_wrapper_handle,
        ];
        crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles, signer_seeds)
    }
}

/// Updates a `Collection`'s `name`/`uri`/`update_authority` via a real
/// `UpdateCollectionV1` CPI. Any argument left `None` is left unchanged.
pub fn update_collection_signed(
    program: CpiHandle<'_>,
    accounts: UpdateCollectionAccounts<'_>,
    new_name: Option<&str>,
    new_uri: Option<&str>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let mut data = crate::prelude::Vec::new();
        data.push(16u8); // UpdateCollectionV1 discriminator
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
            solana_program::instruction::AccountMeta::new(accounts.collection.address(), false),
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
            CpiHandle::from(accounts.collection),
            CpiHandle::from(accounts.payer),
            accounts.authority.unwrap_or_else(|| program.clone()),
            accounts
                .new_update_authority
                .unwrap_or_else(|| program.clone()),
            accounts.system_program,
            accounts.log_wrapper.unwrap_or_else(|| program.clone()),
            program,
        ];
        crate::cpi::invoke_signed(&ix, &cpi_accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        if new_name.is_some_and(|s| s.len() > crate::collection::MAX_COLLECTION_NAME_LEN)
            || new_uri.is_some_and(|s| s.len() > crate::collection::MAX_COLLECTION_URI_LEN)
        {
            return Err(NaclacError::InvalidInstructionData.err(0));
        }
        let mut data = crate::fixed_buf::FixedBuf::<
            {
                1 + 5
                    + crate::collection::MAX_COLLECTION_NAME_LEN
                    + 5
                    + crate::collection::MAX_COLLECTION_URI_LEN
            },
        >::new();
        data.push(16u8); // UpdateCollectionV1 discriminator
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

        let collection_handle: CpiHandle<'_> = CpiHandle::from(accounts.collection);
        let payer_handle: CpiHandle<'_> = CpiHandle::from(accounts.payer);
        let authority_is_some = accounts.authority.is_some();
        let authority_handle = accounts.authority.unwrap_or(program);
        let new_update_authority_handle = accounts.new_update_authority.unwrap_or(program);
        let log_wrapper_handle = accounts.log_wrapper.unwrap_or(program);

        let ix_accounts = [
            ::pinocchio::instruction::InstructionAccount::writable(
                collection_handle.info.view.address(),
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
            collection_handle,
            payer_handle,
            authority_handle,
            new_update_authority_handle,
            accounts.system_program,
            log_wrapper_handle,
        ];
        crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles, signer_seeds)
    }
}

/// Destroys a `Collection` via a real `BurnCollectionV1` CPI. Fails
/// (`MplCoreError::CollectionMustBeEmpty`) if the collection still has
/// member assets — verified directly in the real processor:
/// `if collection.current_size > 0 { return
/// Err(MplCoreError::CollectionMustBeEmpty.into()); }`.
pub fn burn_collection_signed(
    program: CpiHandle<'_>,
    accounts: BurnCollectionAccounts<'_>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let data = [13u8, 0u8]; // BurnCollectionV1 discriminator + compression_proof: None
        let accounts_meta = vec![
            solana_program::instruction::AccountMeta::new(accounts.collection.address(), false),
            solana_program::instruction::AccountMeta::new(accounts.payer.address(), true),
            match &accounts.authority {
                Some(a) => solana_program::instruction::AccountMeta::new(a.address(), true),
                None => solana_program::instruction::AccountMeta::new_readonly(crate::ID, false),
            },
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
            data: data.to_vec(),
        };

        let cpi_accounts = [
            CpiHandle::from(accounts.collection),
            CpiHandle::from(accounts.payer),
            accounts
                .authority
                .map(CpiHandle::from)
                .unwrap_or_else(|| program.clone()),
            accounts.log_wrapper.unwrap_or_else(|| program.clone()),
            program,
        ];
        crate::cpi::invoke_signed(&ix, &cpi_accounts, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        let data = [13u8, 0u8]; // BurnCollectionV1 discriminator + compression_proof: None

        let collection_handle: CpiHandle<'_> = CpiHandle::from(accounts.collection);
        let payer_handle: CpiHandle<'_> = CpiHandle::from(accounts.payer);
        let authority_is_some = accounts.authority.is_some();
        let authority_handle = accounts.authority.map(CpiHandle::from).unwrap_or(program);
        let log_wrapper_handle = accounts.log_wrapper.unwrap_or(program);

        let ix_accounts = [
            ::pinocchio::instruction::InstructionAccount::writable(
                collection_handle.info.view.address(),
            ),
            ::pinocchio::instruction::InstructionAccount::writable_signer(
                payer_handle.info.view.address(),
            ),
            if authority_is_some {
                ::pinocchio::instruction::InstructionAccount::writable_signer(
                    authority_handle.info.view.address(),
                )
            } else {
                ::pinocchio::instruction::InstructionAccount::readonly(
                    authority_handle.info.view.address(),
                )
            },
            ::pinocchio::instruction::InstructionAccount::readonly(
                log_wrapper_handle.info.view.address(),
            ),
        ];
        let instruction = ::pinocchio::instruction::InstructionView {
            program_id: program.info.view.address(),
            accounts: &ix_accounts,
            data: &data,
        };
        let handles = [
            collection_handle,
            payer_handle,
            authority_handle,
            log_wrapper_handle,
        ];
        crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles, signer_seeds)
    }
}
