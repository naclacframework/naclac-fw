use naclac_lang::prelude::*;

/// Removes the accounts passed via `remaining_accounts` (Collection
/// accounts) from a Group via
/// `naclac_metadata::remove_collections_from_group_signed`.
#[derive(Accounts)]
pub struct RemoveCollectionsFromGroup {
    #[account(mut)]
    pub group: Signer,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    /// SAFETY: the real Metaplex Core program — external to this workspace.
    pub mpl_core_program: AccountInfo,
}

pub fn remove_collections_from_group(ctx: Context<RemoveCollectionsFromGroup>) -> Result {
    let items: Vec<CpiHandleMut<'_>> = ctx.remaining_accounts.iter().map(to_item).collect();

    remove_collections_from_group_signed(
        ctx.accounts.mpl_core_program.to_cpi_handle(),
        GroupRelationshipAccounts {
            group: ctx.accounts.group.to_cpi_handle_mut(),
            payer: ctx.accounts.payer.to_cpi_handle_mut(),
            authority: None,
            system_program: ctx.accounts.system_program.to_cpi_handle(),
        },
        &items,
        &[],
    )?;

    Ok(())
}

/// See `add_assets_to_group.rs`'s `to_item` doc comment.
#[cfg(not(feature = "pinocchio"))]
fn to_item(info: &AccountInfo) -> CpiHandleMut<'_> {
    CpiHandleMut {
        info: info.clone(),
        _phantom: core::marker::PhantomData,
    }
}
#[cfg(feature = "pinocchio")]
fn to_item(info: &AccountInfo) -> CpiHandleMut<'_> {
    CpiHandleMut {
        info: *info,
        _phantom: core::marker::PhantomData,
    }
}
