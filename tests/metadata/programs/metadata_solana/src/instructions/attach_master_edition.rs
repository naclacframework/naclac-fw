use naclac_lang::prelude::*;

/// Attaches a MasterEdition plugin to a `Collection` via
/// `naclac_metadata::attach_master_edition_signed`.
#[derive(Accounts)]
pub struct AttachMasterEdition {
    #[account(mut)]
    pub collection: Signer,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    /// The external Metaplex Core program to CPI into.
    /// SAFETY: the real Metaplex Core program — external to this workspace,
    /// so there's no naclac `Program<T>` marker type to constrain it with;
    /// callers are expected to pass the real deployed program ID.
    pub mpl_core_program: AccountInfo,
}

pub fn attach_master_edition(ctx: Context<AttachMasterEdition>) -> Result {
    attach_master_edition_signed(
        ctx.accounts.mpl_core_program.to_cpi_handle(),
        AddCollectionPluginAccounts {
            collection: ctx.accounts.collection.to_cpi_handle_mut(),
            payer: ctx.accounts.payer.to_cpi_handle_mut(),
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            authority: None,
            log_wrapper: None,
        },
        None,
        None,
        None,
        &[],
    )?;

    Ok(())
}
