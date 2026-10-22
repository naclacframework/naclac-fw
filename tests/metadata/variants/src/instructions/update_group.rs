use naclac_lang::prelude::*;

/// Updates an existing `GroupV1`'s name/uri via
/// `naclac_metadata::update_group_signed`.
#[derive(Accounts)]
pub struct UpdateGroup {
    #[account(mut)]
    pub group: Signer,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    /// SAFETY: the real Metaplex Core program — external to this workspace.
    pub mpl_core_program: AccountInfo,
}

pub fn update_group(ctx: Context<UpdateGroup>, new_name: ZcString, new_uri: ZcString) -> Result {
    let name: &str = &new_name;
    let uri: &str = &new_uri;

    update_group_signed(
        ctx.accounts.mpl_core_program.to_cpi_handle(),
        UpdateGroupAccounts {
            group: ctx.accounts.group.to_cpi_handle_mut(),
            payer: ctx.accounts.payer.to_cpi_handle_mut(),
            authority: None,
            new_update_authority: None,
            system_program: ctx.accounts.system_program.to_cpi_handle(),
        },
        Some(name),
        Some(uri),
        &[],
    )?;

    Ok(())
}
