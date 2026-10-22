use naclac_lang::prelude::*;

/// Updates the schema of an already-attached AppData adapter on a
/// Collection via
/// `naclac_metadata::update_collection_app_data_schema_signed`.
#[derive(Accounts)]
pub struct UpdateCollectionAppData {
    #[account(mut)]
    pub collection: Signer,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    /// SAFETY: the real Metaplex Core program — external to this workspace.
    pub mpl_core_program: AccountInfo,
}

pub fn update_collection_app_data(ctx: Context<UpdateCollectionAppData>) -> Result {
    update_collection_app_data_schema_signed(
        ctx.accounts.mpl_core_program.to_cpi_handle(),
        AddCollectionPluginAccounts {
            collection: ctx.accounts.collection.to_cpi_handle_mut(),
            payer: ctx.accounts.payer.to_cpi_handle_mut(),
            authority: None,
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            log_wrapper: None,
        },
        PluginAuthorityArg::UpdateAuthority,
        ExternalPluginAdapterSchemaArg::Binary,
        &[],
    )?;

    Ok(())
}
