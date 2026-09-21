use naclac_lang::prelude::*;

/// Mints a new Metaplex Core `Asset` via `naclac_metadata::create_asset_signed`,
/// with `collection`/`authority`/`owner`/`update_authority`/`log_wrapper` all
/// left at their real-program defaults (`owner`/`update_authority` default to
/// `payer`, verified in `naclac-metadata/src/asset.rs`'s own header comment).
#[derive(Accounts)]
pub struct CreateAsset {
    #[account(mut)]
    pub asset: Signer,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    /// SAFETY: the real Metaplex Core program — external to this workspace,
    /// so there's no naclac `Program<T>` marker type to constrain it with;
    /// callers are expected to pass the real deployed program ID.
    pub mpl_core_program: AccountInfo,
}

pub fn create_asset(ctx: Context<CreateAsset>, name: ZcString, uri: ZcString) -> Result {
    create_asset_signed(
        ctx.accounts.mpl_core_program.to_cpi_handle(),
        CreateAssetAccounts {
            asset: ctx.accounts.asset.to_cpi_handle_mut(),
            collection: None,
            authority: None,
            payer: ctx.accounts.payer.to_cpi_handle_mut(),
            owner: None,
            update_authority: None,
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            log_wrapper: None,
        },
        &name,
        &uri,
        &[],
    )?;
    Ok(())
}
