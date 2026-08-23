// ===========================================================================
// plugin_authority.rs — approve/revoke plugin authority delegation
// ===========================================================================

//! Delegating (or reclaiming) management of a specific already-attached
//! plugin to another address, separate from the plugin's own attach/
//! remove/update lifecycle (`plugin.rs`). Real shapes verified against
//! `mpl-core`: `ApprovePluginAuthorityV1` (discriminator `8`) and
//! `RevokePluginAuthorityV1` (discriminator `10`) for `Asset`s,
//! `ApproveCollectionPluginAuthorityV1` (discriminator `9`) and
//! `RevokeCollectionPluginAuthorityV1` (discriminator `11`) for
//! `Collection`s — all four share the identical account shape as
//! `AddPluginV1`/`AddCollectionPluginV1`, so this file reuses
//! `AddAssetPluginAccounts`/`AddCollectionPluginAccounts` from `plugin.rs`
//! directly rather than redeclaring them. `Groups` is banned here too
//! (`"Groups plugins must be managed only via Group-specific instructions;
//! approve is not allowed"`, verified in the real processor), consistent
//! with every other generic plugin-management path in this crate.

use crate::prelude::*;

/// Backend-neutral mirror of the real `Authority`/`PluginAuthority` enum
/// (verified identical in shape: `None=0, Owner=1, UpdateAuthority=2,
/// Address{address}=3`) — used as the new authority to approve, and (via
/// `PluginAuthorityData`) as what a plugin's authority reads back as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginAuthorityArg {
    None,
    Owner,
    UpdateAuthority,
    Address(Address),
}

/// Approves `new_authority` to manage `plugin_type` (a raw `PluginType`
/// discriminant — see `plugin_type` module) on an `Asset` via a real
/// `ApprovePluginAuthorityV1` CPI.
pub fn approve_asset_plugin_authority_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    plugin_type: u8,
    new_authority: PluginAuthorityArg,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = ::mpl_core::instructions::ApprovePluginAuthorityV1 {
            asset: accounts.asset.address(),
            collection: accounts.collection.as_ref().map(|c| c.address()),
            payer: accounts.payer.address(),
            authority: accounts.authority.as_ref().map(|a| a.address()),
            system_program: accounts.system_program.address(),
            log_wrapper: accounts.log_wrapper.as_ref().map(|l| l.address()),
        }
        .instruction(::mpl_core::instructions::ApprovePluginAuthorityV1InstructionArgs {
            plugin_type: crate::plugin_type::to_real_plugin_type(plugin_type),
            new_authority: to_real_plugin_authority(new_authority),
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

    #[cfg(feature = "pinocchio")]
    {
        let mut data = crate::prelude::Vec::with_capacity(35);
        data.push(8u8); // ApprovePluginAuthorityV1 discriminator
        data.push(plugin_type);
        encode_plugin_authority(&mut data, new_authority);
        invoke_asset_plugin_authority_pinocchio(program, accounts, &data, signer_seeds)
    }
}

/// Approves `new_authority` to manage `plugin_type` (a raw `PluginType`
/// discriminant — see `plugin_type` module) on a `Collection` via a real
/// `ApproveCollectionPluginAuthorityV1` CPI.
pub fn approve_collection_plugin_authority_signed(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    plugin_type: u8,
    new_authority: PluginAuthorityArg,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = ::mpl_core::instructions::ApproveCollectionPluginAuthorityV1 {
            collection: accounts.collection.address(),
            payer: accounts.payer.address(),
            authority: accounts.authority.as_ref().map(|a| a.address()),
            system_program: accounts.system_program.address(),
            log_wrapper: accounts.log_wrapper.as_ref().map(|l| l.address()),
        }
        .instruction(
            ::mpl_core::instructions::ApproveCollectionPluginAuthorityV1InstructionArgs {
                plugin_type: crate::plugin_type::to_real_plugin_type(plugin_type),
                new_authority: to_real_plugin_authority(new_authority),
            },
        );

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

    #[cfg(feature = "pinocchio")]
    {
        let mut data = crate::prelude::Vec::with_capacity(35);
        data.push(9u8); // ApproveCollectionPluginAuthorityV1 discriminator
        data.push(plugin_type);
        encode_plugin_authority(&mut data, new_authority);
        invoke_collection_plugin_authority_pinocchio(program, accounts, &data, signer_seeds)
    }
}

/// Revokes whoever currently manages `plugin_type` (a raw `PluginType`
/// discriminant — see `plugin_type` module) on an `Asset`, reverting it to
/// the plugin's default manager, via a real `RevokePluginAuthorityV1` CPI.
pub fn revoke_asset_plugin_authority_signed(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    plugin_type: u8,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = ::mpl_core::instructions::RevokePluginAuthorityV1 {
            asset: accounts.asset.address(),
            collection: accounts.collection.as_ref().map(|c| c.address()),
            payer: accounts.payer.address(),
            authority: accounts.authority.as_ref().map(|a| a.address()),
            system_program: accounts.system_program.address(),
            log_wrapper: accounts.log_wrapper.as_ref().map(|l| l.address()),
        }
        .instruction(::mpl_core::instructions::RevokePluginAuthorityV1InstructionArgs {
            plugin_type: crate::plugin_type::to_real_plugin_type(plugin_type),
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

    #[cfg(feature = "pinocchio")]
    {
        let data = [10u8, plugin_type]; // RevokePluginAuthorityV1 discriminator + PluginType tag
        invoke_asset_plugin_authority_pinocchio(program, accounts, &data, signer_seeds)
    }
}

/// Revokes whoever currently manages `plugin_type` (a raw `PluginType`
/// discriminant — see `plugin_type` module) on a `Collection`, reverting it
/// to the plugin's default manager, via a real
/// `RevokeCollectionPluginAuthorityV1` CPI.
pub fn revoke_collection_plugin_authority_signed(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    plugin_type: u8,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let ix = ::mpl_core::instructions::RevokeCollectionPluginAuthorityV1 {
            collection: accounts.collection.address(),
            payer: accounts.payer.address(),
            authority: accounts.authority.as_ref().map(|a| a.address()),
            system_program: accounts.system_program.address(),
            log_wrapper: accounts.log_wrapper.as_ref().map(|l| l.address()),
        }
        .instruction(
            ::mpl_core::instructions::RevokeCollectionPluginAuthorityV1InstructionArgs {
                plugin_type: crate::plugin_type::to_real_plugin_type(plugin_type),
            },
        );

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

    #[cfg(feature = "pinocchio")]
    {
        let data = [11u8, plugin_type]; // RevokeCollectionPluginAuthorityV1 discriminator + PluginType tag
        invoke_collection_plugin_authority_pinocchio(program, accounts, &data, signer_seeds)
    }
}

#[cfg(not(feature = "pinocchio"))]
pub(crate) fn to_real_plugin_authority(
    authority: PluginAuthorityArg,
) -> ::mpl_core::types::PluginAuthority {
    match authority {
        PluginAuthorityArg::None => ::mpl_core::types::PluginAuthority::None,
        PluginAuthorityArg::Owner => ::mpl_core::types::PluginAuthority::Owner,
        PluginAuthorityArg::UpdateAuthority => ::mpl_core::types::PluginAuthority::UpdateAuthority,
        PluginAuthorityArg::Address(address) => {
            ::mpl_core::types::PluginAuthority::Address { address }
        }
    }
}

#[cfg(feature = "pinocchio")]
pub(crate) fn encode_plugin_authority(
    data: &mut crate::prelude::Vec<u8>,
    authority: PluginAuthorityArg,
) {
    match authority {
        PluginAuthorityArg::None => data.push(0u8),
        PluginAuthorityArg::Owner => data.push(1u8),
        PluginAuthorityArg::UpdateAuthority => data.push(2u8),
        PluginAuthorityArg::Address(address) => {
            data.push(3u8);
            data.extend_from_slice(address.as_ref());
        }
    }
}

/// `pinocchio`-only: shared account-list/CPI-invoke mechanics for
/// `ApprovePluginAuthorityV1`/`RevokePluginAuthorityV1` — same account
/// shape as `add_asset_plugin_signed_pinocchio` (`plugin.rs`), just with
/// caller-supplied `data` bytes.
#[cfg(feature = "pinocchio")]
fn invoke_asset_plugin_authority_pinocchio(
    program: CpiHandle<'_>,
    accounts: AddAssetPluginAccounts<'_>,
    data: &[u8],
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
        data,
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

/// `pinocchio`-only: shared account-list/CPI-invoke mechanics for
/// `ApproveCollectionPluginAuthorityV1`/`RevokeCollectionPluginAuthorityV1`
/// — same account shape as `add_collection_plugin_signed_pinocchio`
/// (`plugin.rs`), just with caller-supplied `data` bytes.
#[cfg(feature = "pinocchio")]
fn invoke_collection_plugin_authority_pinocchio(
    program: CpiHandle<'_>,
    accounts: AddCollectionPluginAccounts<'_>,
    data: &[u8],
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
        data,
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
