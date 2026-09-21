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
//! `items` is a borrowed `&[CpiHandleMut]`, not owned — building a
//! `CpiHandle` from each only needs to read+copy its `info: AccountInfo`
//! (both fields are `pub`), which `AccountInfo::to_cpi_handle()` (an
//! existing `naclac-core` trait method: `*self` on `pinocchio`, `self.clone()`
//! on `solana`) already does without consuming the `CpiHandleMut`. On
//! `pinocchio` the account list is capped at `MAX_CPI_ACCOUNTS` (the
//! framework's own existing CPI-account-array bound) and built as a
//! stack array — see `invoke_group_relationship_pinocchio`.

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
/// cross-validation list some of the six carry) is written. `items` is
/// capped at `MAX_CPI_ACCOUNTS - 4` (the fixed accounts) — no real
/// protocol maximum exists, but that's the real ceiling this crate's own
/// zero-heap CPI array can hold either way.
#[cfg(feature = "pinocchio")]
fn invoke_group_relationship_pinocchio(
    program: CpiHandle<'_>,
    accounts: GroupRelationshipAccounts<'_>,
    discriminator: u8,
    include_pubkey_arg: bool,
    items: &[CpiHandleMut<'_>],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    const FIXED_ACCOUNTS: usize = 4;
    const MAX_ITEMS: usize = MAX_CPI_ACCOUNTS - FIXED_ACCOUNTS;
    if items.len() > MAX_ITEMS {
        return Err(NaclacError::TooManyCpiAccounts.into());
    }

    let mut data = crate::fixed_buf::FixedBuf::<{ 1 + 4 + MAX_ITEMS * 32 }>::new();
    data.push(discriminator);
    if include_pubkey_arg {
        data.extend_from_slice(&(items.len() as u32).to_le_bytes());
        for item in items {
            data.extend_from_slice(item.info.address().as_ref());
        }
    }

    let group_handle: CpiHandle<'_> = CpiHandle::from(accounts.group);
    let payer_handle: CpiHandle<'_> = CpiHandle::from(accounts.payer);
    let authority_is_some = accounts.authority.is_some();
    let authority_handle = accounts.authority.unwrap_or(program);

    // Fixed-capacity, zero-heap account arrays — see `execute.rs`'s
    // `execute_signed` for the same pattern and why the placeholder fill
    // value is never actually read.
    let mut ix_accounts: [::pinocchio::instruction::InstructionAccount<'_>; MAX_CPI_ACCOUNTS] =
        core::array::from_fn(|_| {
            ::pinocchio::instruction::InstructionAccount::readonly(program.info.view.address())
        });
    ix_accounts[0] =
        ::pinocchio::instruction::InstructionAccount::writable(group_handle.info.view.address());
    ix_accounts[1] = ::pinocchio::instruction::InstructionAccount::writable_signer(
        payer_handle.info.view.address(),
    );
    ix_accounts[2] = if authority_is_some {
        ::pinocchio::instruction::InstructionAccount::readonly_signer(
            authority_handle.info.view.address(),
        )
    } else {
        ::pinocchio::instruction::InstructionAccount::readonly(
            authority_handle.info.view.address(),
        )
    };
    ix_accounts[3] = ::pinocchio::instruction::InstructionAccount::readonly(
        accounts.system_program.info.view.address(),
    );
    for (i, item) in items.iter().enumerate() {
        ix_accounts[FIXED_ACCOUNTS + i] =
            ::pinocchio::instruction::InstructionAccount::writable(item.info.view.address());
    }
    let total = FIXED_ACCOUNTS + items.len();

    let instruction = ::pinocchio::instruction::InstructionView {
        program_id: program.info.view.address(),
        accounts: &ix_accounts[..total],
        data: data.as_slice(),
    };

    let mut handles = [program; MAX_CPI_ACCOUNTS];
    handles[0] = group_handle;
    handles[1] = payer_handle;
    handles[2] = authority_handle;
    handles[3] = accounts.system_program;
    for (i, item) in items.iter().enumerate() {
        handles[FIXED_ACCOUNTS + i] = item.info.to_cpi_handle();
    }
    crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles[..total], signer_seeds)
}

/// `solana`-only: builds the raw `Instruction` shared by all six functions
/// below — discriminator + (if `include_pubkey_arg`) a `Vec<Pubkey>` of
/// `items`' own addresses (the redundant cross-validation list some of the
/// six carry — see this file's header) + the same 4 fixed accounts as
/// `invoke_group_relationship_pinocchio`, plus one writable meta per item.
#[cfg(not(feature = "pinocchio"))]
fn build_group_relationship_instruction(
    accounts: &GroupRelationshipAccounts<'_>,
    discriminator: u8,
    include_pubkey_arg: bool,
    items: &[CpiHandleMut<'_>],
) -> solana_program::instruction::Instruction {
    let mut data = crate::prelude::Vec::new();
    data.push(discriminator);
    if include_pubkey_arg {
        data.extend_from_slice(&(items.len() as u32).to_le_bytes());
        for item in items {
            data.extend_from_slice(item.address().as_ref());
        }
    }

    let mut accounts_meta = vec![
        solana_program::instruction::AccountMeta::new(accounts.group.address(), false),
        solana_program::instruction::AccountMeta::new(accounts.payer.address(), true),
        match &accounts.authority {
            Some(a) => {
                solana_program::instruction::AccountMeta::new_readonly(a.address(), true)
            }
            None => solana_program::instruction::AccountMeta::new_readonly(crate::ID, false),
        },
        solana_program::instruction::AccountMeta::new_readonly(
            accounts.system_program.address(),
            false,
        ),
    ];
    for item in items {
        accounts_meta.push(solana_program::instruction::AccountMeta::new(
            item.address(),
            false,
        ));
    }

    solana_program::instruction::Instruction {
        program_id: crate::ID,
        accounts: accounts_meta,
        data,
    }
}

/// `solana`-only: shared account-list/CPI-invoke mechanics, mirroring
/// `invoke_group_relationship_pinocchio` above.
#[cfg(not(feature = "pinocchio"))]
fn invoke_group_relationship_solana(
    program: CpiHandle<'_>,
    accounts: GroupRelationshipAccounts<'_>,
    ix: solana_program::instruction::Instruction,
    items: &[CpiHandleMut<'_>],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let mut cpi_accounts: crate::prelude::Vec<CpiHandle<'_>> =
        crate::prelude::Vec::with_capacity(5 + items.len());
    cpi_accounts.push(CpiHandle::from(accounts.group));
    cpi_accounts.push(CpiHandle::from(accounts.payer));
    cpi_accounts.push(accounts.authority.unwrap_or_else(|| program.clone()));
    cpi_accounts.push(accounts.system_program);
    cpi_accounts.extend(items.iter().map(|h| h.info.to_cpi_handle()));
    cpi_accounts.push(program);
    crate::cpi::invoke_signed(&ix, &cpi_accounts, signer_seeds)
}

/// Adds `items` (`Asset` accounts) to a group via a real `AddAssetsToGroupV1`
/// CPI (discriminator `35`, no args at all).
pub fn add_assets_to_group_signed(
    program: CpiHandle<'_>,
    accounts: GroupRelationshipAccounts<'_>,
    items: &[CpiHandleMut<'_>],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = build_group_relationship_instruction(&accounts, 35u8, false, items);
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
    items: &[CpiHandleMut<'_>],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = build_group_relationship_instruction(&accounts, 36u8, true, items);
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
    items: &[CpiHandleMut<'_>],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = build_group_relationship_instruction(&accounts, 33u8, false, items);
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
    items: &[CpiHandleMut<'_>],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = build_group_relationship_instruction(&accounts, 34u8, true, items);
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
    items: &[CpiHandleMut<'_>],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = build_group_relationship_instruction(&accounts, 37u8, true, items);
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
    items: &[CpiHandleMut<'_>],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = build_group_relationship_instruction(&accounts, 38u8, true, items);
        invoke_group_relationship_solana(program, accounts, ix, items, signer_seeds)
    }

    #[cfg(feature = "pinocchio")]
    {
        invoke_group_relationship_pinocchio(program, accounts, 38u8, true, items, signer_seeds)
    }
}
