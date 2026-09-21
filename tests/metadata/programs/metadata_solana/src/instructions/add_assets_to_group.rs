use naclac_lang::prelude::*;

/// Adds the accounts passed via `remaining_accounts` (Asset accounts) to a
/// Group via `naclac_metadata::add_assets_to_group_signed`.
#[derive(Accounts)]
pub struct AddAssetsToGroup {
    #[account(mut)]
    pub group: Signer,

    #[account(mut)]
    pub payer: Signer,

    pub system_program: Program<System>,

    /// SAFETY: the real Metaplex Core program — external to this workspace.
    pub mpl_core_program: AccountInfo,
}

pub fn add_assets_to_group(ctx: Context<AddAssetsToGroup>) -> Result {
    let items: Vec<CpiHandleMut<'_>> = ctx.remaining_accounts.iter().map(to_item).collect();

    add_assets_to_group_signed(
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

/// Copies a remaining-account's `AccountInfo` into a `CpiHandleMut` — a
/// plain data copy (`AccountInfo` is `Copy` on `pinocchio`, `Clone` on
/// `solana`), not a real exclusive Rust borrow; matches
/// `naclac-metadata`'s own `execute.rs::ExecutePayer::AssetSignerPda`
/// branch, which builds a `CpiHandleMut` the same way from a shared
/// reference.
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
