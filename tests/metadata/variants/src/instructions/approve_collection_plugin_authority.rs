use naclac_lang::prelude::*;

/// Approves a new authority to manage an already-attached MasterEdition
/// plugin on a Collection via
/// `naclac_metadata::approve_collection_plugin_authority_signed`.
#[derive(Accounts)]
pub struct ApproveCollectionPluginAuthority {
    #[account(mut)]
    pub collection: Signer,

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

pub fn approve_collection_plugin_authority(
    ctx: Context<ApproveCollectionPluginAuthority>,
) -> Result {
    let new_authority_address = ctx.accounts.new_authority.address();

    approve_collection_plugin_authority_signed(
        ctx.accounts.mpl_core_program.to_cpi_handle(),
        AddCollectionPluginAccounts {
            collection: ctx.accounts.collection.to_cpi_handle_mut(),
            payer: ctx.accounts.payer.to_cpi_handle_mut(),
            authority: None,
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            log_wrapper: None,
        },
        plugin_type::MASTER_EDITION,
        PluginAuthorityArg::Address(new_authority_address),
        &[],
    )?;

    Ok(())
}
