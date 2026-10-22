use naclac_lang::prelude::*;

/// Revokes the current authority of an already-attached MasterEdition
/// plugin on a Collection (reverting it to the plugin's default manager)
/// via `naclac_metadata::revoke_collection_plugin_authority_signed`.
#[derive(Accounts)]
pub struct RevokeCollectionPluginAuthority {
    #[account(mut)]
    pub collection: Signer,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    /// SAFETY: the real Metaplex Core program — external to this workspace.
    pub mpl_core_program: AccountInfo,
}

pub fn revoke_collection_plugin_authority(ctx: Context<RevokeCollectionPluginAuthority>) -> Result {
    revoke_collection_plugin_authority_signed(
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
