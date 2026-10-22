use naclac_lang::prelude::*;

/// Closes an existing (empty) `GroupV1` via
/// `naclac_metadata::close_group_signed`, returning its lamports to `payer`.
#[derive(Accounts)]
pub struct CloseGroup {
    #[account(mut)]
    pub group: Signer,

    #[account(mut)]
    pub payer: Signer,

    /// SAFETY: the real Metaplex Core program — external to this workspace.
    pub mpl_core_program: AccountInfo,
}

pub fn close_group(ctx: Context<CloseGroup>) -> Result {
    close_group_signed(
        ctx.accounts.mpl_core_program.to_cpi_handle(),
        CloseGroupAccounts {
            group: ctx.accounts.group.to_cpi_handle_mut(),
            payer: ctx.accounts.payer.to_cpi_handle_mut(),
            authority: None,
        },
        &[],
    )?;

    Ok(())
}
