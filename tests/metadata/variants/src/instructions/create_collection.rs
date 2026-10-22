use naclac_lang::prelude::*;

/// Creates a new Metaplex Core `Collection` via
/// `naclac_metadata::create_collection_signed`, with `update_authority`
/// left at the real program's default (the payer).
#[derive(Accounts)]
pub struct CreateCollection {
    #[account(mut)]
    pub collection: Signer,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    /// SAFETY: the real Metaplex Core program — see `create_asset.rs`'s
    /// `mpl_core_program` field for why this is a bare `AccountInfo`.
    pub mpl_core_program: AccountInfo,
}

pub fn create_collection(ctx: Context<CreateCollection>, name: ZcString, uri: ZcString) -> Result {
    create_collection_signed(
        ctx.accounts.mpl_core_program.to_cpi_handle(),
        CreateCollectionAccounts {
            collection: ctx.accounts.collection.to_cpi_handle_mut(),
            update_authority: None,
            payer: ctx.accounts.payer.to_cpi_handle_mut(),
            system_program: ctx.accounts.system_program.to_cpi_handle(),
        },
        &name,
        &uri,
        &[],
    )?;
    Ok(())
}
