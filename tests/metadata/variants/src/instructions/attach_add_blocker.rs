use naclac_lang::prelude::*;

/// Attaches an AddBlocker plugin to an existing Asset via
/// `attach_add_blocker_signed`.
#[derive(Accounts)]
pub struct AttachAddBlocker {
    #[account(mut)]
    pub asset: Signer,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    /// SAFETY: the real Metaplex Core program — external to this workspace.
    pub mpl_core_program: AccountInfo,
}

pub fn attach_add_blocker(ctx: Context<AttachAddBlocker>) -> Result {
    attach_add_blocker_signed(
        ctx.accounts.mpl_core_program.to_cpi_handle(),
        AddAssetPluginAccounts {
            asset: ctx.accounts.asset.to_cpi_handle_mut(),
            collection: None,
            payer: ctx.accounts.payer.to_cpi_handle_mut(),
            authority: None,
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            log_wrapper: None,
        },
        &[],
    )?;

    Ok(())
}
