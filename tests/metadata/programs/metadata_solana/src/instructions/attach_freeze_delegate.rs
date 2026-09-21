use naclac_lang::prelude::*;

/// Attaches a FreezeDelegate plugin to an existing Asset via
/// `attach_freeze_delegate_signed`.
#[derive(Accounts)]
pub struct AttachFreezeDelegate {
    #[account(mut)]
    pub asset: Signer,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    /// The external Metaplex Core program to CPI into.
    /// SAFETY: the real Metaplex Core program — external to this workspace.
    pub mpl_core_program: AccountInfo,
}

pub fn attach_freeze_delegate(ctx: Context<AttachFreezeDelegate>) -> Result {
    let frozen = false;

    attach_freeze_delegate_signed(
        ctx.accounts.mpl_core_program.to_cpi_handle(),
        AddAssetPluginAccounts {
            asset: ctx.accounts.asset.to_cpi_handle_mut(),
            collection: None,
            payer: ctx.accounts.payer.to_cpi_handle_mut(),
            authority: None,
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            log_wrapper: None,
        },
        frozen,
        &[],
    )?;

    Ok(())
}
