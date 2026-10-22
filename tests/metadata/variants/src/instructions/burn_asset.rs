use naclac_lang::prelude::*;

/// Destroys an existing Asset via `naclac_metadata::burn_asset_signed`.
#[derive(Accounts)]
pub struct BurnAsset {
    #[account(mut)]
    pub asset: Signer,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    /// SAFETY: the real Metaplex Core program — external to this workspace.
    pub mpl_core_program: AccountInfo,
}

pub fn burn_asset(ctx: Context<BurnAsset>) -> Result {
    burn_asset_signed(
        ctx.accounts.mpl_core_program.to_cpi_handle(),
        BurnAssetAccounts {
            asset: ctx.accounts.asset.to_cpi_handle_mut(),
            collection: None,
            payer: ctx.accounts.payer.to_cpi_handle_mut(),
            authority: None,
            system_program: Some(ctx.accounts.system_program.to_cpi_handle()),
            log_wrapper: None,
        },
        &[],
    )?;

    Ok(())
}
