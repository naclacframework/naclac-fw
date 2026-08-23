use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT_AUTHORITY;

/// Real end-to-end proof of `resume_mint` (`naclac-token/src/extensions/pausable.rs`)
/// — resumes an already-paused mint via CPI, signed by the mint's pause
/// authority, then reads it back to confirm the on-chain flag actually
/// changed, not just that Token-2022 accepted the instruction.
#[derive(Accounts)]
pub struct ExerciseResumeMint {
    #[account(seeds = [SEED_MINT_AUTHORITY], bump = mint_authority.bump)]
    pub mint_authority: Account<MintAuthority>,

    #[account(mut)]
    pub mint: InterfaceAccount<Mint>,

    pub token_program: Program<Token2022>,
}

#[instruction]
pub fn exercise_resume_mint(ctx: Context<ExerciseResumeMint>) -> Result {
    let bump = ctx.accounts.mint_authority.bump;
    let signer_seeds: &[&[u8]] = &[SEED_MINT_AUTHORITY, &[bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];

    resume_mint_signed(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        ctx.accounts.mint_authority.to_cpi_handle(),
        signer,
    )?;

    let config: PausableConfig = ctx.accounts.mint.get_extension()?;
    if config.paused() {
        return Err(NaclacError::ConstraintAddress.into());
    }

    Ok(())
}
