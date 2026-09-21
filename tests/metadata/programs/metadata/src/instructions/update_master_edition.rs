use naclac_lang::prelude::*;

/// Updates an already-attached MasterEdition plugin on a Collection (sets
/// `max_supply` to 100) via `naclac_metadata::update_collection_plugin_signed`.
#[derive(Accounts)]
pub struct UpdateMasterEdition {
    #[account(mut)]
    pub collection: Signer,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    /// SAFETY: the real Metaplex Core program — external to this workspace.
    pub mpl_core_program: AccountInfo,
}

pub fn update_master_edition(ctx: Context<UpdateMasterEdition>) -> Result {
    // UpdateCollectionPluginV1 discriminator (7) + MasterEdition tag +
    // `{ max_supply: Some(100), name: None, uri: None }`, no trailing
    // `init_authority` byte (see `plugin.rs::update_collection_plugin_signed`).
    let mut data = [7u8, plugin_type::MASTER_EDITION, 1u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8];
    data[3..7].copy_from_slice(&100u32.to_le_bytes());

    #[cfg(not(feature = "pinocchio"))]
    update_collection_plugin_signed(
        ctx.accounts.mpl_core_program.to_cpi_handle(),
        AddCollectionPluginAccounts {
            collection: ctx.accounts.collection.to_cpi_handle_mut(),
            payer: ctx.accounts.payer.to_cpi_handle_mut(),
            authority: None,
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            log_wrapper: None,
        },
        &data,
        &[],
    )?;

    #[cfg(feature = "pinocchio")]
    update_collection_plugin_signed_pinocchio(
        ctx.accounts.mpl_core_program.to_cpi_handle(),
        AddCollectionPluginAccounts {
            collection: ctx.accounts.collection.to_cpi_handle_mut(),
            payer: ctx.accounts.payer.to_cpi_handle_mut(),
            authority: None,
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            log_wrapper: None,
        },
        &data,
        &[],
    )?;

    Ok(())
}
