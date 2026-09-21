use naclac_lang::prelude::*;

/// Removes an already-attached FreezeDelegate plugin from an Asset via
/// `naclac_metadata::remove_asset_plugin_signed`.
#[derive(Accounts)]
pub struct RemoveFreezeDelegate {
    #[account(mut)]
    pub asset: Signer,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    /// SAFETY: the real Metaplex Core program — external to this workspace.
    pub mpl_core_program: AccountInfo,
}

pub fn remove_freeze_delegate(ctx: Context<RemoveFreezeDelegate>) -> Result {
    #[cfg(not(feature = "pinocchio"))]
    remove_asset_plugin_signed(
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

    #[cfg(feature = "pinocchio")]
    remove_asset_plugin_signed_pinocchio(
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
