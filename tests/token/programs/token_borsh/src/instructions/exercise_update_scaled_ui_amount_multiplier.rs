use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT_AUTHORITY;

/// Real end-to-end proof of `update_scaled_ui_amount_multiplier`
/// (`naclac-token/src/extensions/scaled_ui_amount.rs`) — updates an
/// already-initialized mint's scheduled multiplier via CPI, signed by the
/// mint's multiplier authority, then reads it back to confirm the on-chain
/// `new_multiplier` field actually changed, not just that Token-2022
/// accepted the instruction. Asserts `new_multiplier` specifically (not
/// `multiplier`) since the real processor only promotes `new_multiplier`
/// into `multiplier` once `effective_timestamp` has passed relative to the
/// `Clock` sysvar — `new_multiplier` itself is unconditionally overwritten
/// by every `UpdateMultiplier` call, verified against the real
/// `scaled_ui_amount::processor::process_update_multiplier` before writing
/// this assertion.
#[derive(Accounts)]
pub struct ExerciseUpdateScaledUiAmountMultiplier {
    #[account(seeds = [SEED_MINT_AUTHORITY], bump = mint_authority.bump)]
    pub mint_authority: Account<MintAuthority>,

    #[account(mut)]
    pub mint: InterfaceAccount<Mint>,

    pub token_program: Program<Token2022>,
}

#[instruction]
pub fn exercise_update_scaled_ui_amount_multiplier(
    ctx: Context<ExerciseUpdateScaledUiAmountMultiplier>,
    new_multiplier_bits: u64,
    effective_timestamp: i64,
) -> Result {
    let new_multiplier = f64::from_bits(new_multiplier_bits);
    let bump = ctx.accounts.mint_authority.bump;
    let signer_seeds: &[&[u8]] = &[SEED_MINT_AUTHORITY, &[bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];

    update_scaled_ui_amount_multiplier_signed(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        ctx.accounts.mint_authority.to_cpi_handle(),
        new_multiplier,
        effective_timestamp,
        signer,
    )?;

    let config: ScaledUiAmountConfig = ctx.accounts.mint.get_extension()?;
    if config.new_multiplier() != new_multiplier {
        return Err(NaclacError::ConstraintAddress.into());
    }

    Ok(())
}
