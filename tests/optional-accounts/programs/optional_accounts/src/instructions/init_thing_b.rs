use naclac_lang::prelude::*;
use crate::components::Thing;
use crate::constants::SEED_THING_B;

#[derive(Accounts)]
pub struct InitThingB {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        init,
        payer = payer,
        seeds = [SEED_THING_B]
    )]
    pub thing_b: Account<Thing>,

    pub system_program: Program<System>,
}

#[instruction]
pub fn init_thing_b(ctx: Context<InitThingB>) -> Result {
    let thing_b = &mut ctx.accounts.thing_b;
    thing_b.value = 0;
    Ok(())
}
