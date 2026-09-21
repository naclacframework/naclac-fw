use naclac_lang::prelude::*;

/// Updates the schema of an already-attached AppData adapter on an Asset
/// via `naclac_metadata::update_asset_app_data_schema_signed`.
#[derive(Accounts)]
pub struct UpdateAppData {
    #[account(mut)]
    pub asset: Signer,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    /// SAFETY: the real Metaplex Core program — external to this workspace.
    pub mpl_core_program: AccountInfo,
}

pub fn update_app_data(ctx: Context<UpdateAppData>) -> Result {
    update_asset_app_data_schema_signed(
        ctx.accounts.mpl_core_program.to_cpi_handle(),
        AddAssetPluginAccounts {
            asset: ctx.accounts.asset.to_cpi_handle_mut(),
            collection: None,
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
