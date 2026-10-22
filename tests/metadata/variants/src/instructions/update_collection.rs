use naclac_lang::prelude::*;

/// Updates an existing Collection's name/uri via
/// `naclac_metadata::update_collection_signed`.
#[derive(Accounts)]
pub struct UpdateCollection {
    #[account(mut)]
    pub collection: Signer,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    /// SAFETY: the real Metaplex Core program — external to this workspace.
    pub mpl_core_program: AccountInfo,
}

pub fn update_collection(
    ctx: Context<UpdateCollection>,
    new_name: ZcString,
    new_uri: ZcString,
) -> Result {
    let name: &str = &new_name;
    let uri: &str = &new_uri;

    update_collection_signed(
        ctx.accounts.mpl_core_program.to_cpi_handle(),
        UpdateCollectionAccounts {
            collection: ctx.accounts.collection.to_cpi_handle_mut(),
            payer: ctx.accounts.payer.to_cpi_handle_mut(),
            authority: None,
            new_update_authority: None,
            system_program: ctx.accounts.system_program.to_cpi_handle(),
            log_wrapper: None,
        },
        Some(name),
        Some(uri),
        &[],
    )?;

    Ok(())
}
