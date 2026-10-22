use naclac_lang::prelude::*;

/// Creates a new, empty `GroupV1` via `naclac_metadata::create_group_signed`,
/// with `update_authority` left at the real program's default (the payer).
#[derive(Accounts)]
pub struct CreateGroup {
    #[account(mut)]
    pub group: Signer,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    /// SAFETY: the real Metaplex Core program — external to this workspace.
    pub mpl_core_program: AccountInfo,
}

pub fn create_group(ctx: Context<CreateGroup>, name: ZcString, uri: ZcString) -> Result {
    create_group_signed(
        ctx.accounts.mpl_core_program.to_cpi_handle(),
        CreateGroupAccounts {
            group: ctx.accounts.group.to_cpi_handle_mut(),
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
