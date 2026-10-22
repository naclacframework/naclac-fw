use naclac_lang::prelude::*;

/// Attaches a VerifiedCreators plugin to an existing Asset via
/// `naclac_metadata::attach_verified_creators_signed`.
#[derive(Accounts)]
pub struct AttachVerifiedCreators {
    #[account(mut)]
    pub asset: Signer,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    /// The external Metaplex Core program to CPI into.
    /// SAFETY: the real Metaplex Core program — external to this workspace,
    /// so there's no naclac `Program<T>` marker type to constrain it with;
    /// callers are expected to pass the real deployed program ID.
    pub mpl_core_program: AccountInfo,
}

pub fn attach_verified_creators(ctx: Context<AttachVerifiedCreators>) -> Result {
    // For tests we attach with an empty signatures list.
    attach_verified_creators_signed(
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
        &[],
    )?;

    Ok(())
}
