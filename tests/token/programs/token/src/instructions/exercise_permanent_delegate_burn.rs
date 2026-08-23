use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT_AUTHORITY;

/// Real enforcement proof for `PermanentDelegate` — not just that the
/// extension is readable, but that Token-2022 actually grants `mint`'s
/// permanent delegate the ability to `Burn` from *any* account holding the
/// mint's tokens, without that account's owner ever signing or having
/// approved a per-account delegate. Uses the ordinary `burn`/`burn_signed`
/// CPI (`naclac-token/src/token.rs`) with the mint's permanent-delegate
/// authority as the signer — `PermanentDelegate` is a mint-level
/// authorization rule Token-2022 itself enforces on the standard
/// `Burn`/`Transfer` instructions, not a separate CPI naclac needs to
/// special-case (confirmed: `permanent_delegate.rs` has no action CPI of
/// its own, matching anchor-spl-v2's own module).
#[derive(Accounts)]
pub struct ExercisePermanentDelegateBurn {
    #[account(seeds = [SEED_MINT_AUTHORITY], bump = mint_authority.bump)]
    pub mint_authority: Account<MintAuthority>,

    #[account(mut)]
    pub mint: InterfaceAccount<Mint>,

    #[account(mut)]
    pub vault: InterfaceAccount<TokenAccount>,

    pub token_program: Program<Token2022>,
}

#[instruction]
pub fn exercise_permanent_delegate_burn(
    ctx: Context<ExercisePermanentDelegateBurn>,
    amount: u64,
) -> Result {
    let bump = ctx.accounts.mint_authority.bump;
    let signer_seeds: &[&[u8]] = &[SEED_MINT_AUTHORITY, &[bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];

    naclac_lang::prelude::burn_signed(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        ctx.accounts.vault.to_cpi_handle_mut(),
        ctx.accounts.mint_authority.to_cpi_handle(),
        amount,
        signer,
    )?;

    Ok(())
}
