use naclac_lang::prelude::*;

/// Writes bytes to an already-attached AppData adapter on an Asset via
/// `naclac_metadata::write_asset_app_data_signed`.
#[derive(Accounts)]
pub struct WriteAppData {
    #[account(mut)]
    pub asset: Signer,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    /// SAFETY: the real Metaplex Core program — external to this workspace.
    pub mpl_core_program: AccountInfo,
}

pub fn write_app_data(ctx: Context<WriteAppData>) -> Result {
    write_asset_app_data_signed(
        ctx.accounts.mpl_core_program.to_cpi_handle(),
        WriteExternalAdapterAccounts {
            asset: ctx.accounts.asset.to_cpi_handle_mut(),
            collection: None,
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
