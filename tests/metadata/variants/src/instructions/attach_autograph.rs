use naclac_lang::prelude::*;

/// Attaches an Autograph plugin to an existing Asset via
/// `attach_autograph_signed`.
#[derive(Accounts)]
pub struct AttachAutograph {
    #[account(mut)]
    pub asset: Signer,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    /// The external Metaplex Core program to CPI into.
    /// SAFETY: the real Metaplex Core program — external to this workspace.
    pub mpl_core_program: AccountInfo,
}

pub fn attach_autograph(ctx: Context<AttachAutograph>) -> Result {
    // The real processor's `resolve_authority` falls back to `payer` when no
    // explicit `authority` account is passed (`authority: None` below), and
    // `Autograph`'s own validation only accepts a new signature whose
    // `address` equals that resolved authority (self-signing only) —
    // verified in `programs/mpl-core/src/plugins/internal/owner_managed/autograph.rs`.
    let payer_address = ctx.accounts.payer.address();
    let signatures: &[(Address, &str)] = &[(payer_address, "hello")];

    attach_autograph_signed(
        ctx.accounts.mpl_core_program.to_cpi_handle(),
        AddAssetPluginAccounts {
            asset: ctx.accounts.asset.to_cpi_handle_mut(),
            collection: None,
            payer: ctx.accounts.payer.to_cpi_handle_mut(),
            authority: None,
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            log_wrapper: None,
        },
        signatures,
        &[],
    )?;

    Ok(())
}
