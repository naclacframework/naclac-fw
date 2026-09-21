// ===========================================================================
// plugin.rs — shared `add_plugin`/`add_collection_plugin` CPI mechanics
// ===========================================================================

//! The account-list and CPI-invoke mechanics of attaching a plugin to an
//! `Asset` (`AddPluginV1`, discriminator `2`, verified against real
//! `mpl-core`) or a `Collection` (`AddCollectionPluginV1`, discriminator
//! `3`) are identical no matter which of the 19 plugin types is being
//! attached — only the `Plugin` enum's own tag and payload bytes differ per
//! plugin.
//!
//! Both backends hand-build the raw instruction bytes (discriminator +
//! `PluginType` tag + the variant's own Borsh-encoded payload) rather than
//! going through the real `mpl-core` crate's own typed builders — `solana`
//! can't depend on that crate at all (its `hooked` module, unconditionally
//! compiled in with no feature gate, blows Solana's 4096-byte per-function
//! BPF stack limit; see `docs/07-solana-backend-stack-overflow-fix.md`), so
//! every plugin needs the identical account-list/CPI-invoke boilerplate
//! around whatever raw bytes it produces on *either* backend — that's what
//! `add`/`remove`/`update` × asset/collection share, one implementation per
//! backend. Each concrete plugin file's own
//! per-backend wrapper (e.g. `attach_master_edition_signed`) builds only the
//! payload bytes itself and calls these, with one uniform signature across
//! both backends, matching every other dual-backend CPI helper in this
//! framework.
//!
//! `init_authority` (who can manage the plugin after it's attached) is
//! fixed to `None` here — Core's own default (the asset/collection's update
//! authority) — for every plugin this crate attaches. Exposing a way to
//! override it is deferred until a concrete need for it shows up.

use crate::prelude::*;

/// Accounts consumed when attaching a plugin to an `Asset` — mirrors the
/// real `AddPluginV1` instruction's accounts exactly. Also reused (same
/// name, same shape) for `remove_asset_plugin_signed`/
/// `update_asset_plugin_signed` below: `RemovePluginV1`/`UpdatePluginV1`
/// both use this identical account list, verified against real `mpl-core`.
pub struct AddAssetPluginAccounts<'a> {
    pub asset: CpiHandleMut<'a>,
    pub collection: Option<CpiHandleMut<'a>>,
    pub payer: CpiHandleMut<'a>,
    pub authority: Option<CpiHandle<'a>>,
    pub system_program: CpiHandle<'a>,
    pub log_wrapper: Option<CpiHandle<'a>>,
}

/// Accounts consumed when attaching a plugin to a `Collection` — mirrors
/// the real `AddCollectionPluginV1` instruction's accounts exactly. Also
/// reused for `remove_collection_plugin_signed`/
/// `update_collection_plugin_signed` below — see `AddAssetPluginAccounts`'s
/// doc comment.
pub struct AddCollectionPluginAccounts<'a> {
    pub collection: CpiHandleMut<'a>,
    pub payer: CpiHandleMut<'a>,
    pub authority: Option<CpiHandle<'a>>,
    pub system_program: CpiHandle<'a>,
    pub log_wrapper: Option<CpiHandle<'a>>,
}

/// `solana`-only: attaches a plugin to an `Asset` via a hand-built
/// `AddPluginV1` CPI. `ix_data` is the complete instruction data —
/// discriminator `2` + `PluginType` tag + the variant's own Borsh-encoded
/// payload + trailing `init_authority: None` byte — built by the caller
/// (each concrete plugin file, e.g. `plugins/master_edition.rs`), same
/// layout as the `pinocchio` sibling of this function. Account order/flags
/// verified against the real `mpl-core` crate's own generated
/// `instructions/add_plugin_v1.rs`.
#[cfg(not(feature = "pinocchio"))]
pub fn add_asset_plugin_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    ix_data: &[u8],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
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
        data: ix_data.to_vec(),
    };

    let cpi_accounts = [
        CpiHandle::from(accounts.asset),
        accounts
            .collection
            .map(CpiHandle::from)
            .unwrap_or_else(|| program.clone()),
        CpiHandle::from(accounts.payer),
        accounts.authority.unwrap_or_else(|| program.clone()),
        accounts.system_program,
        accounts.log_wrapper.unwrap_or_else(|| program.clone()),
        program,
    ];
    crate::cpi::invoke_signed(&ix, &cpi_accounts, signer_seeds)
}

/// `solana`-only: attaches a plugin to a `Collection` via a hand-built
/// `AddCollectionPluginV1` CPI. `ix_data` is the complete instruction data —
/// discriminator `3` + `PluginType` tag + the variant's own Borsh-encoded
/// payload + trailing `init_authority: None` byte — built by the caller,
/// same layout as the `pinocchio` sibling of this function. Account
/// order/flags verified against the real `mpl-core` crate's own generated
/// `instructions/add_collection_plugin_v1.rs`.
#[cfg(not(feature = "pinocchio"))]
pub fn add_collection_plugin_signed(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    ix_data: &[u8],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let accounts_meta = vec![
        solana_program::instruction::AccountMeta::new(accounts.collection.address(), false),
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
        data: ix_data.to_vec(),
    };

    let cpi_accounts = [
        CpiHandle::from(accounts.collection),
        CpiHandle::from(accounts.payer),
        accounts.authority.unwrap_or_else(|| program.clone()),
        accounts.system_program,
        accounts.log_wrapper.unwrap_or_else(|| program.clone()),
        program,
    ];
    crate::cpi::invoke_signed(&ix, &cpi_accounts, signer_seeds)
}

/// `pinocchio`-only: attaches a plugin to an `Asset` via a hand-built
/// `AddPluginV1` CPI. `ix_data` is the complete instruction data —
/// discriminator `2` + `PluginType` tag + the variant's own Borsh-encoded
/// payload + trailing `init_authority: None` byte — built by the caller
/// (each concrete plugin file, e.g. `plugins/master_edition.rs`) into its
/// own `FixedBuf`, since only the caller knows its own payload's exact
/// compile-time bound (see `fixed_buf.rs`'s header for why this layer
/// can't build it generically).
#[cfg(feature = "pinocchio")]
pub fn add_asset_plugin_signed_pinocchio(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    ix_data: &[u8],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let asset_handle: CpiHandle<'_> = CpiHandle::from(accounts.asset);
    let payer_handle: CpiHandle<'_> = CpiHandle::from(accounts.payer);
    let collection_is_some = accounts.collection.is_some();
    let authority_is_some = accounts.authority.is_some();
    let collection_handle = accounts.collection.map(CpiHandle::from).unwrap_or(program);
    let authority_handle = accounts.authority.unwrap_or(program);
    let log_wrapper_handle = accounts.log_wrapper.unwrap_or(program);

    let ix_accounts = [
        ::pinocchio::instruction::InstructionAccount::writable(asset_handle.info.view.address()),
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
            accounts.system_program.info.view.address(),
        ),
        ::pinocchio::instruction::InstructionAccount::readonly(
            log_wrapper_handle.info.view.address(),
        ),
    ];
    let instruction = ::pinocchio::instruction::InstructionView {
        program_id: program.info.view.address(),
        accounts: &ix_accounts,
        data: ix_data,
    };
    let handles = [
        asset_handle,
        collection_handle,
        payer_handle,
        authority_handle,
        accounts.system_program,
        log_wrapper_handle,
    ];
    crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles, signer_seeds)
}

/// `pinocchio`-only: attaches a plugin to a `Collection` via a hand-built
/// `AddCollectionPluginV1` CPI. `ix_data` is the complete instruction data
/// (discriminator `3` + tag + payload + trailing `init_authority: None`
/// byte), built by the caller — see `add_asset_plugin_signed_pinocchio`.
#[cfg(feature = "pinocchio")]
pub fn add_collection_plugin_signed_pinocchio(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    ix_data: &[u8],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let collection_handle: CpiHandle<'_> = CpiHandle::from(accounts.collection);
    let payer_handle: CpiHandle<'_> = CpiHandle::from(accounts.payer);
    let authority_is_some = accounts.authority.is_some();
    let authority_handle = accounts.authority.unwrap_or(program);
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
            accounts.system_program.info.view.address(),
        ),
        ::pinocchio::instruction::InstructionAccount::readonly(
            log_wrapper_handle.info.view.address(),
        ),
    ];
    let instruction = ::pinocchio::instruction::InstructionView {
        program_id: program.info.view.address(),
        accounts: &ix_accounts,
        data: ix_data,
    };
    let handles = [
        collection_handle,
        payer_handle,
        authority_handle,
        accounts.system_program,
        log_wrapper_handle,
    ];
    crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles, signer_seeds)
}

// ===========================================================================
// remove_plugin / remove_collection_plugin — same account shape as add_plugin
// ===========================================================================

/// `solana`-only: removes a plugin from an `Asset` via a hand-built
/// `RemovePluginV1` CPI (discriminator `4`, verified against real
/// `mpl-core`) — same account shape as `AddPluginV1`, args are just the
/// `PluginType` raw tag byte (see `plugin_registry::plugin_type`), no
/// payload.
#[cfg(not(feature = "pinocchio"))]
pub fn remove_asset_plugin_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    plugin_type: u8,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
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
        data: vec![4u8, plugin_type], // RemovePluginV1 discriminator + PluginType tag
    };

    let cpi_accounts = [
        CpiHandle::from(accounts.asset),
        accounts
            .collection
            .map(CpiHandle::from)
            .unwrap_or_else(|| program.clone()),
        CpiHandle::from(accounts.payer),
        accounts.authority.unwrap_or_else(|| program.clone()),
        accounts.system_program,
        accounts.log_wrapper.unwrap_or_else(|| program.clone()),
        program,
    ];
    crate::cpi::invoke_signed(&ix, &cpi_accounts, signer_seeds)
}

/// `pinocchio`-only: removes a plugin from an `Asset` via a hand-built
/// `RemovePluginV1` CPI. `plugin_type` is the `PluginType` enum's raw tag
/// byte (see `plugin_registry::plugin_type`).
#[cfg(feature = "pinocchio")]
pub fn remove_asset_plugin_signed_pinocchio(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    plugin_type: u8,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let data = [4u8, plugin_type]; // RemovePluginV1 discriminator + PluginType tag

    let asset_handle: CpiHandle<'_> = CpiHandle::from(accounts.asset);
    let payer_handle: CpiHandle<'_> = CpiHandle::from(accounts.payer);
    let collection_is_some = accounts.collection.is_some();
    let authority_is_some = accounts.authority.is_some();
    let collection_handle = accounts.collection.map(CpiHandle::from).unwrap_or(program);
    let authority_handle = accounts.authority.unwrap_or(program);
    let log_wrapper_handle = accounts.log_wrapper.unwrap_or(program);

    let ix_accounts = [
        ::pinocchio::instruction::InstructionAccount::writable(asset_handle.info.view.address()),
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
            accounts.system_program.info.view.address(),
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
        accounts.system_program,
        log_wrapper_handle,
    ];
    crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles, signer_seeds)
}

/// `solana`-only: removes a plugin from a `Collection` via a hand-built
/// `RemoveCollectionPluginV1` CPI (discriminator `5`). `plugin_type` is the
/// `PluginType` enum's raw tag byte.
#[cfg(not(feature = "pinocchio"))]
pub fn remove_collection_plugin_signed(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    plugin_type: u8,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let accounts_meta = vec![
        solana_program::instruction::AccountMeta::new(accounts.collection.address(), false),
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
        data: vec![5u8, plugin_type], // RemoveCollectionPluginV1 discriminator + PluginType tag
    };

    let cpi_accounts = [
        CpiHandle::from(accounts.collection),
        CpiHandle::from(accounts.payer),
        accounts.authority.unwrap_or_else(|| program.clone()),
        accounts.system_program,
        accounts.log_wrapper.unwrap_or_else(|| program.clone()),
        program,
    ];
    crate::cpi::invoke_signed(&ix, &cpi_accounts, signer_seeds)
}

/// `pinocchio`-only: removes a plugin from a `Collection` via a hand-built
/// `RemoveCollectionPluginV1` CPI.
#[cfg(feature = "pinocchio")]
pub fn remove_collection_plugin_signed_pinocchio(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    plugin_type: u8,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let data = [5u8, plugin_type]; // RemoveCollectionPluginV1 discriminator + PluginType tag

    let collection_handle: CpiHandle<'_> = CpiHandle::from(accounts.collection);
    let payer_handle: CpiHandle<'_> = CpiHandle::from(accounts.payer);
    let authority_is_some = accounts.authority.is_some();
    let authority_handle = accounts.authority.unwrap_or(program);
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
            accounts.system_program.info.view.address(),
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
        collection_handle,
        payer_handle,
        authority_handle,
        accounts.system_program,
        log_wrapper_handle,
    ];
    crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles, signer_seeds)
}

// ===========================================================================
// update_plugin / update_collection_plugin — same account shape as add_plugin
// ===========================================================================

/// `solana`-only: updates an existing plugin on an `Asset` via a hand-built
/// `UpdatePluginV1` CPI (discriminator `6`, verified against real
/// `mpl-core`) — same account shape as `AddPluginV1`. `ix_data` is the
/// complete instruction data (discriminator `6` + `PluginType` tag +
/// payload, no trailing `init_authority` byte), built by the caller, same
/// layout as the `pinocchio` sibling of this function.
#[cfg(not(feature = "pinocchio"))]
pub fn update_asset_plugin_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    ix_data: &[u8],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
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
        data: ix_data.to_vec(),
    };

    let cpi_accounts = [
        CpiHandle::from(accounts.asset),
        accounts
            .collection
            .map(CpiHandle::from)
            .unwrap_or_else(|| program.clone()),
        CpiHandle::from(accounts.payer),
        accounts.authority.unwrap_or_else(|| program.clone()),
        accounts.system_program,
        accounts.log_wrapper.unwrap_or_else(|| program.clone()),
        program,
    ];
    crate::cpi::invoke_signed(&ix, &cpi_accounts, signer_seeds)
}

/// `pinocchio`-only: updates an existing plugin on an `Asset` via a
/// hand-built `UpdatePluginV1` CPI. `ix_data` is the complete instruction
/// data (discriminator `6` + tag + payload), built by the caller — see
/// `add_asset_plugin_signed_pinocchio`.
#[cfg(feature = "pinocchio")]
pub fn update_asset_plugin_signed_pinocchio(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    ix_data: &[u8],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let asset_handle: CpiHandle<'_> = CpiHandle::from(accounts.asset);
    let payer_handle: CpiHandle<'_> = CpiHandle::from(accounts.payer);
    let collection_is_some = accounts.collection.is_some();
    let authority_is_some = accounts.authority.is_some();
    let collection_handle = accounts.collection.map(CpiHandle::from).unwrap_or(program);
    let authority_handle = accounts.authority.unwrap_or(program);
    let log_wrapper_handle = accounts.log_wrapper.unwrap_or(program);

    let ix_accounts = [
        ::pinocchio::instruction::InstructionAccount::writable(asset_handle.info.view.address()),
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
            accounts.system_program.info.view.address(),
        ),
        ::pinocchio::instruction::InstructionAccount::readonly(
            log_wrapper_handle.info.view.address(),
        ),
    ];
    let instruction = ::pinocchio::instruction::InstructionView {
        program_id: program.info.view.address(),
        accounts: &ix_accounts,
        data: ix_data,
    };
    let handles = [
        asset_handle,
        collection_handle,
        payer_handle,
        authority_handle,
        accounts.system_program,
        log_wrapper_handle,
    ];
    crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles, signer_seeds)
}

/// `solana`-only: updates an existing plugin on a `Collection` via a
/// hand-built `UpdateCollectionPluginV1` CPI (discriminator `7`). `ix_data`
/// is the complete instruction data, built by the caller — see
/// `update_asset_plugin_signed`.
#[cfg(not(feature = "pinocchio"))]
pub fn update_collection_plugin_signed(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    ix_data: &[u8],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let accounts_meta = vec![
        solana_program::instruction::AccountMeta::new(accounts.collection.address(), false),
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
        data: ix_data.to_vec(),
    };

    let cpi_accounts = [
        CpiHandle::from(accounts.collection),
        CpiHandle::from(accounts.payer),
        accounts.authority.unwrap_or_else(|| program.clone()),
        accounts.system_program,
        accounts.log_wrapper.unwrap_or_else(|| program.clone()),
        program,
    ];
    crate::cpi::invoke_signed(&ix, &cpi_accounts, signer_seeds)
}

/// `pinocchio`-only: updates an existing plugin on a `Collection` via a
/// hand-built `UpdateCollectionPluginV1` CPI. `ix_data` is the complete
/// instruction data (discriminator `7` + tag + payload), built by the
/// caller — see `add_asset_plugin_signed_pinocchio`.
#[cfg(feature = "pinocchio")]
pub fn update_collection_plugin_signed_pinocchio(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    ix_data: &[u8],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let collection_handle: CpiHandle<'_> = CpiHandle::from(accounts.collection);
    let payer_handle: CpiHandle<'_> = CpiHandle::from(accounts.payer);
    let authority_is_some = accounts.authority.is_some();
    let authority_handle = accounts.authority.unwrap_or(program);
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
            accounts.system_program.info.view.address(),
        ),
        ::pinocchio::instruction::InstructionAccount::readonly(
            log_wrapper_handle.info.view.address(),
        ),
    ];
    let instruction = ::pinocchio::instruction::InstructionView {
        program_id: program.info.view.address(),
        accounts: &ix_accounts,
        data: ix_data,
    };
    let handles = [
        collection_handle,
        payer_handle,
        authority_handle,
        accounts.system_program,
        log_wrapper_handle,
    ];
    crate::cpi::invoke_signed_pinocchio_handles(&instruction, &handles, signer_seeds)
}
