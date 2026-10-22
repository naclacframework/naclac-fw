use naclac_lang::prelude::*;

/// Attaches a FreezeExecute plugin to an existing Asset via
/// `attach_freeze_execute_signed`.
#[derive(Accounts)]
pub struct AttachFreezeExecute {
    #[account(mut)]
    pub asset: Signer,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    /// SAFETY: the real Metaplex Core program — external to this workspace.
    pub mpl_core_program: AccountInfo,
}

pub fn attach_freeze_execute(ctx: Context<AttachFreezeExecute>) -> Result {
    let frozen = false;

    attach_freeze_execute_signed(
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
