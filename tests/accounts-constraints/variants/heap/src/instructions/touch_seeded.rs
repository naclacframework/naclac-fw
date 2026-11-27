use naclac_lang::prelude::*;
use crate::state::SeededThing;
use crate::constants::SEED_SEEDED;

// `seeds` + a bare `bump` on an existing (non-`init`) account whose seeds are
// all constants: the address and canonical bump are computed at compile time.
#[derive(Accounts)]
pub struct TouchSeeded {
    #[account(mut, seeds = [SEED_SEEDED], bump)]
    pub seeded: Account<SeededThing>,
}

pub fn touch_seeded(ctx: Context<TouchSeeded>) -> Result {
    ctx.accounts.seeded.value += 1;
    Ok(())
}
