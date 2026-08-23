use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT_AUTHORITY;

/// Real end-to-end proof of `update_interest_bearing_mint_rate`
/// (`naclac-token/src/extensions/interest_bearing_mint.rs`) — updates an
/// already-initialized mint's interest rate via CPI, signed by the mint's
/// rate authority, then reads it back to confirm the on-chain value
/// actually changed, not just that Token-2022 accepted the instruction.
#[derive(Accounts)]
pub struct ExerciseInterestBearingMintUpdateRate {
    #[account(seeds = [SEED_MINT_AUTHORITY], bump = mint_authority.bump)]
    pub mint_authority: Account<MintAuthority>,

    #[account(mut)]
    pub mint: InterfaceAccount<Mint>,

    pub token_program: Program<Token2022>,
}

#[instruction]
pub fn exercise_interest_bearing_mint_update_rate(
    ctx: Context<ExerciseInterestBearingMintUpdateRate>,
    new_rate: i16,
) -> Result {
    let bump = ctx.accounts.mint_authority.bump;
    let signer_seeds: &[&[u8]] = &[SEED_MINT_AUTHORITY, &[bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];

    update_interest_bearing_mint_rate_signed(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        ctx.accounts.mint_authority.to_cpi_handle(),
        new_rate,
        signer,
    )?;

    let config: InterestBearingConfig = ctx.accounts.mint.get_extension()?;
    if config.current_rate() != new_rate {
        return Err(NaclacError::ConstraintAddress.into());
    }

    Ok(())
}
