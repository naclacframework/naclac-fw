use naclac_lang::prelude::*;

/// Removes an already-attached AppData adapter from a Collection via
/// `naclac_metadata::remove_collection_app_data_signed`.
#[derive(Accounts)]
pub struct RemoveCollectionAppData {
    #[account(mut)]
    pub collection: Signer,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    /// SAFETY: the real Metaplex Core program — external to this workspace.
    pub mpl_core_program: AccountInfo,
}

pub fn remove_collection_app_data(ctx: Context<RemoveCollectionAppData>) -> Result {
    remove_collection_app_data_signed(
        ctx.accounts.mpl_core_program.to_cpi_handle(),
        AddCollectionPluginAccounts {
            collection: ctx.accounts.collection.to_cpi_handle_mut(),
            payer: ctx.accounts.payer.to_cpi_handle_mut(),
            authority: None,
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            log_wrapper: None,
        },
        PluginAuthorityArg::UpdateAuthority,
        &[],
    )?;

    Ok(())
}
