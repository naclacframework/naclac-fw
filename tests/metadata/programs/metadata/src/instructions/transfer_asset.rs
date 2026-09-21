use naclac_lang::prelude::*;

/// Transfers an existing Asset to a new owner via
/// `naclac_metadata::transfer_asset_signed`.
#[derive(Accounts)]
pub struct TransferAsset {
    #[account(mut)]
    pub asset: Signer,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    /// The account whose pubkey becomes the asset's new owner.
    /// SAFETY: only its pubkey is read (passed readonly, non-signer, to the
    /// real Metaplex Core program) — it need not exist or be owned by
    /// anything in particular.
    pub new_owner: AccountInfo,

    /// SAFETY: the real Metaplex Core program — external to this workspace.
    pub mpl_core_program: AccountInfo,
}

pub fn transfer_asset(ctx: Context<TransferAsset>) -> Result {
    transfer_asset_signed(
        ctx.accounts.mpl_core_program.to_cpi_handle(),
        TransferAssetAccounts {
            asset: ctx.accounts.asset.to_cpi_handle_mut(),
            collection: None,
            payer: ctx.accounts.payer.to_cpi_handle_mut(),
            authority: None,
            new_owner: ctx.accounts.new_owner.to_cpi_handle(),
            system_program: Some(ctx.accounts.system_program.to_cpi_handle()),
            log_wrapper: None,
        },
        &[],
    )?;

    Ok(())
}
