use naclac_lang::prelude::*;

/// Removes an already-attached AppData adapter from an Asset via
/// `naclac_metadata::remove_asset_app_data_signed`.
#[derive(Accounts)]
pub struct RemoveAppData {
    #[account(mut)]
    pub asset: Signer,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    /// SAFETY: the real Metaplex Core program — external to this workspace.
    pub mpl_core_program: AccountInfo,
}

pub fn remove_app_data(ctx: Context<RemoveAppData>) -> Result {
    remove_asset_app_data_signed(
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
        &[],
    )?;

    Ok(())
}
