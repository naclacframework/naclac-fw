use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT_AUTHORITY;

/// Real end-to-end proof of `permissioned_burn`
/// (`naclac-token/src/extensions/permissioned_burn.rs`) — burns real tokens
/// from `vault` via CPI, co-signed by both the mint's permissioned-burn
/// authority and the vault's own owner (here the same `mint_authority` PDA
/// for both, matching the pattern this test's mints/vaults already share),
/// then reads the vault's balance back to confirm it actually dropped, not
/// just that Token-2022 accepted the instruction. Calls `vault.reload()`
/// before that read — on the Borsh backend, `InterfaceAccount<TokenAccount>`
/// caches a deserialized copy at construction time and never re-reads it on
/// its own, so a post-CPI balance read without `reload()` would silently
/// return the pre-CPI value.
#[derive(Accounts)]
pub struct ExercisePermissionedBurn {
    #[account(seeds = [SEED_MINT_AUTHORITY], bump = mint_authority.bump)]
    pub mint_authority: Account<MintAuthority>,

    #[account(mut)]
    pub mint: InterfaceAccount<Mint>,

    #[account(mut)]
    pub vault: InterfaceAccount<TokenAccount>,

    pub token_program: Program<Token2022>,
}

pub fn exercise_permissioned_burn(ctx: Context<ExercisePermissionedBurn>, amount: u64) -> Result {
    let balance_before = ctx.accounts.vault.amount();

    let bump = ctx.accounts.mint_authority.bump;
    let signer_seeds: &[&[u8]] = &[SEED_MINT_AUTHORITY, &[bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];

    permissioned_burn_signed(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.vault.to_cpi_handle_mut(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        ctx.accounts.mint_authority.to_cpi_handle(),
        ctx.accounts.mint_authority.to_cpi_handle(),
        amount,
        signer,
    )?;

    ctx.accounts.vault.reload()?;
    let balance_after = ctx.accounts.vault.amount();
    if balance_after != balance_before.saturating_sub(amount) {
        return Err(NaclacError::ConstraintAddress.into());
    }

    Ok(())
}
