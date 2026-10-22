use naclac_lang::prelude::*;

/// Attaches an AppData external adapter to an existing Asset via
/// `attach_asset_app_data_signed`.
#[derive(Accounts)]
pub struct AttachAppData {
    #[account(mut)]
    pub asset: Signer,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    /// The external Metaplex Core program to CPI into.
    /// SAFETY: the real Metaplex Core program — external to this workspace.
    pub mpl_core_program: AccountInfo,
}

pub fn attach_app_data(ctx: Context<AttachAppData>) -> Result {
    let data_authority = PluginAuthorityArg::UpdateAuthority;
    let schema = ExternalPluginAdapterSchemaArg::Json;

    attach_asset_app_data_signed(
        ctx.accounts.mpl_core_program.to_cpi_handle(),
        AddAssetPluginAccounts {
            asset: ctx.accounts.asset.to_cpi_handle_mut(),
            collection: None,
            payer: ctx.accounts.payer.to_cpi_handle_mut(),
            authority: None,
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            log_wrapper: None,
        },
        data_authority,
        schema,
        &[],
    )?;

    Ok(())
}
