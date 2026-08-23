use naclac_lang::prelude::*;
use crate::components::SeededThing;
use crate::constants::SEED_SEEDED;

// `seeds` + explicit `bump = <expr>` on an existing (non-`init`) account —
// the on-chain-`find_program_address`-banned hash-and-compare path
// (`security.rs`), positive with the correct stored bump and negative with
// a deliberately wrong one (`NaclacError::ConstraintSeeds`).
#[derive(Accounts)]
#[instruction(bump: u8)]
pub struct TouchSeeded {
    #[account(mut, seeds = [SEED_SEEDED], bump = bump)]
    pub seeded: Account<SeededThing>,
}

#[instruction]
pub fn touch_seeded(ctx: Context<TouchSeeded>, _bump: u8) -> Result {
    ctx.accounts.seeded.value += 1;
    Ok(())
}
