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
//! On `solana`, each concrete plugin file (`plugins/master_edition.rs`,
//! `plugins/edition.rs`, ...) builds the real, strongly-typed
//! `::mpl_core::types::Plugin` value itself and calls the real crate's own
//! `AddPluginV1`/`AddCollectionPluginV1` builder directly — there's little
//! to share there beyond what the real crate already provides. On
//! `pinocchio` there's no such builder to lean on, and every plugin needs
//! the identical account-list/CPI-invoke boilerplate around whatever raw
//! tag+payload bytes it produces, so that part genuinely is shared — the
//! two `*_pinocchio` functions below. Each concrete plugin file's own
//! per-backend wrapper (e.g. `attach_master_edition_signed`) is the only
//! thing callers actually see, with one uniform signature across both
//! backends, matching every other dual-backend CPI helper in this
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

/// `solana`-only: attaches a plugin to an `Asset` via a real `AddPluginV1`
/// CPI, using the real crate's own typed `Plugin` value directly.
#[cfg(not(feature = "pinocchio"))]
pub fn add_asset_plugin_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    plugin: ::mpl_core::types::Plugin,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let ix = ::mpl_core::instructions::AddPluginV1 {
        asset: accounts.asset.address(),
        collection: accounts.collection.as_ref().map(|c| c.address()),
        payer: accounts.payer.address(),
        authority: accounts.authority.as_ref().map(|a| a.address()),
        system_program: accounts.system_program.address(),
        log_wrapper: accounts.log_wrapper.as_ref().map(|l| l.address()),
    }
    .instruction(::mpl_core::instructions::AddPluginV1InstructionArgs {
        plugin,
        init_authority: None,
    });

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

/// `solana`-only: attaches a plugin to a `Collection` via a real
/// `AddCollectionPluginV1` CPI, using the real crate's own typed `Plugin`
/// value directly.
#[cfg(not(feature = "pinocchio"))]
pub fn add_collection_plugin_signed(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    plugin: ::mpl_core::types::Plugin,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let ix = ::mpl_core::instructions::AddCollectionPluginV1 {
        collection: accounts.collection.address(),
        payer: accounts.payer.address(),
        authority: accounts.authority.as_ref().map(|a| a.address()),
        system_program: accounts.system_program.address(),
        log_wrapper: accounts.log_wrapper.as_ref().map(|l| l.address()),
    }
    .instruction(::mpl_core::instructions::AddCollectionPluginV1InstructionArgs {
        plugin,
        init_authority: None,
    });

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
/// `AddPluginV1` CPI. `plugin_type` is the `Plugin`/`PluginType` enum's raw
/// tag byte (see `plugin_registry::plugin_type`); `plugin_payload` is that
/// variant's own already Borsh-encoded payload bytes, built by the caller.
#[cfg(feature = "pinocchio")]
pub fn add_asset_plugin_signed_pinocchio(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    plugin_type: u8,
    plugin_payload: &[u8],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let mut data = crate::prelude::Vec::with_capacity(3 + plugin_payload.len());
    data.push(2u8); // AddPluginV1 discriminator
    data.push(plugin_type);
    data.extend_from_slice(plugin_payload);
    data.push(0u8); // init_authority: None

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

/// `pinocchio`-only: attaches a plugin to a `Collection` via a hand-built
/// `AddCollectionPluginV1` CPI. See `add_asset_plugin_signed_pinocchio` for
/// `plugin_type`/`plugin_payload`.
#[cfg(feature = "pinocchio")]
pub fn add_collection_plugin_signed_pinocchio(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    plugin_type: u8,
    plugin_payload: &[u8],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let mut data = crate::prelude::Vec::with_capacity(3 + plugin_payload.len());
    data.push(3u8); // AddCollectionPluginV1 discriminator
    data.push(plugin_type);
    data.extend_from_slice(plugin_payload);
    data.push(0u8); // init_authority: None

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
// remove_plugin / remove_collection_plugin — same account shape as add_plugin
// ===========================================================================

/// `solana`-only: removes a plugin from an `Asset` via a real
/// `RemovePluginV1` CPI (discriminator `4`, verified against real
/// `mpl-core`) — same account shape as `AddPluginV1`, args are just the
/// `PluginType` to remove (no payload).
#[cfg(not(feature = "pinocchio"))]
pub fn remove_asset_plugin_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    plugin_type: ::mpl_core::types::PluginType,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let ix = ::mpl_core::instructions::RemovePluginV1 {
        asset: accounts.asset.address(),
        collection: accounts.collection.as_ref().map(|c| c.address()),
        payer: accounts.payer.address(),
        authority: accounts.authority.as_ref().map(|a| a.address()),
        system_program: accounts.system_program.address(),
        log_wrapper: accounts.log_wrapper.as_ref().map(|l| l.address()),
    }
    .instruction(::mpl_core::instructions::RemovePluginV1InstructionArgs { plugin_type });

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

/// `solana`-only: removes a plugin from a `Collection` via a real
/// `RemoveCollectionPluginV1` CPI (discriminator `5`).
#[cfg(not(feature = "pinocchio"))]
pub fn remove_collection_plugin_signed(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    plugin_type: ::mpl_core::types::PluginType,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let ix = ::mpl_core::instructions::RemoveCollectionPluginV1 {
        collection: accounts.collection.address(),
        payer: accounts.payer.address(),
        authority: accounts.authority.as_ref().map(|a| a.address()),
        system_program: accounts.system_program.address(),
        log_wrapper: accounts.log_wrapper.as_ref().map(|l| l.address()),
    }
    .instruction(::mpl_core::instructions::RemoveCollectionPluginV1InstructionArgs {
        plugin_type,
    });

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

/// `solana`-only: updates an existing plugin on an `Asset` via a real
/// `UpdatePluginV1` CPI (discriminator `6`, verified against real
/// `mpl-core`) — same account shape as `AddPluginV1`; args are the full new
/// `Plugin` value (tag + payload, no trailing `init_authority` byte).
#[cfg(not(feature = "pinocchio"))]
pub fn update_asset_plugin_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    plugin: ::mpl_core::types::Plugin,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let ix = ::mpl_core::instructions::UpdatePluginV1 {
        asset: accounts.asset.address(),
        collection: accounts.collection.as_ref().map(|c| c.address()),
        payer: accounts.payer.address(),
        authority: accounts.authority.as_ref().map(|a| a.address()),
        system_program: accounts.system_program.address(),
        log_wrapper: accounts.log_wrapper.as_ref().map(|l| l.address()),
    }
    .instruction(::mpl_core::instructions::UpdatePluginV1InstructionArgs { plugin });

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
/// hand-built `UpdatePluginV1` CPI. `plugin_type`/`plugin_payload` are the
/// new value's tag and payload bytes (same split as `add_asset_plugin_signed_pinocchio`).
#[cfg(feature = "pinocchio")]
pub fn update_asset_plugin_signed_pinocchio(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    plugin_type: u8,
    plugin_payload: &[u8],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let mut data = crate::prelude::Vec::with_capacity(2 + plugin_payload.len());
    data.push(6u8); // UpdatePluginV1 discriminator
    data.push(plugin_type);
    data.extend_from_slice(plugin_payload);

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

/// `solana`-only: updates an existing plugin on a `Collection` via a real
/// `UpdateCollectionPluginV1` CPI (discriminator `7`).
#[cfg(not(feature = "pinocchio"))]
pub fn update_collection_plugin_signed(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    plugin: ::mpl_core::types::Plugin,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let ix = ::mpl_core::instructions::UpdateCollectionPluginV1 {
        collection: accounts.collection.address(),
        payer: accounts.payer.address(),
        authority: accounts.authority.as_ref().map(|a| a.address()),
        system_program: accounts.system_program.address(),
        log_wrapper: accounts.log_wrapper.as_ref().map(|l| l.address()),
    }
    .instruction(::mpl_core::instructions::UpdateCollectionPluginV1InstructionArgs { plugin });

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
/// hand-built `UpdateCollectionPluginV1` CPI.
#[cfg(feature = "pinocchio")]
pub fn update_collection_plugin_signed_pinocchio(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    plugin_type: u8,
    plugin_payload: &[u8],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let mut data = crate::prelude::Vec::with_capacity(2 + plugin_payload.len());
    data.push(7u8); // UpdateCollectionPluginV1 discriminator
    data.push(plugin_type);
    data.extend_from_slice(plugin_payload);

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
