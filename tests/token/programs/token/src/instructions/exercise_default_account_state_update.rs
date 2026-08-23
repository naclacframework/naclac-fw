use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT_AUTHORITY;

/// Real end-to-end proof of `update_default_account_state`
/// (`naclac-token/src/extensions/default_account_state.rs`) — updates an
/// already-initialized mint's default account state via CPI, signed by the
/// mint's freeze authority, then reads it back to confirm the on-chain
/// value actually changed, not just that Token-2022 accepted the
/// instruction.
#[derive(Accounts)]
pub struct ExerciseDefaultAccountStateUpdate {
    #[account(seeds = [SEED_MINT_AUTHORITY], bump = mint_authority.bump)]
    pub mint_authority: Account<MintAuthority>,

    #[account(mut)]
    pub mint: InterfaceAccount<Mint>,

    pub token_program: Program<Token2022>,
}

#[instruction]
pub fn exercise_default_account_state_update(
    ctx: Context<ExerciseDefaultAccountStateUpdate>,
    new_state: u8,
) -> Result {
    let bump = ctx.accounts.mint_authority.bump;
    let signer_seeds: &[&[u8]] = &[SEED_MINT_AUTHORITY, &[bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];

    update_default_account_state_signed(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        ctx.accounts.mint_authority.to_cpi_handle(),
        new_state,
        signer,
    )?;

    let config: DefaultAccountState = ctx.accounts.mint.get_extension()?;
    if config.state() != new_state {
        return Err(NaclacError::ConstraintAddress.into());
    }

    Ok(())
}
