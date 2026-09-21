use naclac_lang::prelude::*;

/// Attaches a PermanentFreezeExecute plugin to an existing Asset via
/// `attach_permanent_freeze_execute_signed`. Real mpl-core rejects this
/// unconditionally (`PermanentFreezeExecute::validate_add_plugin`) —
/// Permanent* plugins may only be set at asset creation time, never via a
/// later `AddPluginV1` — so this instruction exists to exercise (and
/// confirm) that rejection, not to succeed.
#[derive(Accounts)]
pub struct AttachPermanentFreezeExecute {
    #[account(mut)]
    pub asset: Signer,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    /// SAFETY: the real Metaplex Core program — external to this workspace.
    pub mpl_core_program: AccountInfo,
}

pub fn attach_permanent_freeze_execute(ctx: Context<AttachPermanentFreezeExecute>) -> Result {
    attach_permanent_freeze_execute_signed(
        ctx.accounts.mpl_core_program.to_cpi_handle(),
        AddAssetPluginAccounts {
            asset: ctx.accounts.asset.to_cpi_handle_mut(),
            collection: None,
            payer: ctx.accounts.payer.to_cpi_handle_mut(),
            authority: None,
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            log_wrapper: None,
        },
        false,
        &[],
    )?;

    Ok(())
}
