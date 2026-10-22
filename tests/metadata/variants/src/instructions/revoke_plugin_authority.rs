use naclac_lang::prelude::*;

/// Revokes the current authority of an already-attached FreezeDelegate
/// plugin on an Asset (reverting it to the plugin's default manager) via
/// `naclac_metadata::revoke_asset_plugin_authority_signed`.
#[derive(Accounts)]
pub struct RevokePluginAuthority {
    #[account(mut)]
    pub asset: Signer,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    /// SAFETY: the real Metaplex Core program — external to this workspace.
    pub mpl_core_program: AccountInfo,
}

pub fn revoke_plugin_authority(ctx: Context<RevokePluginAuthority>) -> Result {
    revoke_asset_plugin_authority_signed(
        ctx.accounts.mpl_core_program.to_cpi_handle(),
        AddAssetPluginAccounts {
            asset: ctx.accounts.asset.to_cpi_handle_mut(),
            collection: None,
            payer: ctx.accounts.payer.to_cpi_handle_mut(),
            authority: None,
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            log_wrapper: None,
        },
        plugin_type::FREEZE_DELEGATE,
        &[],
    )?;

    Ok(())
}
