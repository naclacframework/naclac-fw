use naclac_lang::prelude::*;

/// Writes bytes to an already-attached AppData adapter on a Collection via
/// `naclac_metadata::write_collection_app_data_signed`.
#[derive(Accounts)]
pub struct WriteCollectionAppData {
    #[account(mut)]
    pub collection: Signer,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    /// SAFETY: the real Metaplex Core program — external to this workspace.
    pub mpl_core_program: AccountInfo,
}

pub fn write_collection_app_data(ctx: Context<WriteCollectionAppData>) -> Result {
    write_collection_app_data_signed(
        ctx.accounts.mpl_core_program.to_cpi_handle(),
        WriteCollectionExternalAdapterAccounts {
            collection: ctx.accounts.collection.to_cpi_handle_mut(),
            payer: ctx.accounts.payer.to_cpi_handle_mut(),
            authority: None,
            buffer: None,
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            log_wrapper: None,
        },
        PluginAuthorityArg::UpdateAuthority,
        b"hello",
        &[],
    )?;

    Ok(())
}
