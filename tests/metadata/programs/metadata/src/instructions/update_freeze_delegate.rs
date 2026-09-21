use naclac_lang::prelude::*;

/// Updates an already-attached FreezeDelegate plugin on an Asset to frozen
/// via `naclac_metadata::update_asset_plugin_signed`.
#[derive(Accounts)]
pub struct UpdateFreezeDelegate {
    #[account(mut)]
    pub asset: Signer,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    /// SAFETY: the real Metaplex Core program — external to this workspace.
    pub mpl_core_program: AccountInfo,
}

pub fn update_freeze_delegate(ctx: Context<UpdateFreezeDelegate>) -> Result {
    // UpdatePluginV1 discriminator (6) + FreezeDelegate tag + `{ frozen: true }`,
    // no trailing `init_authority` byte (see `plugin.rs::update_asset_plugin_signed`).
    let data = [6u8, plugin_type::FREEZE_DELEGATE, 1u8];

    #[cfg(not(feature = "pinocchio"))]
    update_asset_plugin_signed(
        ctx.accounts.mpl_core_program.to_cpi_handle(),
        AddAssetPluginAccounts {
            asset: ctx.accounts.asset.to_cpi_handle_mut(),
            collection: None,
            payer: ctx.accounts.payer.to_cpi_handle_mut(),
            authority: None,
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            log_wrapper: None,
        },
        &data,
        &[],
    )?;

    #[cfg(feature = "pinocchio")]
    update_asset_plugin_signed_pinocchio(
        ctx.accounts.mpl_core_program.to_cpi_handle(),
        AddAssetPluginAccounts {
            asset: ctx.accounts.asset.to_cpi_handle_mut(),
            collection: None,
            payer: ctx.accounts.payer.to_cpi_handle_mut(),
            authority: None,
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            log_wrapper: None,
        },
        &data,
        &[],
    )?;

    Ok(())
}
