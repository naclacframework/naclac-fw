// ===========================================================================
// extensions/token_group.rs — TokenGroup / TokenGroupMember
// ===========================================================================

//! CPI wrappers for the `spl-token-group-interface` instructions Token-2022
//! processes directly against a `GroupPointer`/`GroupMemberPointer`-linked
//! account: `InitializeGroup`, `UpdateGroupMaxSize`, `UpdateGroupAuthority`,
//! `InitializeMember`. Unlike `TokenMetadata`, both `TokenGroup` and
//! `TokenGroupMember` are fixed-size Pod structs — no reallocation is ever
//! needed for any of these instructions. Each instruction's data/account
//! shape is built via `spl-token-group-interface`'s own real
//! `instruction::*` constructors, the same approach `token_metadata.rs` uses
//! for `spl-token-metadata-interface`.
//!
//! Both backends: `pinocchio-token-2022` itself covers only the
//! `GroupPointer`/`GroupMemberPointer` extensions (not the content
//! instructions they point at), but every instruction here is built as a
//! data-only `Instruction` value with no `AccountInfo` involved, so
//! `super::invoke_interface_instruction` (`extensions/mod.rs`) can dispatch
//! it under either backend, same as `token_metadata.rs`. Beyond the
//! official `anchor-spl` crate's own `token_2022_extensions::token_group`
//! module (which exposes only
//! `token_group_initialize`/`token_member_initialize`), this also covers
//! `UpdateGroupMaxSize`/`UpdateGroupAuthority`, both real,
//! separately-dispatched interface instructions it omits.

use super::Extension;
use crate::prelude::{Address, CpiHandle, CpiHandleMut, Result};

/// `TokenGroup`/`TokenGroupMember` (`spl_token_group_interface::state`) are
/// real, already `#[repr(C)]`/`Pod`/`Zeroable` fixed-size structs — unlike
/// `TokenMetadata`, they fit naclac's own `Extension` trait directly (no
/// wrapper type needed), so they're read the same way every other extension
/// in this crate is, via `InterfaceAccount<Mint>::get_extension`. `TYPE`
/// values (`TokenGroup` = 21, `TokenGroupMember` = 23) verified against
/// `spl-token-2022-interface`'s real `ExtensionType` enum ordering (both
/// participate in the same TLV numbering space as every other extension,
/// confirmed via that crate's own `ExtensionType::get_type_len` match arms).
impl Extension for spl_token_group_interface::state::TokenGroup {
    const TYPE: u16 = 21;
    const ACCOUNT_TYPE: u8 = 1;
}

impl Extension for spl_token_group_interface::state::TokenGroupMember {
    const TYPE: u16 = 23;
    const ACCOUNT_TYPE: u8 = 1;
}

/// Initializes a `TokenGroup` on an already-allocated, rent-exempt group
/// account (the mint itself, when `GroupPointer` points at it — see
/// `initialize_group_pointer` in `group_pointer.rs`). Must be called after
/// `initialize_mint`/`initialize_mint_signed`, since it assumes the mint it
/// references is already initialized.
pub fn initialize_token_group(
    program: CpiHandle<'_>,
    group: CpiHandleMut<'_>,
    mint: CpiHandle<'_>,
    mint_authority: CpiHandle<'_>,
    update_authority: Option<&Address>,
    max_size: u64,
) -> Result<()> {
    initialize_token_group_signed(
        program,
        group,
        mint,
        mint_authority,
        update_authority,
        max_size,
        &[],
    )
}

pub fn initialize_token_group_signed(
    program: CpiHandle<'_>,
    group: CpiHandleMut<'_>,
    mint: CpiHandle<'_>,
    mint_authority: CpiHandle<'_>,
    update_authority: Option<&Address>,
    max_size: u64,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    let ix = spl_token_group_interface::instruction::initialize_group(
        &super::ix_addr(&program),
        &super::ix_addr(&group.info),
        &super::ix_addr(&mint),
        &super::ix_addr(&mint_authority),
        update_authority.map(super::ix_addr_ref),
        max_size,
    );
    let accounts = [CpiHandle::from(group), mint, mint_authority, program];
    super::invoke_interface_instruction(&ix, &accounts, signer_seeds)
}

/// Updates an already-initialized `TokenGroup`'s max member count. Fails
/// on-chain if `new_max_size` is below the group's current member count.
pub fn update_token_group_max_size(
    program: CpiHandle<'_>,
    group: CpiHandleMut<'_>,
    update_authority: CpiHandle<'_>,
    max_size: u64,
) -> Result<()> {
    update_token_group_max_size_signed(program, group, update_authority, max_size, &[])
}

pub fn update_token_group_max_size_signed(
    program: CpiHandle<'_>,
    group: CpiHandleMut<'_>,
    update_authority: CpiHandle<'_>,
    max_size: u64,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    let ix = spl_token_group_interface::instruction::update_group_max_size(
        &super::ix_addr(&program),
        &super::ix_addr(&group.info),
        &super::ix_addr(&update_authority),
        max_size,
    );
    let accounts = [CpiHandle::from(group), update_authority, program];
    super::invoke_interface_instruction(&ix, &accounts, signer_seeds)
}

/// Updates a `TokenGroup`'s update authority. `new_authority` of `None`
/// permanently removes update authority from the group.
pub fn update_token_group_authority(
    program: CpiHandle<'_>,
    group: CpiHandleMut<'_>,
    current_authority: CpiHandle<'_>,
    new_authority: Option<&Address>,
) -> Result<()> {
    update_token_group_authority_signed(program, group, current_authority, new_authority, &[])
}

pub fn update_token_group_authority_signed(
    program: CpiHandle<'_>,
    group: CpiHandleMut<'_>,
    current_authority: CpiHandle<'_>,
    new_authority: Option<&Address>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    let ix = spl_token_group_interface::instruction::update_group_authority(
        &super::ix_addr(&program),
        &super::ix_addr(&group.info),
        &super::ix_addr(&current_authority),
        new_authority.map(super::ix_addr_ref),
    );
    let accounts = [CpiHandle::from(group), current_authority, program];
    super::invoke_interface_instruction(&ix, &accounts, signer_seeds)
}

/// Bundles `InitializeMember`'s five account handles — keeps
/// `initialize_token_group_member(_signed)` under
/// `clippy::too_many_arguments`, the same fix `AtaCpiAccounts`
/// (`associated_token.rs`) applies to `create`'s own account list.
pub struct TokenGroupMemberInitializeAccounts<'a> {
    pub member: CpiHandleMut<'a>,
    pub member_mint: CpiHandle<'a>,
    pub member_mint_authority: CpiHandle<'a>,
    pub group: CpiHandleMut<'a>,
    pub group_update_authority: CpiHandle<'a>,
}

/// Initializes a `TokenGroupMember` on an already-allocated, rent-exempt
/// member account (the member mint itself, when `GroupMemberPointer` points
/// at it — see `initialize_group_member_pointer` in
/// `group_member_pointer.rs`), incrementing `group`'s member count. Must be
/// called after `initialize_mint`/`initialize_mint_signed` for the member
/// mint, and after `initialize_token_group` for the group it joins.
pub fn initialize_token_group_member(
    program: CpiHandle<'_>,
    accounts: TokenGroupMemberInitializeAccounts<'_>,
) -> Result<()> {
    initialize_token_group_member_signed(program, accounts, &[])
}

pub fn initialize_token_group_member_signed(
    program: CpiHandle<'_>,
    accounts: TokenGroupMemberInitializeAccounts<'_>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    super::validate_token_2022_program(&program)?;
    let TokenGroupMemberInitializeAccounts {
        member,
        member_mint,
        member_mint_authority,
        group,
        group_update_authority,
    } = accounts;

    let ix = spl_token_group_interface::instruction::initialize_member(
        &super::ix_addr(&program),
        &super::ix_addr(&member.info),
        &super::ix_addr(&member_mint),
        &super::ix_addr(&member_mint_authority),
        &super::ix_addr(&group.info),
        &super::ix_addr(&group_update_authority),
    );
    let cpi_accounts = [
        CpiHandle::from(member),
        member_mint,
        member_mint_authority,
        CpiHandle::from(group),
        group_update_authority,
        program,
    ];
    super::invoke_interface_instruction(&ix, &cpi_accounts, signer_seeds)
}
