use naclac_lang::prelude::*;
use crate::components::SeededThing;
use crate::constants::SEED_SEEDED;

#[derive(Accounts)]
pub struct InitSeeded {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        init,
        payer = payer,
        seeds = [SEED_SEEDED],
        bump
    )]
    pub seeded: Account<SeededThing>,

    pub system_program: Program<System>,
}

#[instruction]
pub fn init_seeded(ctx: Context<InitSeeded>) -> Result {
    ctx.accounts.seeded.value = 7;
    Ok(())
}
