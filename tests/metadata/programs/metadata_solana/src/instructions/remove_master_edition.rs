use naclac_lang::prelude::*;

/// Removes an already-attached MasterEdition plugin from a Collection via
/// `naclac_metadata::remove_collection_plugin_signed`.
#[derive(Accounts)]
pub struct RemoveMasterEdition {
    #[account(mut)]
    pub collection: Signer,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    /// SAFETY: the real Metaplex Core program — external to this workspace.
    pub mpl_core_program: AccountInfo,
}

pub fn remove_master_edition(ctx: Context<RemoveMasterEdition>) -> Result {
    #[cfg(not(feature = "pinocchio"))]
    remove_collection_plugin_signed(
        ctx.accounts.mpl_core_program.to_cpi_handle(),
        AddCollectionPluginAccounts {
            collection: ctx.accounts.collection.to_cpi_handle_mut(),
            payer: ctx.accounts.payer.to_cpi_handle_mut(),
            authority: None,
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            log_wrapper: None,
        },
        plugin_type::MASTER_EDITION,
        &[],
    )?;

    #[cfg(feature = "pinocchio")]
    remove_collection_plugin_signed_pinocchio(
        ctx.accounts.mpl_core_program.to_cpi_handle(),
        AddCollectionPluginAccounts {
            collection: ctx.accounts.collection.to_cpi_handle_mut(),
            payer: ctx.accounts.payer.to_cpi_handle_mut(),
            authority: None,
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            log_wrapper: None,
        },
        plugin_type::MASTER_EDITION,
        &[],
    )?;

    Ok(())
}
