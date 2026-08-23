// ===========================================================================
// group_relationship.rs — add/remove assets, collections, and child groups
// ===========================================================================

//! The 6 remaining `GroupV1` relationship instructions
//! (`AddAssetsToGroupV1`/`RemoveAssetsFromGroupV1`,
//! `AddCollectionsToGroupV1`/`RemoveCollectionsFromGroupV1`,
//! `AddGroupsToGroupV1`/`RemoveGroupsFromGroupV1`). Each is written out
//! explicitly rather than through a shared macro — an earlier attempt at
//! one hid three real per-instruction differences that only surfaced at
//! compile time: `AddAssetsToGroupV1`/`AddCollectionsToGroupV1` have *no*
//! args parameter at all (not even an empty struct — verified via the real
//! SDK: `instruction_with_remaining_accounts(&self, remaining_accounts)`,
//! one parameter, no args); `AddGroupsToGroupV1`/`RemoveGroupsFromGroupV1`
//! name their group field `parent_group`, not `group`; and the four
//! "Remove"/`AddGroupsToGroupV1` instructions carry a redundant
//! `Vec<Pubkey>` arg used purely to cross-validate `remaining_accounts`
//! position-by-position (verified in the real processors, e.g.
//! `add_groups_to_group.rs`: `if child_info.key != &args.groups[i] { ...
//! IncorrectAccount ... }`) — this file derives that list from `items`'
//! own addresses rather than asking the caller to pass the same data
//! twice.
//!
//! All six share the same fixed-account shape otherwise — `group`/
//! `parent_group`, `payer`, `authority` (optional), `system_program` —
//! plus `remaining_accounts`: one *writable* account per item, since
//! adding/removing a relationship writes to both sides where relevant
//! (e.g. `AddGroupsToGroupV1` pushes onto the child's own `parent_groups`
//! list, not just the parent's `groups` list).
//!
//! `items` is taken as an owned `Vec<CpiHandleMut>` (not a borrowed slice)
//! — `CpiHandleMut` deliberately isn't `Clone` (it represents exclusive
//! access), so a variable-length list of them can only be consumed by
//! value, not extracted from a shared reference.

use crate::prelude::*;

/// Accounts consumed by every function in this file — mirrors the real
/// `Add*ToGroupV1`/`Remove*FromGroupV1` instructions' fixed accounts
/// exactly (identical across all six; `group` doubles as `parent_group`
/// for the groups-to-group variants).
pub struct GroupRelationshipAccounts<'a> {
    pub group: CpiHandleMut<'a>,
    pub payer: CpiHandleMut<'a>,
    pub authority: Option<CpiHandle<'a>>,
    pub system_program: CpiHandle<'a>,
}

/// `pinocchio`-only: shared account-list/CPI-invoke mechanics for all six
/// instructions in this file — same 4 fixed accounts, differing only in
/// discriminator and whether `pubkey_arg_data` (the redundant
/// cross-validation list some of the six carry) is written.
#[cfg(feature = "pinocchio")]
fn invoke_group_relationship_pinocchio(
    program: CpiHandle<'_>,
    accounts: GroupRelationshipAccounts<'_>,
    discriminator: u8,
    include_pubkey_arg: bool,
    items: crate::prelude::Vec<CpiHandleMut<'_>>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let item_handles: crate::prelude::Vec<CpiHandle<'_>> =
        items.into_iter().map(CpiHandle::from).collect();
    // A separate, stable address list — `ix_accounts` below needs `&Address`
    // references that outlive `item_handles` itself, since `item_handles`
    // (the `Vec`, not its individually-`Copy` elements) is moved into
    // `handles` further down; borrowing straight from `item_handles` would
    // conflict with that move even though each `CpiHandle` is `Copy`.
    // Element type left to inference deliberately: `AccountView::address()`
    // returns `pinocchio`'s own `Address` (from `solana-address`), a
    // distinct type from naclac's own `Address` that `crate::prelude::*`
    // would otherwise shadow it with.
    let item_addresses = item_handles
        .iter()
        .map(|h| *h.info.view.address())
        .collect::<crate::prelude::Vec<_>>();

    let mut data = crate::prelude::Vec::with_capacity(
        1 + if include_pubkey_arg { 4 + item_handles.len() * 32 } else { 0 },
    );
    data.push(discriminator);
    if include_pubkey_arg {
        data.extend_from_slice(&(item_addresses.len() as u32).to_le_bytes());
        for addr in &item_addresses {
            data.extend_from_slice(addr.as_ref());
        }
    }

    let group_handle: CpiHandle<'_> = CpiHandle::from(accounts.group);
    let payer_handle: CpiHandle<'_> = CpiHandle::from(accounts.payer);
    let authority_is_some = accounts.authority.is_some();
    let authority_handle = accounts.authority.unwrap_or(program);

    let mut ix_accounts: crate::prelude::Vec<::pinocchio::instruction::InstructionAccount<'_>> =
        crate::prelude::Vec::with_capacity(4 + item_handles.len());
    ix_accounts.push(::pinocchio::instruction::InstructionAccount::writable(
        group_handle.info.view.address(),
    ));
    ix_accounts.push(::pinocchio::instruction::InstructionAccount::writable_signer(
        payer_handle.info.view.address(),
    ));
    ix_accounts.push(if authority_is_some {
        ::pinocchio::instruction::InstructionAccount::readonly_signer(
            authority_handle.info.view.address(),
        )
    } else {
        ::pinocchio::instruction::InstructionAccount::readonly(
            authority_handle.info.view.address(),
        )
    });
    ix_accounts.push(::pinocchio::instruction::InstructionAccount::readonly(
        accounts.system_program.info.view.address(),
    ));
    for addr in &item_addresses {
        ix_accounts.push(::pinocchio::instruction::InstructionAccount::writable(addr));
    }

    let instruction = ::pinocchio::instruction::InstructionView {
        program_id: program.info.view.address(),
        accounts: &ix_accounts,
        data: &data,
    };

    let mut handles: crate::prelude::Vec<CpiHandle<'_>> =
        crate::prelude::Vec::with_capacity(4 + item_handles.len());
    handles.push(group_handle);
    handles.push(payer_handle);
    handles.push(authority_handle);
    handles.push(accounts.system_program);
    handles.extend(item_handles);
    crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles, signer_seeds)
}

/// `solana`-only: shared account-list/CPI-invoke mechanics, mirroring
/// `invoke_group_relationship_pinocchio` above.
#[cfg(not(feature = "pinocchio"))]
fn invoke_group_relationship_solana(
    program: CpiHandle<'_>,
    accounts: GroupRelationshipAccounts<'_>,
    ix: solana_program::instruction::Instruction,
    items: crate::prelude::Vec<CpiHandleMut<'_>>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let mut cpi_accounts: crate::prelude::Vec<CpiHandle<'_>> =
        crate::prelude::Vec::with_capacity(5 + items.len());
    cpi_accounts.push(CpiHandle::from(accounts.group));
    cpi_accounts.push(CpiHandle::from(accounts.payer));
    cpi_accounts.push(accounts.authority.unwrap_or_else(|| program.clone()));
    cpi_accounts.push(accounts.system_program);
    cpi_accounts.extend(items.into_iter().map(CpiHandle::from));
    cpi_accounts.push(program);
    crate::cpi::invoke_signed(&ix, &cpi_accounts, signer_seeds)
}

/// Adds `items` (`Asset` accounts) to a group via a real `AddAssetsToGroupV1`
/// CPI (discriminator `35`, no args at all).
pub fn add_assets_to_group_signed(
    program: CpiHandle<'_>,
    accounts: GroupRelationshipAccounts<'_>,
    items: crate::prelude::Vec<CpiHandleMut<'_>>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let item_metas: crate::prelude::Vec<solana_program::instruction::AccountMeta> = items
            .iter()
            .map(|h| solana_program::instruction::AccountMeta::new(h.address(), false))
            .collect();
        let ix = ::mpl_core::instructions::AddAssetsToGroupV1 {
            group: accounts.group.address(),
            payer: accounts.payer.address(),
            authority: accounts.authority.as_ref().map(|a| a.address()),
            system_program: accounts.system_program.address(),
        }
        .instruction_with_remaining_accounts(&item_metas);
        invoke_group_relationship_solana(program, accounts, ix, items, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        invoke_group_relationship_pinocchio(program, accounts, 35u8, false, items, signer_seeds)
    }
}

/// Removes `items` (`Asset` accounts) from a group via a real
/// `RemoveAssetsFromGroupV1` CPI (discriminator `36`).
pub fn remove_assets_from_group_signed(
    program: CpiHandle<'_>,
    accounts: GroupRelationshipAccounts<'_>,
    items: crate::prelude::Vec<CpiHandleMut<'_>>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let item_metas: crate::prelude::Vec<solana_program::instruction::AccountMeta> = items
            .iter()
            .map(|h| solana_program::instruction::AccountMeta::new(h.address(), false))
            .collect();
        let ix = ::mpl_core::instructions::RemoveAssetsFromGroupV1 {
            group: accounts.group.address(),
            payer: accounts.payer.address(),
            authority: accounts.authority.as_ref().map(|a| a.address()),
            system_program: accounts.system_program.address(),
        }
        .instruction_with_remaining_accounts(
            ::mpl_core::instructions::RemoveAssetsFromGroupV1InstructionArgs {
                assets: items.iter().map(|h| h.address()).collect(),
            },
            &item_metas,
        );
        invoke_group_relationship_solana(program, accounts, ix, items, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        invoke_group_relationship_pinocchio(program, accounts, 36u8, true, items, signer_seeds)
    }
}

/// Adds `items` (`Collection` accounts) to a group via a real
/// `AddCollectionsToGroupV1` CPI (discriminator `33`, no args at all).
pub fn add_collections_to_group_signed(
    program: CpiHandle<'_>,
    accounts: GroupRelationshipAccounts<'_>,
    items: crate::prelude::Vec<CpiHandleMut<'_>>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let item_metas: crate::prelude::Vec<solana_program::instruction::AccountMeta> = items
            .iter()
            .map(|h| solana_program::instruction::AccountMeta::new(h.address(), false))
            .collect();
        let ix = ::mpl_core::instructions::AddCollectionsToGroupV1 {
            group: accounts.group.address(),
            payer: accounts.payer.address(),
            authority: accounts.authority.as_ref().map(|a| a.address()),
            system_program: accounts.system_program.address(),
        }
        .instruction_with_remaining_accounts(&item_metas);
        invoke_group_relationship_solana(program, accounts, ix, items, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        invoke_group_relationship_pinocchio(program, accounts, 33u8, false, items, signer_seeds)
    }
}

/// Removes `items` (`Collection` accounts) from a group via a real
/// `RemoveCollectionsFromGroupV1` CPI (discriminator `34`).
pub fn remove_collections_from_group_signed(
    program: CpiHandle<'_>,
    accounts: GroupRelationshipAccounts<'_>,
    items: crate::prelude::Vec<CpiHandleMut<'_>>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let item_metas: crate::prelude::Vec<solana_program::instruction::AccountMeta> = items
            .iter()
            .map(|h| solana_program::instruction::AccountMeta::new(h.address(), false))
            .collect();
        let ix = ::mpl_core::instructions::RemoveCollectionsFromGroupV1 {
            group: accounts.group.address(),
            payer: accounts.payer.address(),
            authority: accounts.authority.as_ref().map(|a| a.address()),
            system_program: accounts.system_program.address(),
        }
        .instruction_with_remaining_accounts(
            ::mpl_core::instructions::RemoveCollectionsFromGroupV1InstructionArgs {
                collections: items.iter().map(|h| h.address()).collect(),
            },
            &item_metas,
        );
        invoke_group_relationship_solana(program, accounts, ix, items, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        invoke_group_relationship_pinocchio(program, accounts, 34u8, true, items, signer_seeds)
    }
}

/// Adds `items` (child `GroupV1` accounts) to `accounts.group` (acting as
/// the parent) via a real `AddGroupsToGroupV1` CPI (discriminator `37`).
/// Real SDK field is `parent_group`, not `group` — verified, not assumed
/// uniform with the other five.
pub fn add_groups_to_group_signed(
    program: CpiHandle<'_>,
    accounts: GroupRelationshipAccounts<'_>,
    items: crate::prelude::Vec<CpiHandleMut<'_>>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let item_metas: crate::prelude::Vec<solana_program::instruction::AccountMeta> = items
            .iter()
            .map(|h| solana_program::instruction::AccountMeta::new(h.address(), false))
            .collect();
        let ix = ::mpl_core::instructions::AddGroupsToGroupV1 {
            parent_group: accounts.group.address(),
            payer: accounts.payer.address(),
            authority: accounts.authority.as_ref().map(|a| a.address()),
            system_program: accounts.system_program.address(),
        }
        .instruction_with_remaining_accounts(
            ::mpl_core::instructions::AddGroupsToGroupV1InstructionArgs {
                groups: items.iter().map(|h| h.address()).collect(),
            },
            &item_metas,
        );
        invoke_group_relationship_solana(program, accounts, ix, items, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        invoke_group_relationship_pinocchio(program, accounts, 37u8, true, items, signer_seeds)
    }
}

/// Removes `items` (child `GroupV1` accounts) from `accounts.group` (acting
/// as the parent) via a real `RemoveGroupsFromGroupV1` CPI (discriminator
/// `38`). Real SDK field is `parent_group`, not `group`.
pub fn remove_groups_from_group_signed(
    program: CpiHandle<'_>,
    accounts: GroupRelationshipAccounts<'_>,
    items: crate::prelude::Vec<CpiHandleMut<'_>>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let item_metas: crate::prelude::Vec<solana_program::instruction::AccountMeta> = items
            .iter()
            .map(|h| solana_program::instruction::AccountMeta::new(h.address(), false))
            .collect();
        let ix = ::mpl_core::instructions::RemoveGroupsFromGroupV1 {
            parent_group: accounts.group.address(),
            payer: accounts.payer.address(),
            authority: accounts.authority.as_ref().map(|a| a.address()),
            system_program: accounts.system_program.address(),
        }
        .instruction_with_remaining_accounts(
            ::mpl_core::instructions::RemoveGroupsFromGroupV1InstructionArgs {
                groups: items.iter().map(|h| h.address()).collect(),
            },
            &item_metas,
        );
        invoke_group_relationship_solana(program, accounts, ix, items, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        invoke_group_relationship_pinocchio(program, accounts, 38u8, true, items, signer_seeds)
    }
}
