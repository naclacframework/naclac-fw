use naclac_lang::prelude::*;

/// Attaches an AppData external adapter to an existing Collection via
/// `attach_collection_app_data_signed`.
#[derive(Accounts)]
pub struct AttachCollectionAppData {
    #[account(mut)]
    pub collection: Signer,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    /// SAFETY: the real Metaplex Core program — external to this workspace.
    pub mpl_core_program: AccountInfo,
}

pub fn attach_collection_app_data(ctx: Context<AttachCollectionAppData>) -> Result {
    let data_authority = PluginAuthorityArg::UpdateAuthority;
    let schema = ExternalPluginAdapterSchemaArg::Json;

    attach_collection_app_data_signed(
        ctx.accounts.mpl_core_program.to_cpi_handle(),
        AddCollectionPluginAccounts {
            collection: ctx.accounts.collection.to_cpi_handle_mut(),
            payer: ctx.accounts.payer.to_cpi_handle_mut(),
            authority: None,
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            log_wrapper: None,
        },
        data_authority,
        schema,
        &[],
    )?;

    Ok(())
}
