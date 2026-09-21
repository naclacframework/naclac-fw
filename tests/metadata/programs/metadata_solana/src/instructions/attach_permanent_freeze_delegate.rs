use naclac_lang::prelude::*;

/// Attaches a PermanentFreezeDelegate plugin to an existing Asset via
/// `attach_permanent_freeze_delegate_signed`. Real mpl-core rejects this
/// unconditionally (`PermanentFreezeDelegate::validate_add_plugin`) —
/// Permanent* plugins may only be set at asset creation time, never via a
/// later `AddPluginV1` — so this instruction exists to exercise (and
/// confirm) that rejection, not to succeed.
#[derive(Accounts)]
pub struct AttachPermanentFreezeDelegate {
    #[account(mut)]
    pub asset: Signer,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    /// SAFETY: the real Metaplex Core program — external to this workspace.
    pub mpl_core_program: AccountInfo,
}

pub fn attach_permanent_freeze_delegate(ctx: Context<AttachPermanentFreezeDelegate>) -> Result {
    attach_permanent_freeze_delegate_signed(
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
