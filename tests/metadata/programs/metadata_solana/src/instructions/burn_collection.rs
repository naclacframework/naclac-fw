use naclac_lang::prelude::*;

/// Destroys an existing (empty) Collection via
/// `naclac_metadata::burn_collection_signed`.
#[derive(Accounts)]
pub struct BurnCollection {
    #[account(mut)]
    pub collection: Signer,

    #[account(mut)]
    pub payer: Signer,

    /// SAFETY: the real Metaplex Core program — external to this workspace.
    pub mpl_core_program: AccountInfo,
}

pub fn burn_collection(ctx: Context<BurnCollection>) -> Result {
    burn_collection_signed(
        ctx.accounts.mpl_core_program.to_cpi_handle(),
        BurnCollectionAccounts {
            collection: ctx.accounts.collection.to_cpi_handle_mut(),
            payer: ctx.accounts.payer.to_cpi_handle_mut(),
            authority: None,
            log_wrapper: None,
        },
        &[],
    )?;

    Ok(())
}
