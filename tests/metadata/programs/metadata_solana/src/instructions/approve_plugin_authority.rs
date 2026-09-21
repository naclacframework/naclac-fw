use naclac_lang::prelude::*;

/// Approves a new authority to manage an already-attached FreezeDelegate
/// plugin on an Asset via
/// `naclac_metadata::approve_asset_plugin_authority_signed`.
#[derive(Accounts)]
pub struct ApprovePluginAuthority {
    #[account(mut)]
    pub asset: Signer,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    /// The account whose pubkey becomes the plugin's new authority.
    /// SAFETY: only its pubkey is read — it need not exist or be owned by
    /// anything in particular.
    pub new_authority: AccountInfo,

    /// SAFETY: the real Metaplex Core program — external to this workspace.
    pub mpl_core_program: AccountInfo,
}

pub fn approve_plugin_authority(ctx: Context<ApprovePluginAuthority>) -> Result {
    let new_authority_address = ctx.accounts.new_authority.address();

    approve_asset_plugin_authority_signed(
        ctx.accounts.mpl_core_program.to_cpi_handle(),
        AddAssetPluginAccounts {
            asset: ctx.accounts.asset.to_cpi_handle_mut(),
            collection: None,
            payer: ctx.accounts.payer.to_cpi_handle_mut(),
            authority: None,
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            log_wrapper: None,
        },
        plugin_type::FREEZE_DELEGATE,
        PluginAuthorityArg::Address(new_authority_address),
        &[],
    )?;

    Ok(())
}
