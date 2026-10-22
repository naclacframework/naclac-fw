use naclac_lang::prelude::*;

/// Updates an existing Asset's name/uri via
/// `naclac_metadata::update_asset_signed`.
#[derive(Accounts)]
pub struct UpdateAsset {
    #[account(mut)]
    pub asset: Signer,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    /// SAFETY: the real Metaplex Core program — external to this workspace.
    pub mpl_core_program: AccountInfo,
}

pub fn update_asset(ctx: Context<UpdateAsset>, new_name: ZcString, new_uri: ZcString) -> Result {
    let name: &str = &new_name;
    let uri: &str = &new_uri;

    update_asset_signed(
        ctx.accounts.mpl_core_program.to_cpi_handle(),
        UpdateAssetAccounts {
            asset: ctx.accounts.asset.to_cpi_handle_mut(),
            collection: None,
            payer: ctx.accounts.payer.to_cpi_handle_mut(),
            authority: None,
            new_collection: None,
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            log_wrapper: None,
        },
        Some(name),
        Some(uri),
        None,
        &[],
    )?;

    Ok(())
}
